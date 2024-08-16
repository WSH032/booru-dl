//! A core module to download images from the API data.
//!
//! See [`Scheduler`] for more information.
//!
//! Following is the low-level module wrapped by this module:
//! - [`crate::download`]
//! - [`crate::hash`]
//! - [`crate::tool`]

use std::future::Future;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use indicatif::{ProgressBar, ProgressFinish, ProgressStyle, WeakProgressBar};
use reqwest::Client;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::api::data::field::Post;
use crate::download::{DownloadError, Downloader};
use crate::hash::hash_file;
use crate::tool::NUM_CPUS;

type ApiPostData = Vec<Post>;

const PB_FINISH_MODE: ProgressFinish = ProgressFinish::Abandon;
const PB_TICK_SECS: u64 = 1;
/// The time interval for updating the download speed.
const SPEED_UPDATE_SECS: u64 = 1;

/// The result of a single download task.
enum SingleDownloadResult {
    /// The file was downloaded successfully.
    Done,
    /// The file already existed.
    Existed,
}

/// current download number status
struct DownloadStatus {
    /// the number of files that have been downloaded successfully
    done: u64,
    // the number of files that already,which means no need to download
    existed: u64,
    // the number of files that failed to download
    failed: u64,
}

/** The scheduler to download images from the API data.

- This struct will wrap a [`Downloader`] to download images from the `api_post_data` API data to the `download_dir`.
    Also, it will write the [`tags`] to a tag file with the same name as the image file.

    *If the file already exists, the download and tag writing will be skipped.*

- The number of concurrent downloads will be limited to the number of CPUs available.

- A process bar will be displayed to show the download status and speed when downloading images.

[`tags`]: crate::api::data::field::Post::tags

# Example
```no_run
use reqwest::Client;
use std::path::PathBuf;
use booru_dl::api::BatchGetter;
use booru_dl::scheduler::Scheduler;

#[tokio::main]
async fn main() {
    let client = Client::new();

    let getter = BatchGetter::build(&client, "cat", 10).unwrap();
    let api_post_data = getter.run().await.expect("Failed to get data from API");

    let scheduler = Scheduler::build(client, "download_dir", api_post_data).await.unwrap();
    scheduler.launch().await;
}
```
*/
pub struct Scheduler {
    downloader: Downloader,
    // TODO, XXX: remove the duplicated `download_dir` field,
    // get it from `downloader` field
    download_dir: PathBuf,
    api_post_data: ApiPostData,
}

impl Scheduler {
    /// Create a new scheduler.
    ///
    /// Usually, you prefer to use [`crate::api`] to get the `api_post_data`.
    ///
    /// # Errors
    ///
    /// If the `download_dir` cannot be created, an error will be returned.
    pub async fn build(
        client: Client,
        download_dir: impl Into<PathBuf>,
        api_post_data: impl Into<ApiPostData>,
    ) -> std::io::Result<Self> {
        let download_dir = download_dir.into();
        let downloader = Downloader::session(client, download_dir.clone())
            .ensure()
            .await?;
        Ok(Scheduler {
            downloader,
            download_dir,
            api_post_data: api_post_data.into(),
        })
    }

    /// Check if the file already exists by comparing the MD5 hash.
    /// If the file does not exist, return `false`.
    ///
    /// Consume max to 2MB memory when hashing file.
    #[inline]
    async fn check_file_existed(
        filepath: impl AsRef<Path>,
        hashed_value: impl AsRef<str>,
    ) -> std::io::Result<bool> {
        type Hasher = md5::Md5;

        hash_file::<Hasher>(filepath)
            .await
            .map(|file_md5| file_md5 == hashed_value.as_ref())
            .or_else(|err| {
                if err.kind() == ErrorKind::NotFound {
                    Ok(false)
                } else {
                    Err(err)
                }
            })
    }

    /// Return the formated download status message
    #[inline]
    fn pb_msg(status: &DownloadStatus) -> String {
        let DownloadStatus {
            done,
            existed,
            failed,
        } = status;
        format!("[done:{done}\texisted:{existed}\tfailed:{failed}]")
    }

    /// Return the formated speed status message in bytes
    #[inline]
    fn pb_prefix(speed: u64) -> String {
        format!("[{}/S]", indicatif::HumanBytes(speed))
    }

    /// Build a process bar with a specific length and custom style.
    #[inline]
    fn build_process_bar(len: u64) -> ProgressBar {
        // see: https://docs.rs/indicatif/latest/indicatif/#templates
        const PROCESS_CHARS: &str = "#>-";
        // `prefix` for speed, `msg` for download status
        const TEMPLATE: &str = "[{elapsed_precise}] {prefix} [{wide_bar:.cyan/blue}] {msg} {human_pos}/{human_len} ({eta})";

        let style = ProgressStyle::with_template(TEMPLATE)
            .unwrap()
            .progress_chars(PROCESS_CHARS);

        ProgressBar::new(len)
            .with_style(style)
            .with_message(Self::pb_msg(&DownloadStatus {
                done: 0,
                existed: 0,
                failed: 0,
            }))
            .with_prefix(Self::pb_prefix(0))
            .with_finish(PB_FINISH_MODE)
    }

