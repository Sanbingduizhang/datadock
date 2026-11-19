//! 本地持久化层（SQLite）。
//!
//! 只负责 DataDock 自身数据；远程数据库访问由数据源适配器负责（阶段 D）。

pub mod data_source_repository;
pub mod favorite_repository;
pub mod history_repository;
pub mod settings_repository;
pub mod workspace_repository;
