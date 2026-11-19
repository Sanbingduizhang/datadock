//! SSH 隧道：通过跳板机把本地端口加密转发到目标数据库 host:port。
//!
//! 用法：`open_tunnel` 建立隧道并返回本地监听地址，上层把数据库连接目标
//! 改写为该地址即可，数据库适配器无需感知 SSH 的存在。

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use russh::client::{Config, Handle};
use russh::keys::{load_secret_key, PrivateKeyWithHashAlg};
use russh::{client, ChannelMsg, Disconnect};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

use crate::errors::{error_code, AppError};
use crate::models::connection::{SshAuthType, SshConfig};

/// 已建立的 SSH 隧道。
///
/// 生命周期：持有期间隧道保持可用；丢弃时后台转发任务停止，SSH 会话关闭。
pub struct SshTunnel {
    /// 本地监听地址（数据库应连接这里）
    pub local_addr: SocketAddr,
    session: Arc<Mutex<Handle<ClientHandler>>>,
    forward_task: tokio::task::JoinHandle<()>,
    closed: bool,
}

impl SshTunnel {
    /// 建立到跳板机的 SSH 连接，并在本地随机端口开启监听。
    pub async fn open(
        ssh: &SshConfig,
        target_host: &str,
        target_port: u16,
    ) -> Result<Self, AppError> {
        let config = Arc::new(Config {
            nodelay: true,
            ..Default::default()
        });

        let mut session = client::connect(config, (ssh.host.as_str(), ssh.port), ClientHandler)
            .await
            .map_err(|error| {
                AppError::new(
                    error_code::CONNECTION_FAILED,
                    format!("连接 SSH 跳板机失败：{error}"),
                )
            })?;

        authenticate(&mut session, ssh).await?;

        let listener = TcpListener::bind("127.0.0.1:0").await.map_err(|error| {
            AppError::new(
                error_code::CONNECTION_FAILED,
                format!("SSH 隧道本地端口绑定失败：{error}"),
            )
        })?;
        let local_addr = listener.local_addr().map_err(|error| {
            AppError::new(
                error_code::CONNECTION_FAILED,
                format!("SSH 隧道读取本地地址失败：{error}"),
            )
        })?;

        // 后台任务：每接受一个本地连接，就开一条 direct-tcpip 通道转发
        let session = Arc::new(Mutex::new(session));
        let forward_host = target_host.to_string();
        let forward_task = tokio::spawn(tunnel_loop(
            listener,
            session.clone(),
            forward_host,
            target_port,
        ));

        Ok(Self {
            local_addr,
            session,
            forward_task,
            closed: false,
        })
    }

    /// 关闭 SSH 会话与转发任务（幂等）。
    pub async fn close(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        self.forward_task.abort();
        let session = self.session.lock().await;
        let _ = session
            .disconnect(Disconnect::ByApplication, "", "English")
            .await;
    }
}

impl Drop for SshTunnel {
    fn drop(&mut self) {
        // 阻塞上下文无法 await；仅中止转发任务，SSH 会话由连接池断开时回收
        if !self.closed {
            self.forward_task.abort();
        }
    }
}

