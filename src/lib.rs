#![warn(missing_docs)]

//! # Usage
//!
//! <div class="warning">
//!
//! Note: API is unstable, and may change in `0.x` versions.
//!
//! </div>
//!
//! ## As a library
//!
//! As a library, usually you prefer to use [`scheduler`]
//! and [`api`] to download images from gelbooru api.
//!
//! See [`scheduler::Scheduler#example`] for example.
//!
//! ## As a binary
//!
//! <div class="warning">
//!
//! Note: The [`cli`] may become optional feature in the future.
//!
//! </div>
//!
//! In addition to the above, you also need [`cli`] to build the command line.
//!
//! See `main.rs` to know how to assemble these modules as a binary.
//!
//! # Requirements
//!
//! This crate use [`reqwest`] to send `https` requests to booru api,
//! which means [tls] is required.
//!
//! So, this crate enables [`reqwest/default-tls`] through [default features],
//! which has [**this requirement**][reqwest/default-tls:requirement].
//!
//! If you don't want to use the default tls(*I mean, system `openssl` on `linux`*),
//! you can [disable default features][default features],
//! and enable other tls features like [`rustls-tls`] or [`native-tls-vendored`].
//! See following [Feature flags](#feature-flags).
//!
//! # Feature flags
//!
//! This crate re-exports(*same name*) all `tls` features from `reqwest`.
//!
//! See [reqwest#optional-features] for details.
//!
//! [tls]: https://en.wikipedia.org/wiki/Transport_Layer_Security
//! [`reqwest/default-tls`]: https://docs.rs/reqwest/0.12/reqwest/tls/index.html#default-tls
//! [default features]: https://doc.rust-lang.org/stable/cargo/reference/features.html#the-default-feature
//! [reqwest/default-tls:requirement]: https://github.com/seanmonstar/reqwest/tree/v0.12.5?tab=readme-ov-file#requirements
//! [reqwest#optional-features]: https://docs.rs/reqwest/0.12/reqwest/#optional-features
//! [`rustls-tls`]: https://github.com/rustls/rustls
//! [`native-tls-vendored`]: https://docs.rs/openssl/0.10/openssl/#vendored

pub mod api;
pub mod cli;
pub mod scheduler;

pub mod config;
pub mod download;
pub mod hash;
pub mod tool;
