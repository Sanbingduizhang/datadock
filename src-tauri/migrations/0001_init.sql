-- 0001_init.sql
-- DataDock 本地 SQLite 初始结构，对应 docs/04-数据库设计.md 第 3~8 章。
-- 约定：
--   1. 业务主键统一 TEXT（UUID）。
--   2. 时间统一 UTC Unix 毫秒（INTEGER）。
--   3. 连接密码不落库，data_sources.credential_key 仅为系统凭据引用。

CREATE TABLE IF NOT EXISTS data_sources (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    source_type TEXT NOT NULL,
    host TEXT NOT NULL,
    port INTEGER NOT NULL,
    username TEXT NOT NULL,
    credential_key TEXT,
    default_database TEXT,
    options_json TEXT,
    remark TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_data_sources_type
ON data_sources(source_type);

CREATE INDEX IF NOT EXISTS idx_data_sources_sort
ON data_sources(sort_order);

CREATE TABLE IF NOT EXISTS query_history (
    id TEXT PRIMARY KEY,
    connection_id TEXT,
    source_type TEXT NOT NULL,
    database_name TEXT,
    sql_text TEXT NOT NULL,
    status TEXT NOT NULL,
    returned_rows INTEGER NOT NULL DEFAULT 0,
    affected_rows INTEGER,
    elapsed_ms INTEGER NOT NULL DEFAULT 0,
    error_message TEXT,
    started_at INTEGER NOT NULL,
    finished_at INTEGER
);

CREATE INDEX IF NOT EXISTS idx_query_history_connection
ON query_history(connection_id);

CREATE INDEX IF NOT EXISTS idx_query_history_started
ON query_history(started_at DESC);

CREATE INDEX IF NOT EXISTS idx_query_history_status
ON query_history(status);

CREATE TABLE IF NOT EXISTS sql_favorites (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    connection_id TEXT,
    database_name TEXT,
    sql_text TEXT NOT NULL,
    category TEXT,
    remark TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_sql_favorites_connection
ON sql_favorites(connection_id);

CREATE INDEX IF NOT EXISTS idx_sql_favorites_category
ON sql_favorites(category);

CREATE INDEX IF NOT EXISTS idx_sql_favorites_sort
ON sql_favorites(sort_order);

CREATE TABLE IF NOT EXISTS app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    value_type TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS workspace_state (
    id TEXT PRIMARY KEY,
    state_key TEXT NOT NULL UNIQUE,
    state_json TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);