    /// Download a single file.
    ///
    /// - `semaphore`: limit the number of concurrent downloads.
    /// - `filepath`: the path to save the file.
    /// - `md5`: the MD5 hash to compare for checking if the file already exists.
    /// - `tags`: the tags to write to the tag file.
    /// - `download_future`: the future to download the file,
    ///     created by [`crate::download::DownloadFutureBuilder::build`].
    #[inline]
    async fn single_download(
        semaphore: Arc<Semaphore>,
        filepath: PathBuf,
        md5: String,
        tags: String,
        download_future: impl Future<Output = Result<PathBuf, DownloadError>>,
    ) -> anyhow::Result<SingleDownloadResult> {
        // we must use semaphore to limit the number of concurrent downloads,
        // because `check_file_existed` will hold a file handle, and consume 2MB memory
        let _permit = semaphore
            .acquire()
            .await
            .expect("semaphore was closed too early");

        // check if the file existed
        if Self::check_file_existed(&filepath, md5)
            .await
            .with_context(|| {
                format!(
                    "Failed to check if file is already existed: {}",
                    filepath.display()
                )
            })?
        {
            return Ok(SingleDownloadResult::Existed);
        }

        // download the file
        download_future
            .await
            .with_context(|| format!("Failed to download: {}", filepath.display()))?;

        // write tags to file
        let tag_file_path = filepath.with_extension("txt");
        tokio::fs::write(&tag_file_path, tags.replace(' ', ", ")) // "a b" -> "a, b"
            .await
            .with_context(|| format!("Failed to write tags: {}", tag_file_path.display()))?;

        // success = download + write tags
        Ok(SingleDownloadResult::Done)
    }

    /// Update the download speed prefix of `process_bar` every `SPEED_UPDATE_SECS` seconds forever,
    /// until `process_bar` bar was dropped.
    ///
    /// The `speed_cursor` will be swapped(Ordering::Acquire) to 0 after each update.
    #[inline]
    async fn update_speed(process_bar: WeakProgressBar, speed_cursor: Arc<AtomicUsize>) {
        const ORDER: Ordering = Ordering::Acquire;

        let mut interval = tokio::time::interval(Duration::from_secs(SPEED_UPDATE_SECS));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        interval.tick().await; // The first tick completes immediately.
        speed_cursor.swap(0, ORDER); // ignore previous data

        if process_bar.upgrade().is_none() {
            // process bar was dropped, so we exit
            return;
        }

        loop {
            // Calculate the average speed over time
            let last_instant = tokio::time::Instant::now();
            interval.tick().await;
            let elapsed: u64 = last_instant
                .elapsed()
                .as_millis()
                .try_into()
                .expect("elapsed time is too long to `u64`");
            let current_size: u64 = speed_cursor
                .swap(0, ORDER)
                .try_into()
                .expect("speed data is too large to `u64`");
            // multiply by 1000 because `elapsed` is in milliseconds
            let speed = (current_size * 1000) / elapsed;

            if let Some(process_bar) = process_bar.upgrade() {
                process_bar.set_prefix(Self::pb_prefix(speed));
            } else {
                // process bar was dropped, so we exit
                return;
            }
        }
    }

    /// Update the download status message of `process_bar` until all tasks of `download_join_set` are completed.
    ///
    /// # Panics
    ///
    /// If a task panic, the panic will be resumed when `join` the task.
    #[inline]
    async fn update_status(
        process_bar: ProgressBar,
        mut download_join_set: JoinSet<anyhow::Result<SingleDownloadResult>>,
    ) {
        let mut status = DownloadStatus {
            done: 0,
            existed: 0,
            failed: 0,
        };
        // Check result and update process bar
        while let Some(task_result) = download_join_set.join_next().await {
            let task_result = match task_result {
                Ok(task_result) => task_result,
                Err(join_error) => {
                    if let Ok(reason) = join_error.try_into_panic() {
                        // Expect unknown error, so we just resume the panic
                        std::panic::resume_unwind(reason)
                    }
                    // task was cancelled if not panic, so we do nothing here
                    panic!("Unexpected task cancelled");
                }
            };

            match task_result {
                Ok(SingleDownloadResult::Done) => {
                    status.done += 1;
                }
                Ok(SingleDownloadResult::Existed) => {
                    status.existed += 1;
                }
                // why `suspend`: https://docs.rs/indicatif/0.17.8/indicatif/struct.ProgressBar.html#method.suspend
                // why `{:#}`: https://docs.rs/anyhow/1.0.86/anyhow/struct.Error.html#display-representations
                Err(err) => {
                    status.failed += 1;
                    process_bar.suspend(|| eprintln!("{:#}", err));
                }
            }
            process_bar.set_message(Self::pb_msg(&status));
            process_bar.inc(1);
        }
        process_bar.finish();
    }