/// 认证：按配置走密码或私钥。
async fn authenticate(
    session: &mut Handle<ClientHandler>,
    ssh: &SshConfig,
) -> Result<(), AppError> {
    let auth_ok = match ssh.auth_type {
        SshAuthType::Password => {
            let password = ssh.password.as_deref().unwrap_or_default();
            let auth = session
                .authenticate_password(ssh.username.as_str(), password)
                .await
                .map_err(|error| {
                    AppError::new(
                        error_code::CONNECTION_FAILED,
                        format!("SSH 密码认证失败：{error}"),
                    )
                })?;
            auth.success()
        }
        SshAuthType::PrivateKey => {
            let key_path = ssh.private_key_path.as_deref().unwrap_or_default();
            if key_path.is_empty() {
                return Err(AppError::new(
                    error_code::CONNECTION_FAILED,
                    "SSH 私钥认证需要填写私钥文件路径",
                ));
            }
            let key_pair = load_secret_key(PathBuf::from(key_path), None).map_err(|error| {
                AppError::new(
                    error_code::CONNECTION_FAILED,
                    format!("加载 SSH 私钥失败：{error}"),
                )
            })?;
            let best_hash = session
                .best_supported_rsa_hash()
                .await
                .unwrap_or_default()
                .flatten();
            let auth = session
                .authenticate_publickey(
                    ssh.username.as_str(),
                    PrivateKeyWithHashAlg::new(Arc::new(key_pair), best_hash),
                )
                .await
                .map_err(|error| {
                    AppError::new(
                        error_code::CONNECTION_FAILED,
                        format!("SSH 私钥认证失败：{error}"),
                    )
                })?;
            auth.success()
        }
    };

    if !auth_ok {
        return Err(AppError::new(
            error_code::CONNECTION_FAILED,
            "SSH 认证失败，请检查用户名与认证信息",
        ));
    }
    Ok(())
}

/// 隧道主循环：接受本地连接并逐个转发。
async fn tunnel_loop(
    listener: TcpListener,
    session: Arc<Mutex<Handle<ClientHandler>>>,
    forward_host: String,
    forward_port: u16,
) {
    loop {
        let Ok((stream, originator)) = listener.accept().await else {
            break;
        };
        let session = session.clone();
        let forward_host = forward_host.clone();
        tokio::spawn(async move {
            if let Err(error) =
                forward(stream, originator, session, forward_host, forward_port).await
            {
                tracing::debug!(target: "datadock::ssh", "SSH 隧道转发结束：{error}");
            }
        });
    }
}

/// 把一条本地 TCP 连接通过 SSH direct-tcpip 通道转发到目标。
async fn forward(
    mut stream: TcpStream,
    originator: SocketAddr,
    session: Arc<Mutex<Handle<ClientHandler>>>,
    forward_host: String,
    forward_port: u16,
) -> Result<(), AppError> {
    let mut channel = {
        let guard = session.lock().await;
        guard
            .channel_open_direct_tcpip(
                forward_host,
                u32::from(forward_port),
                originator.ip().to_string(),
                u32::from(originator.port()),
            )
            .await
            .map_err(|error| {
                AppError::new(
                    error_code::CONNECTION_FAILED,
                    format!("SSH 通道建立失败：{error}"),
                )
            })?
    };

    let mut stream_closed = false;
    let mut buffer = vec![0; 65536];
    loop {
        tokio::select! {
            read = stream.read(&mut buffer), if !stream_closed => {
                match read {
                    Ok(0) => {
                        stream_closed = true;
                        let _ = channel.eof().await;
                    }
                    Ok(n) => {
                        channel.data(&buffer[..n]).await.map_err(|error| {
                            AppError::new(
                                error_code::CONNECTION_FAILED,
                                format!("SSH 通道写入失败：{error}"),
                            )
                        })?;
                    }
                    Err(error) => {
                        return Err(AppError::new(
                            error_code::CONNECTION_FAILED,
                            format!("SSH 隧道读取失败：{error}"),
                        ));
                    }
                }
            }
            message = channel.wait() => {
                let Some(message) = message else { break };
                match message {
                    ChannelMsg::Data { ref data } => {
                        stream.write_all(data).await.map_err(|error| {
                            AppError::new(
                                error_code::CONNECTION_FAILED,
                                format!("SSH 隧道写入失败：{error}"),
                            )
                        })?;
                    }
                    ChannelMsg::Eof => {
                        if !stream_closed {
                            let _ = channel.eof().await;
                        }
                        break;
                    }
                    ChannelMsg::WindowAdjusted { .. } => {}
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

/// SSH 服务器密钥校验（当前信任所有主机密钥）。
struct ClientHandler;

impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        // TODO: 首次连接应记录主机密钥指纹，后续校验；当前为可用性优先
        Ok(true)
    }
}

use tokio::io::{AsyncReadExt, AsyncWriteExt};
