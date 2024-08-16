#![warn(missing_docs)]

//! <div class="warning">
//!
//! Note: API is unstable, and may change in `0.x` versions.
//!
//! </div>
//!
//! # As a library
//!
//! As a library, usually you prefer to use [`scheduler`]
//! and [`api`] to download images from gelbooru api.
//!
//! See [`scheduler::Scheduler#example`] for example.
//!
//! # As a binary
//!
//! In addition to the above, you also need [`cli`] to build the command line.
//!
//! See `main.rs` to know how to assemble these modules as a binary.

pub mod api;
pub mod cli;
pub mod scheduler;

pub mod config;
pub mod download;
pub mod hash;
pub mod tool;
