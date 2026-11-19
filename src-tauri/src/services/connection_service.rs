//! 连接管理业务：配置 CRUD、连通性测试、运行时连接生命周期。
//!
//! 安全约束：密码只写入系统凭据服务，SQLite 仅保存 `credential_key`；
//! 任何返回结构与日志都不得包含密码。

use std::sync::Arc;

use serde_json::json;

use crate::adapters::mysql::connection as mysql_connection;
use crate::adapters::pg::connection as pg_connection;
use crate::adapters::redis::connection as redis_connection;
use crate::core::{
    close_connection, ConnectionManager, ManagedConnection, ManagedPg, ManagedRedis, PgManager,
    RedisManager,
};
use crate::credentials::CredentialStore;
use crate::database::Database;
use crate::errors::{error_code, AppError};
use crate::models::connection::{
    ConnectDatabaseResponse, ConnectionExportEntry, ConnectionExportFile, ConnectionImportResponse,
    ConnectionInfo, ConnectionRuntimeInfo, ConnectionStatus, ConnectionTarget,
    CreateConnectionRequest, CreateConnectionResponse, DisconnectDatabaseResponse,
    SimpleSuccessResponse, TestConnectionRequest, TestConnectionResponse, UpdateConnectionRequest,
    DEFAULT_CONNECT_TIMEOUT_MS, DEFAULT_MYSQL_PORT, DEFAULT_REDIS_PORT, SOURCE_TYPE_MYSQL,
    SOURCE_TYPE_POSTGRES, SOURCE_TYPE_REDIS,
};
use crate::models::data_source::{DataSourceConfig, NewDataSource};
use crate::repositories::data_source_repository;
use crate::utils::time::{new_id, now_ms, to_db_time};

const CREDENTIAL_KEY_PREFIX: &str = "datadock";
const MAX_NAME_LEN: usize = 100;
const MAX_DATABASE_LEN: usize = 64;
const MAX_CHARSET_LEN: usize = 32;

pub struct ConnectionService {
    db: Database,
    credentials: Arc<dyn CredentialStore>,
    manager: Arc<ConnectionManager>,
    redis: Arc<RedisManager>,
    pg: Arc<PgManager>,
    /// 活跃的 SSH 隧道，按连接 ID 持久存活，断开时关闭
    tunnels: tokio::sync::Mutex<
        std::collections::HashMap<String, crate::adapters::ssh_tunnel::SshTunnel>,
    >,
}

