use std::path::PathBuf;

use clap::builder::{PathBufValueParser, TypedValueParser};
use clap::error::ErrorKind;
use clap::Command;
pub use clap::{CommandFactory, Parser};
use dialoguer::Editor;

use crate::config::{Config, Validate, DEFAULT_CONFIG_STR};

const EDITOR_EXTENSION: &str = ".toml";

#[non_exhaustive]
#[derive(Parser)]
#[command(version, about)]
pub struct Cli {
    /// The config file to use.
    /// if `None`, will automatically open an editor to create one temporarily.
    #[arg(value_name = "PATH")]
    #[arg(value_parser = PathBufValueParser::new().try_map(Self::parse_config_from_filepath))]
    pub config: Option<Config>,
}

impl Cli {
    #[inline]
    fn parse_config_from_filepath(path: PathBuf) -> anyhow::Result<Config> {
        let config = std::fs::read_to_string(path)?;
        let config = toml::from_str::<Config>(&config)?;
        config.validate()?;
        Ok(config)
    }

    /// We open an editor to ask the user to write a config file.
    pub fn get_config_from_editor(cmd: &mut Command) -> Result<Config, clap::Error> {
        let config: Option<String> = match Editor::new()
            .extension(EDITOR_EXTENSION)
            .edit(DEFAULT_CONFIG_STR)
        {
            Ok(config) => config,
            Err(err) => {
                return Err(cmd.error(ErrorKind::Io, err));
            }
        };
        let config = match config {
            Some(config) => config,
            None => {
                return Err(cmd.error(
                    ErrorKind::ValueValidation,
                    "Empty content. Maybe you forget to save in the editor?",
                ))
            }
        };
        let config = match toml::from_str::<Config>(&config) {
            Ok(config) => config,
            Err(err) => return Err(cmd.error(ErrorKind::ValueValidation, err)),
        };

        match config.validate() {
            Ok(_) => Ok(config),
            Err(err) => Err(cmd.error(ErrorKind::ValueValidation, err)),
        }
    }
}
