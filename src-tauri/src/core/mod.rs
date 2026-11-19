//! 核心运行时能力：连接管理、数据源注册（后续阶段）。

pub mod connection_manager;

pub use connection_manager::{close_connection, ConnectionManager, ManagedConnection};

pub mod redis_manager;

pub use redis_manager::{ManagedRedis, RedisEndpoint, RedisManager};

pub mod pg_manager;

pub use pg_manager::{ManagedPg, PgManager};