impl ConnectionService {
    pub fn new(
        db: Database,
        credentials: Arc<dyn CredentialStore>,
        manager: Arc<ConnectionManager>,
        redis: Arc<RedisManager>,
        pg: Arc<PgManager>,
    ) -> Self {
        Self {
            db,
            credentials,
            manager,
            redis,
            pg,
            tunnels: tokio::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// 关闭并移除某连接的 SSH 隧道（若存在）。
    async fn close_tunnel(&self, connection_id: &str) {
        if let Some(mut tunnel) = self.tunnels.lock().await.remove(connection_id) {
            tunnel.close().await;
            tracing::info!(target: "datadock::ssh", connection_id, "SSH 隧道已关闭");
        }
    }

    pub async fn list(&self) -> Result<Vec<ConnectionInfo>, AppError> {
        let rows = data_source_repository::list(self.db.pool()).await?;
        let mut infos = Vec::with_capacity(rows.len());

        for row in rows {
            let runtime = self.manager.get(&row.id).await;
            let redis_runtime = self.redis.get(&row.id).await;
            let pg_runtime = self.pg.get(&row.id).await;
            infos.push(to_info(row, runtime, redis_runtime, pg_runtime));
        }

        Ok(infos)
    }

    /// 导出全部连接配置（不含密码），用于迁移到其他电脑。
    pub async fn export_connections(&self) -> Result<ConnectionExportFile, AppError> {
        let rows = data_source_repository::list(self.db.pool()).await?;
        let mut connections = Vec::with_capacity(rows.len());

        for row in rows {
            let (charset, connect_timeout_ms, options) =
                parse_options_json(row.options_json.as_deref());

            let is_redis = row.source_type.eq_ignore_ascii_case(SOURCE_TYPE_REDIS);
            let fallback_port = if is_redis {
                DEFAULT_REDIS_PORT
            } else {
                DEFAULT_MYSQL_PORT
            };
            let port = u16::try_from(row.port).unwrap_or(fallback_port);

            connections.push(ConnectionExportEntry {
                name: row.name,
                source_type: row.source_type,
                host: row.host,
                port,
                username: row.username,
                default_database: row.default_database,
                charset,
                connect_timeout_ms,
                options,
            });
        }

        Ok(ConnectionExportFile {
            format: "datadock-connections".to_string(),
            version: 1,
            exported_at: to_db_time(now_ms()),
            connections,
        })
    }

    /// 导入连接配置：重名的跳过（避免覆盖），返回导入与跳过数量。
    /// 导入的连接不含密码，用户需在目标机器上编辑连接重新输入。
    pub async fn import_connections(
        &self,
        file: &ConnectionExportFile,
    ) -> Result<ConnectionImportResponse, AppError> {
        if file.format != "datadock-connections" {
            return Err(AppError::validation("不是 DataDock 连接导出文件"));
        }
        if file.version != 1 {
            return Err(AppError::validation(format!(
                "不支持的导出文件版本：{}（当前支持 1）",
                file.version
            )));
        }

        let mut imported = 0;
        let mut skipped = 0;

        for entry in &file.connections {
            let target = ConnectionTarget {
                host: entry.host.clone(),
                port: entry.port,
                username: entry.username.clone(),
                default_database: entry.default_database.clone(),
                charset: entry.charset.clone(),
                connect_timeout_ms: entry
                    .connect_timeout_ms
                    .unwrap_or(DEFAULT_CONNECT_TIMEOUT_MS),
                options: entry.options.clone(),
            };

            // 重名跳过：避免误覆盖已有连接
            if validate_target(&target, &entry.name, &entry.source_type).is_err()
                || self.name_exists(&entry.name).await
            {
                skipped += 1;
                continue;
            }

            let request = CreateConnectionRequest {
                name: entry.name.clone(),
                source_type: entry.source_type.clone(),
                host: entry.host.clone(),
                port: entry.port,
                username: entry.username.clone(),
                password: String::new(), // 导入不含密码，用户编辑时补
                default_database: entry.default_database.clone(),
                charset: entry.charset.clone(),
                connect_timeout_ms: entry.connect_timeout_ms,
                options: Some(entry.options.clone()),
            };

            match self.create(request).await {
                Ok(_) => imported += 1,
                Err(_) => skipped += 1,
            }
        }

        Ok(ConnectionImportResponse { imported, skipped })
    }

    /// 判断同名连接是否已存在（名称去重）。
    async fn name_exists(&self, name: &str) -> bool {
        let Ok(rows) = data_source_repository::list(self.db.pool()).await else {
            return false;
        };
        rows.iter().any(|row| row.name == name)
    }

    pub async fn create(
        &self,
        request: CreateConnectionRequest,
    ) -> Result<CreateConnectionResponse, AppError> {
        let target = ConnectionTarget::from_create(&request);
        validate_target(&target, &request.name, &request.source_type)?;

        let id = new_id();
        let credential_key = format!("{CREDENTIAL_KEY_PREFIX}/{id}");

        // 先写凭据，失败则整体失败，不留下无凭据的连接配置
        self.credentials
            .set_secret(&credential_key, &request.password)?;

        let source_type = normalize_source_type(&request.source_type);

        let input = NewDataSource {
            name: request.name.trim().to_string(),
            source_type,
            host: target.host.clone(),
            port: target.port,
            username: target.username.clone(),
            default_database: target.default_database.clone(),
            options_json: build_options_json(&target),
            remark: None,
        };

        if let Err(error) =
            data_source_repository::insert(self.db.pool(), &id, &input, Some(&credential_key)).await
        {
            // 补偿：配置写入失败时清掉刚写入的凭据
            let _ = self.credentials.delete_secret(&credential_key);
            return Err(error);
        }

        tracing::info!(target: "datadock::connection", connection_id = %id, "连接配置已创建");

        Ok(CreateConnectionResponse { connection_id: id })
    }

    pub async fn update(
        &self,
        request: UpdateConnectionRequest,
    ) -> Result<SimpleSuccessResponse, AppError> {
        let existing = data_source_repository::get(self.db.pool(), &request.connection_id)
            .await?
            .ok_or_else(|| AppError::new(error_code::CONNECTION_NOT_FOUND, "连接配置不存在"))?;

        let target = ConnectionTarget::from_update(&request);
        validate_target(&target, &request.name, &existing.source_type)?;

        let mut credential_key = existing.credential_key.clone();
        if let Some(password) = request
            .new_password
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            let key = credential_key
                .clone()
                .unwrap_or_else(|| format!("{CREDENTIAL_KEY_PREFIX}/{}", existing.id));
            self.credentials.set_secret(&key, password)?;
            credential_key = Some(key);
        }

        let input = NewDataSource {
            name: request.name.trim().to_string(),
            source_type: existing.source_type.clone(),
            host: target.host.clone(),
            port: target.port,
            username: target.username.clone(),
            default_database: target.default_database.clone(),
            options_json: build_options_json(&target),
            remark: existing.remark.clone(),
        };

        let updated = data_source_repository::update(
            self.db.pool(),
            &existing.id,
            &input,
            credential_key.as_deref(),
        )
        .await?;

        if !updated {
            return Err(AppError::new(
                error_code::CONNECTION_NOT_FOUND,
                "连接配置不存在",
            ));
        }

        // 连接参数变化后旧会话可能已失效，主动断开，提示用户重新连接
        if let Some(runtime) = self.manager.remove(&existing.id).await {
            close_connection(runtime).await;
            tracing::info!(target: "datadock::connection", connection_id = %existing.id, "连接配置已变更，运行时连接已断开");
        }

        Ok(SimpleSuccessResponse { success: true })
    }

    pub async fn delete(&self, connection_id: &str) -> Result<SimpleSuccessResponse, AppError> {
        let existing = data_source_repository::get(self.db.pool(), connection_id)
            .await?
            .ok_or_else(|| AppError::new(error_code::CONNECTION_NOT_FOUND, "连接配置不存在"))?;

        if let Some(runtime) = self.manager.remove(connection_id).await {
            close_connection(runtime).await;
        }
        self.redis.remove(connection_id).await;
        self.close_tunnel(connection_id).await;

        // 凭据删除失败时不删除配置，避免留下无法清理的凭据
        if let Some(key) = existing.credential_key.as_deref() {
            self.credentials.delete_secret(key)?;
        }

        let deleted = data_source_repository::delete(self.db.pool(), connection_id).await?;
        if !deleted {
            return Err(AppError::new(
                error_code::CONNECTION_NOT_FOUND,
                "连接配置不存在",
            ));
        }

        tracing::info!(target: "datadock::connection", connection_id, "连接配置已删除");

        Ok(SimpleSuccessResponse { success: true })
    }

    pub async fn test(
        &self,
        request: TestConnectionRequest,
    ) -> Result<TestConnectionResponse, AppError> {
        let target = ConnectionTarget::from_test(&request);
        validate_target(&target, "测试连接", &request.source_type)?;

        // SSH 隧道：隧道仅存活于本次测试，结束时自动关闭
        if target.options.ssh_enabled {
            let mut tunnel = match open_tunnel_for(&target, &request.password).await {
                Ok(tunnel) => tunnel,
                Err(error) => {
                    return Ok(TestConnectionResponse {
                        success: false,
                        message: error.message,
                        server_version: None,
                        elapsed_ms: 0,
                    });
                }
            };
            let mut tunneled = target.clone();
            tunneled.host = tunnel.local_addr.ip().to_string();
            tunneled.port = tunnel.local_addr.port();
            let result = self.run_test(&tunneled, &request).await;
            tunnel.close().await;
            return result;
        }

        self.run_test(&target, &request).await
    }

    /// 按数据源类型执行连通性测试（目标已是最终地址，可能经过隧道改写）。
    async fn run_test(
        &self,
        target: &ConnectionTarget,
        request: &TestConnectionRequest,
    ) -> Result<TestConnectionResponse, AppError> {
        if normalize_source_type(&request.source_type) == SOURCE_TYPE_REDIS {
            let params = redis_params(target, &request.password);
            return match redis_connection::test(&params).await {
                Ok(version) => Ok(TestConnectionResponse {
                    success: true,
                    message: "连接成功".to_string(),
                    server_version: Some(version),
                    elapsed_ms: 0,
                }),
                Err(error) => Ok(TestConnectionResponse {
                    success: false,
                    message: error.message,
                    server_version: None,
                    elapsed_ms: 0,
                }),
            };
        }

        if normalize_source_type(&request.source_type) == SOURCE_TYPE_POSTGRES {
            let params = pg_params(target, &request.password);
            return match pg_connection::connect_single(&params).await {
                Ok(pool) => {
                    let version = pg_server_version(&pool).await;
                    pool.close().await;
                    Ok(TestConnectionResponse {
                        success: true,
                        message: "连接成功".to_string(),
                        server_version: Some(version),
                        elapsed_ms: 0,
                    })
                }
                Err(error) => Ok(TestConnectionResponse {
                    success: false,
                    message: error.message,
                    server_version: None,
                    elapsed_ms: 0,
                }),
            };
        }

        match mysql_connection::test(target, &request.password).await {
            Ok(outcome) => Ok(TestConnectionResponse {
                success: true,
                message: "连接成功".to_string(),
                server_version: Some(outcome.server_version),
                elapsed_ms: outcome.elapsed_ms,
            }),
            // 测试失败是正常的业务结果，返回 success=false 而不是抛错
            Err(error) => Ok(TestConnectionResponse {
                success: false,
                message: error.message,
                server_version: None,
                elapsed_ms: 0,
            }),
        }
    }

    pub async fn connect(&self, connection_id: &str) -> Result<ConnectDatabaseResponse, AppError> {
        let config = data_source_repository::get(self.db.pool(), connection_id)
            .await?
            .ok_or_else(|| AppError::new(error_code::CONNECTION_NOT_FOUND, "连接配置不存在"))?;

        if self.manager.is_connected(connection_id).await
            || self.redis.is_connected(connection_id).await
            || self.pg.is_connected(connection_id).await
        {
            return Err(AppError::new(
                error_code::CONNECTION_ALREADY_CONNECTED,
                "该连接已经建立，请先断开",
            ));
        }

        let credential_key = config.credential_key.clone().ok_or_else(|| {
            AppError::new(
                error_code::CREDENTIAL_NOT_FOUND,
                "该连接没有保存的密码，请编辑连接重新输入",
            )
        })?;

        let password = self
            .credentials
            .get_secret(&credential_key)?
            .ok_or_else(|| {
                AppError::new(
                    error_code::CREDENTIAL_NOT_FOUND,
                    "系统凭据中找不到该连接的密码，请编辑连接重新输入",
                )
            })?;

        let target = target_from_config(&config);

        // SSH 隧道：建立后把连接目标改写为本地转发地址，隧道随连接存活
        let target = if target.options.ssh_enabled {
            let tunnel = open_tunnel_for(&target, &password).await?;
            let mut tunneled = target;
            tunneled.host = tunnel.local_addr.ip().to_string();
            tunneled.port = tunnel.local_addr.port();
            self.tunnels
                .lock()
                .await
                .insert(connection_id.to_string(), tunnel);
            tracing::info!(target: "datadock::ssh", connection_id, "SSH 隧道已建立");
            tunneled
        } else {
            target
        };

        if config.source_type.eq_ignore_ascii_case(SOURCE_TYPE_REDIS) {
            let params = redis_params(&target, &password);
            let database = params.database;
            let endpoint = crate::core::RedisEndpoint {
                host: params.host.to_string(),
                port: params.port,
                username: params.username.to_string(),
                password: password.to_string(),
                use_tls: target.options.ssl_enabled,
            };
            let (connection, server_version) = redis_connection::connect(&params).await?;
            let current_database = format!("db{database}");

            self.redis
                .insert(
                    connection_id.to_string(),
                    ManagedRedis::new(connection, server_version.clone(), database, endpoint),
                )
                .await;

            tracing::info!(
                target: "datadock::connection",
                connection_id,
                server_version = %server_version,
                "Redis 连接已建立"
            );

            return Ok(ConnectDatabaseResponse {
                connection_id: connection_id.to_string(),
                status: ConnectionStatus::Connected.as_str().to_string(),
                server_version: Some(server_version),
                current_database: Some(current_database),
            });
        }

        if config
            .source_type
            .eq_ignore_ascii_case(SOURCE_TYPE_POSTGRES)
        {
            let params = pg_params(&target, &password);
            let database = params
                .database
                .clone()
                .unwrap_or_else(|| "postgres".to_string());
            let pool = pg_connection::connect_single(&params).await?;
            let server_version = pg_server_version(&pool).await;

            self.pg
                .insert(
                    connection_id.to_string(),
                    crate::core::ManagedPg::new(
                        pool,
                        server_version.clone(),
                        params.host,
                        params.port,
                        params.username,
                        password,
                        database.clone(),
                        params.ssl_mode,
                    ),
                )
                .await;

            tracing::info!(
                target: "datadock::connection",
                connection_id,
                server_version = %server_version,
                "PostgreSQL 连接已建立"
            );

            return Ok(ConnectDatabaseResponse {
                connection_id: connection_id.to_string(),
                status: ConnectionStatus::Connected.as_str().to_string(),
                server_version: Some(server_version),
                current_database: Some(database),
            });
        }

        let options = mysql_connection::connect_options(&target, &password);
        let pool = mysql_connection::create_pool(&target, &password).await?;
        let server_version = mysql_connection::server_version(&pool).await?;
        let server_connection_id = mysql_connection::server_connection_id(&pool).await?;

        self.manager
            .insert(
                connection_id.to_string(),
                ManagedConnection::new(
                    pool,
                    options,
                    server_version.clone(),
                    server_connection_id,
                    config.default_database.clone(),
                ),
            )
            .await;

        tracing::info!(
            target: "datadock::connection",
            connection_id,
            server_version = %server_version,
            "数据库连接已建立"
        );

        Ok(ConnectDatabaseResponse {
            connection_id: connection_id.to_string(),
            status: ConnectionStatus::Connected.as_str().to_string(),
            server_version: Some(server_version),
            current_database: config.default_database,
        })
    }

    pub async fn disconnect(
        &self,
        connection_id: &str,
    ) -> Result<DisconnectDatabaseResponse, AppError> {
        if let Some(runtime) = self.manager.remove(connection_id).await {
            close_connection(runtime).await;
            tracing::info!(target: "datadock::connection", connection_id, "数据库连接已断开");
        }
        if let Some(pg_runtime) = self.pg.remove(connection_id).await {
            pg_runtime.close_all().await;
            tracing::info!(target: "datadock::connection", connection_id, "PostgreSQL 连接已断开");
        }
        if self.redis.remove(connection_id).await.is_some() {
            tracing::info!(target: "datadock::connection", connection_id, "Redis 连接已断开");
        }
        self.close_tunnel(connection_id).await;

        Ok(DisconnectDatabaseResponse {
            connection_id: connection_id.to_string(),
            status: ConnectionStatus::Disconnected.as_str().to_string(),
        })
    }

    pub async fn status(&self, connection_id: &str) -> ConnectionRuntimeInfo {
        if let Some(runtime) = self.pg.get(connection_id).await {
            return ConnectionRuntimeInfo {
                connection_id: connection_id.to_string(),
                status: ConnectionStatus::Connected.as_str().to_string(),
                current_database: Some(runtime.current_database()),
                connected_at: Some(to_db_time(runtime.connected_at_ms)),
                last_error: None,
            };
        }

        if let Some(runtime) = self.redis.get(connection_id).await {
            return ConnectionRuntimeInfo {
                connection_id: connection_id.to_string(),
                status: ConnectionStatus::Connected.as_str().to_string(),
                current_database: Some(runtime.current_database()),
                connected_at: Some(to_db_time(runtime.connected_at_ms)),
                last_error: None,
            };
        }

        match self.manager.get(connection_id).await {
            Some(runtime) => ConnectionRuntimeInfo {
                connection_id: connection_id.to_string(),
                status: ConnectionStatus::Connected.as_str().to_string(),
                current_database: runtime.current_database.clone(),
                connected_at: Some(to_db_time(runtime.connected_at_ms)),
                last_error: None,
            },
            None => ConnectionRuntimeInfo {
                connection_id: connection_id.to_string(),
                status: ConnectionStatus::Disconnected.as_str().to_string(),
                current_database: None,
                connected_at: None,
                last_error: None,
            },
        }
    }
}

fn to_info(
    config: DataSourceConfig,
    runtime: Option<Arc<ManagedConnection>>,
    redis_runtime: Option<Arc<ManagedRedis>>,
    pg_runtime: Option<Arc<ManagedPg>>,
) -> ConnectionInfo {
    let (charset, _, options) = parse_options_json(config.options_json.as_deref());
    let is_redis = config.source_type.eq_ignore_ascii_case(SOURCE_TYPE_REDIS);

    ConnectionInfo {
        connected: runtime.is_some() || redis_runtime.is_some() || pg_runtime.is_some(),
        server_version: runtime
            .map(|connection| connection.server_version.clone())
            .or_else(|| redis_runtime.map(|connection| connection.server_version.clone()))
            .or_else(|| pg_runtime.map(|connection| connection.server_version.clone())),
        id: config.id,
        name: config.name,
        source_type: config.source_type,
        host: config.host,
        port: u16::try_from(config.port).unwrap_or(if is_redis {
            DEFAULT_REDIS_PORT
        } else {
            DEFAULT_MYSQL_PORT
        }),
        username: config.username,
        default_database: config.default_database,
        charset,
        created_at: config.created_at,
        updated_at: config.updated_at,
        options,
    }
}

fn normalize_source_type(source_type: &str) -> String {
    if source_type.eq_ignore_ascii_case(SOURCE_TYPE_REDIS) {
        SOURCE_TYPE_REDIS.to_string()
    } else if source_type.eq_ignore_ascii_case(SOURCE_TYPE_POSTGRES) {
        SOURCE_TYPE_POSTGRES.to_string()
    } else {
        SOURCE_TYPE_MYSQL.to_string()
    }
}

/// 若启用 SSH 隧道，建立隧道并把目标改写为本地转发地址。
/// 密码认证时复用数据库密码（前端约定）。
async fn open_tunnel_for(
    target: &ConnectionTarget,
    db_password: &str,
) -> Result<crate::adapters::ssh_tunnel::SshTunnel, AppError> {
    let options = &target.options;
    let ssh = crate::models::connection::SshConfig {
        host: options.ssh_host.clone().unwrap_or_default(),
        port: options.ssh_port.unwrap_or(22),
        username: options.ssh_username.clone().unwrap_or_default(),
        auth_type: options.ssh_auth_type.unwrap_or_default(),
        private_key_path: options.ssh_private_key_path.clone(),
        password: Some(db_password.to_string()),
    };

    // 目标 host 是数据库真实地址；经隧道后连接本地转发端口
    crate::adapters::ssh_tunnel::SshTunnel::open(&ssh, &target.host, target.port).await
}

fn redis_params<'a>(
    target: &'a ConnectionTarget,
    password: &'a str,
) -> redis_connection::RedisParams<'a> {
    redis_connection::RedisParams {
        host: &target.host,
        port: target.port,
        username: &target.username,
        password,
        database: target
            .default_database
            .as_deref()
            .and_then(|value| value.trim().parse::<u32>().ok())
            .unwrap_or(0),
        use_tls: target.options.ssl_enabled,
    }
}

