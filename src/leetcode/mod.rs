//! LeetCode "古法时代" — Terminal-native LeetCode practice environment.
//!
//! This entire module is gated behind the `leetcode` Cargo feature.
//! To build without it: `cargo build --no-default-features`

pub mod models;
pub mod api;
pub mod auth;
pub mod cache;
pub mod panel;

pub use models::*;
pub use panel::LeetCodePanel;
