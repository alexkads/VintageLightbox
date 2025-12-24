//! Async Workers for Background Processing
//!
//! This module provides Relm4 workers for async image loading and processing.

pub mod thumbnail_worker;

pub use thumbnail_worker::{ThumbnailWorker, ThumbnailRequest, ThumbnailResult};