async fn pg_server_version(pool: &sqlx::PgPool) -> String {
    let row: Option<(String,)> = sqlx::query_as("SELECT version()")
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
    row.map(|(value,)| value)
        .unwrap_or_else(|| "PostgreSQL".to_string())
}

fn pg_params<'a>(target: &'a ConnectionTarget, password: &'a str) -> pg_connection::PgParams {
    // 用户配置的 SSL 模式；未启用加密时回退 disabled，启用则按所选模式
    let ssl_mode = if target.options.ssl_enabled {
        target.options.ssl_mode.unwrap_or_default()
    } else {
        crate::models::connection::SslMode::Disabled
    };

    pg_connection::PgParams {
        host: target.host.clone(),
        port: target.port,
        username: target.username.clone(),
        password: password.to_string(),
        database: target.default_database.clone(),
        ssl_mode: pg_connection::ssl_mode_from_ssl(ssl_mode),
        connect_timeout: std::time::Duration::from_millis(target.connect_timeout_ms.min(30_000)),
    }
}

fn target_from_config(config: &DataSourceConfig) -> ConnectionTarget {
    let (charset, connect_timeout_ms, options) = parse_options_json(config.options_json.as_deref());
    let is_redis = config.source_type.eq_ignore_ascii_case(SOURCE_TYPE_REDIS);

    ConnectionTarget {
        host: config.host.clone(),
        port: u16::try_from(config.port).unwrap_or(if is_redis {
            DEFAULT_REDIS_PORT
        } else {
            DEFAULT_MYSQL_PORT
        }),
        username: config.username.clone(),
        default_database: config.default_database.clone(),
        charset,
        connect_timeout_ms: connect_timeout_ms
            .unwrap_or(crate::models::connection::DEFAULT_CONNECT_TIMEOUT_MS),
        options,
    }
}

