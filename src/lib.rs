//! NQU-ERP 校務系統 library crate。
//!
//! 測試分層：
//! - 單元測試：各模組內 `#[cfg(test)]`，跑 `cargo test --lib`
//! - 系統（Web API）測試：`tests/system_tests.rs`，跑 `cargo test --test system_tests`

pub mod app;
pub mod config;
pub mod db;
pub mod entities;
pub mod errors;
pub mod handlers;
pub mod middleware;

pub use app::{build_router, AppState};
