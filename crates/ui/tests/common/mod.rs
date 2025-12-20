//! Shared test utilities for egui_kittest E2E tests

pub use egui_kittest::{Harness, kittest::{Queryable, Node}};

/// Standard test harness size for consistent snapshots
pub const TEST_SIZE: [u32; 2] = [400, 200];