fn build_options_json(target: &ConnectionTarget) -> Option<String> {
    // 无扩展配置（charset/timeout/SSL/SSH）时不写 options_json，保持旧行为
    let has_charset = target.charset.is_some();
    let has_timeout =
        target.connect_timeout_ms != crate::models::connection::DEFAULT_CONNECT_TIMEOUT_MS;
    let has_ssl = target.options.ssl_enabled;
    let has_ssh = target.options.ssh_enabled;

    if !has_charset && !has_timeout && !has_ssl && !has_ssh {
        return None;
    }

    let options_value = serde_json::to_value(&target.options).unwrap_or_default();
    let mut merged = options_value.as_object().cloned().unwrap_or_default();
    // 始终写入 charset / timeout，保证旧行为不回归
    merged.insert("charset".to_string(), json!(target.charset));
    merged.insert(
        "connectTimeoutMs".to_string(),
        json!(target.connect_timeout_ms),
    );

    Some(serde_json::Value::Object(merged).to_string())
}

fn parse_options_json(
    raw: Option<&str>,
) -> (
    Option<String>,
    Option<u64>,
    crate::models::connection::ConnectionOptions,
) {
    let Some(raw) = raw else {
        return (None, None, Default::default());
    };

    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return (None, None, Default::default());
    };

    let charset = value
        .get("charset")
        .and_then(|item| item.as_str())
        .map(str::to_string);
    let timeout = value.get("connectTimeoutMs").and_then(|item| item.as_u64());
    let options =
        serde_json::from_value::<crate::models::connection::ConnectionOptions>(value.clone())
            .unwrap_or_default();

    (charset, timeout, options)
}

