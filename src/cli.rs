//! A core module for command line interface.
//!
//! See [`Cli`] for more information.

use std::path::PathBuf;

use clap::builder::{PathBufValueParser, TypedValueParser};
use clap::error::ErrorKind;
use clap::Command;
pub use clap::{CommandFactory, Parser};
use dialoguer::Editor;

use crate::config::{Config, Validate, DEFAULT_CONFIG_STR};

const EDITOR_EXTENSION: &str = ".toml";

/// [`clap`] command line interface.
///
/// The [`Self::parse`] trait and [`Self::get_config_from_editor`]
/// will use [`toml`] to parse the config file,
/// then use [`Config::validate`] to validate the config.
///
/// You need check out the [`Self`] source code to figure out what [`Self`] do when parsing the config.
///
/// # Example
///
/// ```no_run
/// use booru_dl::cli::{Cli, Parser as _};
///
/// let cli = Cli::parse();
/// ```
#[non_exhaustive]
#[derive(Parser)]
#[command(version, about)]
pub struct Cli {
    /// The config file to use.
    ///
    /// If `None`, you can use [`Self::get_config_from_editor`]
    /// to open an editor to ask the user to write a temp config file.
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

    /// Open an editor to ask the user to write a config file.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use booru_dl::cli::{Cli, CommandFactory as _};
    ///
    /// let config = Cli::get_config_from_editor(&mut Cli::command())?;
    ///
    /// Ok::<(), clap::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// If the editor fails to write, or the content is empty, or the content is invalid,
    /// it will return an error.
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
