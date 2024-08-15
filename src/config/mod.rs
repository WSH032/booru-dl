use std::num::NonZeroU64;
use std::path::PathBuf;

use serde::Deserialize;
pub use validator::Validate;

pub const DEFAULT_CONFIG_STR: &str = include_str!("default.toml");

#[non_exhaustive]
#[derive(Debug, Deserialize, Clone, Validate)]
pub struct Config {
    #[validate(length(min = 1, message = "tags must not be empty"))]
    pub tags: String,
    pub num_imgs: NonZeroU64,
    pub download_dir: PathBuf,
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