fn validate_target(
    target: &ConnectionTarget,
    name: &str,
    source_type: &str,
) -> Result<(), AppError> {
    if !crate::models::connection::is_supported_source_type(source_type) {
        return Err(AppError::validation(format!(
            "当前版本仅支持 MySQL 数据源，收到 {source_type}"
        )));
    }

    let trimmed_name = name.trim();
    if trimmed_name.is_empty() {
        return Err(AppError::validation("连接名称不能为空"));
    }
    if trimmed_name.chars().count() > MAX_NAME_LEN {
        return Err(AppError::validation(format!(
            "连接名称过长，最多 {MAX_NAME_LEN} 个字符"
        )));
    }

    if target.host.trim().is_empty() {
        return Err(AppError::validation("主机地址不能为空"));
    }
    if target.port == 0 {
        return Err(AppError::validation("端口必须在 1~65535 之间"));
    }
    // Redis 支持匿名/默认用户（ACL 未启用时用户名可留空）
    if !source_type.eq_ignore_ascii_case(SOURCE_TYPE_REDIS) && target.username.trim().is_empty() {
        return Err(AppError::validation("用户名不能为空"));
    }

    if let Some(database) = &target.default_database {
        if database.chars().count() > MAX_DATABASE_LEN {
            return Err(AppError::validation("默认数据库名过长"));
        }
    }

    if let Some(charset) = &target.charset {
        if charset.chars().count() > MAX_CHARSET_LEN {
            return Err(AppError::validation("字符集名称过长"));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_json_round_trip() {
        let target = ConnectionTarget {
            host: "127.0.0.1".into(),
            port: 3306,
            username: "root".into(),
            default_database: None,
            charset: Some("utf8mb4".into()),
            connect_timeout_ms: 5_000,
            options: Default::default(),
        };

        let raw = build_options_json(&target).expect("应生成 options");
        let (charset, timeout, _) = parse_options_json(Some(&raw));

        assert_eq!(charset.as_deref(), Some("utf8mb4"));
        assert_eq!(timeout, Some(5_000));
    }

    #[test]
    fn default_options_are_omitted() {
        let target = ConnectionTarget {
            host: "127.0.0.1".into(),
            port: 3306,
            username: "root".into(),
            default_database: None,
            charset: None,
            connect_timeout_ms: crate::models::connection::DEFAULT_CONNECT_TIMEOUT_MS,
            options: Default::default(),
        };

        assert!(build_options_json(&target).is_none());
    }

    #[test]
    fn invalid_targets_are_rejected() {
        let mut target = ConnectionTarget {
            host: "127.0.0.1".into(),
            port: 3306,
            username: "root".into(),
            default_database: None,
            charset: None,
            connect_timeout_ms: 8_000,
            options: Default::default(),
        };

        assert!(validate_target(&target, "", "mysql").is_err());
        assert!(validate_target(&target, "本地", "postgresql").is_err());

        target.port = 0;
        assert!(validate_target(&target, "本地", "mysql").is_err());

        target.port = 3306;
        target.host = "  ".into();
        assert!(validate_target(&target, "本地", "mysql").is_err());
    }
}
