//! Utils for parsing and validating the config.
//!
//! Usually, you don't need to use this module directly.
//! [`Cli::parse`] and [`Cli::get_config_from_editor`]
//! will handle the config for you.
//!
//! See [`Config`] for more information.
#[cfg(all(doc, feature = "cli"))]
// we only need these for documentation, or the link will be too long.
use crate::cli::{Cli, Parser};

use std::num::NonZeroU64;
use std::path::PathBuf;

use serde::Deserialize;
pub use validator::Validate;

/// The default config string.
pub const DEFAULT_CONFIG_STR: &str = include_str!("default.toml");

/// The config data struct.
///
/// This struct impl [`Deserialize`] and [`Validate`] to parse and validate the config.
#[non_exhaustive]
#[derive(Debug, Deserialize, Clone, Validate)]
pub struct Config {
    /// The tags to search for.
    ///
    /// This field is validated to ensure it is not empty.
    #[validate(length(min = 1, message = "tags must not be empty"))]
    pub tags: String,
    /// The number of images to download.
    pub num_imgs: NonZeroU64,
    /// The directory to download the images to.
    pub download_dir: PathBuf,
    /// The timeout for the request.
    pub timeout: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_default_config() -> anyhow::Result<()> {
        let config: Config = toml::from_str(DEFAULT_CONFIG_STR)?;
        config.validate()?;
        Ok(())
    }

    #[test]
    fn test_parse_empty_tags() {
        let toml = r#"
            tags = ""
            num_imgs = 1
            download_dir = "test"
            timeout = 10
        "#;
        let config: Config = toml::from_str(toml).unwrap();
        config.validate().expect_err("empty tags should be invalid");
    }
}