    /// Launch the scheduler and download all images from api data to the download directory.
    /// A process bar will be displayed to show the download status adn speed.
    ///
    /// # Panics
    ///
    /// If one of the download tasks panic, the panic will be resumed when `join` the task.
    ///
    /// Usually, this will **not happen**. If you encounter this situation, please report it as a bug.
    pub async fn launch(self) {
        let Self {
            downloader,
            download_dir,
            api_post_data,
        } = self;

        let process_bar = Self::build_process_bar(api_post_data.len().try_into().unwrap());
        process_bar.enable_steady_tick(Duration::from_secs(PB_TICK_SECS));

        let speed_cursor = Arc::new(AtomicUsize::new(0));
        let semaphore = Arc::new(Semaphore::new(NUM_CPUS.get()));
        let mut download_join_set = JoinSet::new();
        // Arrange tasks
        process_bar.suspend(|| eprintln!("Arranging tasks..."));
        for data in api_post_data {
            let Post {
                md5,
                file_url,
                filename,
                tags,
                ..
            } = data;

            let download_future = downloader
                .future(file_url, &filename)
                .add_data_cursor(Arc::downgrade(&speed_cursor))
                .build();
            download_join_set.spawn(Self::single_download(
                semaphore.clone(),
                download_dir.join(filename),
                md5,
                tags,
                download_future,
            ));
        }

        process_bar.suspend(|| eprintln!("Arranging tasks done"));

        // NOTE: We update the download speed only after arranging all tasks,
        // otherwise there may be a situation where the download progress remains unchanged while the speed keeps changing
        let update_speed = Self::update_speed(process_bar.downgrade(), speed_cursor);
        let update_status = Self::update_status(process_bar, download_join_set);

        // Note: `join!` `update_speed` may wait an additional `SPEED_UPDATE_SECS` seconds,
        // use `select!` if you want to avoid this.
        tokio::join!(update_speed, update_status);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::LazyLock;

    use tempfile::TempDir;

    use crate::api::PostInner;

    const MD5: &str = "9e107d9d372bb6826bd81d3542a419d6";
    const CONTENT: &str = "The quick brown fox jumps over the lazy dog";
    const EXT: &str = "jpg";
    const ID: u64 = 1234;
    const FILE_URL: &str = "https://httpbin.org/image/png";
    static CONTENT_FILE_NAME: LazyLock<String> = LazyLock::new(|| format!("{ID}.{EXT}"));
    static EMPTY_FILE_NAME: LazyLock<String> = LazyLock::new(|| format!("empty.{EXT}"));

    fn default_post_data() -> Post {
        PostInner {
            id: ID,
            tags: String::from("foo bar"),
            md5: String::from(MD5),
            file_url: String::from(FILE_URL),
            image: PathBuf::from(format!("{MD5}.{EXT}")),
        }
        .into()
    }

    struct DefaultScheduler {
        inner: Scheduler,
        temp_dir: TempDir,
    }

    impl DefaultScheduler {
        async fn new() -> Self {
            let temp_dir = TempDir::new().unwrap();
            let temp_dir_path = temp_dir.path();
            std::fs::write(temp_dir_path.join(&(*CONTENT_FILE_NAME)), CONTENT).unwrap();
            std::fs::write(temp_dir_path.join(&(*EMPTY_FILE_NAME)), b"").unwrap();

            let inner = Scheduler::build(
                reqwest::Client::new(),
                temp_dir_path,
                Vec::from([default_post_data()]),
            )
            .await
            .unwrap();

            Self { inner, temp_dir }
        }
    }

    #[tokio::test]
    async fn test_check_file_existed() {
        let default_scheduler = DefaultScheduler::new().await;
        let temp_dir_path = default_scheduler.temp_dir.path();

        let content_file_path = temp_dir_path.join(&(*CONTENT_FILE_NAME));
        let no_existed_file_path = temp_dir_path.join("no_exist_file");

        let is_existed = Scheduler::check_file_existed(&content_file_path, MD5)
            .await
            .unwrap();
        assert!(is_existed);

        let is_existed = Scheduler::check_file_existed(&content_file_path, "wrong md5")
            .await
            .unwrap();
        assert!(!is_existed);

        let is_existed = Scheduler::check_file_existed(&no_existed_file_path, "whatever md5")
            .await
            .unwrap();
        assert!(!is_existed);
    }

    #[tokio::test]
    async fn test_launch() {
        let default_scheduler = DefaultScheduler::new().await;
        default_scheduler.inner.launch().await
    }
}
