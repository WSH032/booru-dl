use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Weak;

use reqwest::{Client, IntoUrl};
use thiserror::Error;
use tokio::fs::{create_dir_all, File};
use tokio::io::{AsyncWriteExt, BufWriter};

#[non_exhaustive]
#[derive(Error, Debug)]
pub enum DownloadError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error("There is no content to download")]
    ZeroContentLength,
    #[error("Failed to allocate file size: {0}")]
    FileAllocationFailed(std::io::Error),
}

pub struct DownloadFutureBuilder<U, P>
where
    U: IntoUrl,
    P: AsRef<Path>,
{
    client: Client,
    url: U,
    file_path: P,
    data_cursor: Option<Weak<AtomicUsize>>,
}

impl<U, P> DownloadFutureBuilder<U, P>
where
    U: IntoUrl,
    P: AsRef<Path>,
{
    fn new(client: Client, url: U, file_path: P) -> Self {
        Self {
            client,
            url,
            file_path,
            data_cursor: None,
        }
    }

    pub fn add_data_cursor(mut self, speed_cursor: Weak<AtomicUsize>) -> Self {
        self.data_cursor = Some(speed_cursor);
        self
    }

    pub fn build(self) -> impl Future<Output = Result<P, DownloadError>> {
        let Self {
            client,
            url,
            file_path,
            data_cursor,
        } = self;

        async move {
            let mut response = client.get(url).send().await?.error_for_status()?;
            let mut file_buf = BufWriter::new(File::create(&file_path).await?);

            // pre-allocate file size
            if let Some(content_length) = response.content_length() {
                if content_length == 0 {
                    return Err(DownloadError::ZeroContentLength);
                }

                file_buf
                    .get_ref()
                    .set_len(content_length)
                    .await
                    // if disk is full, this will fail
                    .map_err(DownloadError::FileAllocationFailed)?;
            }

            while let Some(mut chunk) = response.chunk().await? {
                let chunk_len: usize = chunk.len();
                // may be we should check if occurr `FileAllocationFailed` error
                file_buf.write_all_buf(&mut chunk).await?;

                if let Some(ref data_cursor) = data_cursor {
                    if let Some(data_cursor) = data_cursor.upgrade() {
                        let previous_value = data_cursor.fetch_add(chunk_len, Ordering::Release);
                        // or unstable `strict_add`
                        if previous_value.checked_add(chunk_len).is_none() {
                            panic!("Data cursor overflow");
                        }
                    }
                }
            }

            file_buf.flush().await?;
            Ok::<P, DownloadError>(file_path)
        }
    }
}

pub struct Downloader {
    client: Client,
    download_dir: PathBuf,
}

impl Downloader {
    pub fn session(client: Client, download_dir: impl Into<PathBuf>) -> Self {
        let download_dir = download_dir.into();
        Self {
            client,
            download_dir,
        }
    }

    #[inline]
    pub async fn ensure(self) -> std::io::Result<Self> {
        create_dir_all(&self.download_dir).await?;
        Ok(self)
    }

    #[inline]
    pub fn future<U>(&self, url: U, filename: impl AsRef<Path>) -> DownloadFutureBuilder<U, PathBuf>
    where
        U: IntoUrl,
    {
        DownloadFutureBuilder::new(self.client.clone(), url, self.download_dir.join(filename))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    const URL: &str = "https://httpbin.org/image/png";
    const FILE_NAME: &str = ".test.png";

    #[tokio::test]
    async fn test_download() {
        let temp_dir = tempfile::tempdir().unwrap();

        let downloader = Downloader::session(Client::new(), temp_dir.path())
            .ensure()
            .await
            .unwrap();

        let future = downloader.future(URL, FILE_NAME).build();
        future.await.expect("Download failed");

        temp_dir.close().unwrap();
    }

    #[tokio::test]
    async fn test_download_task_with_cursor() {
        let temp_dir = tempfile::tempdir().unwrap();

        let downloader = Downloader::session(Client::new(), temp_dir.path())
            .ensure()
            .await
            .unwrap();

        let data_cursor = Arc::new(AtomicUsize::new(0));
        let future = downloader
            .future(URL, FILE_NAME)
            .add_data_cursor(Arc::downgrade(&data_cursor))
            .build();
        tokio::spawn(future)
            .await
            .expect("Task failed")
            .expect("Download failed");
        // data cursor should be updated
        assert_ne!(data_cursor.load(Ordering::Acquire), 0);

        let future = downloader
            .future(URL, FILE_NAME)
            .add_data_cursor(Arc::downgrade(&data_cursor))
            .build();
        // test weak reference
        drop(data_cursor);
        tokio::spawn(future)
            .await
            .expect("Data cursor weak reference failed")
            .expect("Download failed");

        temp_dir.close().unwrap();
    }
}
