use std::process::ExitCode;
use std::time::Duration;

use anyhow::Context;
use indicatif::{ProgressBar, ProgressFinish, ProgressStyle};
use reqwest::Client;
use tokio::runtime::Runtime;
use tokio::signal;

use booru_dl::api::data::BatchGetter;
use booru_dl::cli::{Cli, CommandFactory, Parser};
use booru_dl::config::Config;
use booru_dl::scheduler::Scheduler;

const SPINNER_FINISH_MODE: ProgressFinish = ProgressFinish::AndClear;
const SPINNER_TICK_SECS: f32 = 0.1;

#[inline]
fn build_spinner() -> ProgressBar {
    ProgressBar::new_spinner()
        .with_finish(SPINNER_FINISH_MODE)
        .with_style(
            ProgressStyle::with_template("{spinner:.blue} {msg}")
                .unwrap()
                // For more spinners check out the cli-spinners project:
                // https://github.com/sindresorhus/cli-spinners/blob/master/spinners.json
                // NOTE: use `ascii` only, because cmd/powershell maybe not support unicode.
                .tick_strings(&[".  ", ".. ", "...", " ..", "  .", "   "]),
        )
}

#[inline]
fn build_client(timeout: u64) -> reqwest::Result<Client> {
    let client_builder = Client::builder();
    let client_builder = if timeout > 0 {
        client_builder.timeout(Duration::from_secs(timeout))
    } else {
        client_builder
    };
    client_builder.build()
}

#[inline]
async fn async_main(config: Config) -> anyhow::Result<()> {
    let client = build_client(config.timeout).context("failed to build reqwest client")?;

    // Because `config` and `cli` modules have already validated the config, we can safely unwrap here.
    let getter = BatchGetter::build(&client, &config.tags, config.num_imgs.get())
        .expect("wrong config parser, please raise an issue on GitHub");

    let spinner = build_spinner();
    spinner.set_message("Fetching image data from Gelbooru API...");
    spinner.enable_steady_tick(Duration::from_secs_f32(SPINNER_TICK_SECS));
    let api_post_data = getter.run().await.context("failed to get data from API")?;
    spinner.finish_with_message("Image data fetched successfully!");

    // HACK: This is not considered an error, so we just return Ok(()).
    if api_post_data.is_empty() {
        println!(
            "There is no image found with the given tags: {}",
            config.tags
        );
        return Ok(());
    }

    let scheduler = Scheduler::build(client, config.download_dir, api_post_data)
        .await
        .context("Unable to ensure the existence of the download directory")?;

    scheduler.launch().await;

    Ok(())
}

fn main() -> anyhow::Result<ExitCode> {
    // here, if parse fails, the program will be `abort`ed, and no `Drop` will be called,
    // but it's okay, because we don't need to clean up anything.
    let cli = Cli::parse();

    let config = match cli.config {
        Some(config) => config,
        None => match Cli::get_config_from_editor(&mut Cli::command()) {
            Ok(config) => config,
            // if we can't get the config from the editor, we drop the whole program.
            Err(err) => {
                let _ = err.print();
                return Ok(ExitCode::from(u8::try_from(err.exit_code()).unwrap()));
            }
        },
    };

    let runtime = Runtime::new().context("failed to build tokio runtime")?;
    runtime.block_on(async {
        tokio::select! {
            result = async_main(config) => {result},
            result = signal::ctrl_c() => {
                result.expect("failed to listen for ctrl-c signal");
                println!("Ctrl-C received, exiting...");
                Ok(())
            },
        }
    })?;

    Ok(ExitCode::SUCCESS)
}
