//! A core module for interacting with the Gelbooru API.
//!
//! Usually, you prefer to use the [`BatchGetter`] struct to get the [`data`] from the Gelbooru API.

use std::path::PathBuf;
use std::sync::LazyLock;

use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};

/// The URLs for the Gelbooru API.
pub mod url {
    use super::*;

    /// The base URL of the Gelbooru.
    pub const BASE_URL: &str = "https://gelbooru.com/index.php";

    /// The Api URL of the Gelbooru, which can be used to query gelbooru's database.
    pub static API_URL: LazyLock<Url> = LazyLock::new(|| {
        // see: https://gelbooru.com/index.php?page=wiki&s=view&id=18780
        Url::parse_with_params(
            BASE_URL,
            &[
                ("page", "dapi"),
                ("s", "post"),
                ("q", "index"),
                ("json", "1"),
            ],
        )
        .unwrap()
    });

    /// The Post URL of the Gelbooru, which can be used to display the images.
    pub static POST_URL: LazyLock<Url> = LazyLock::new(|| {
        // see: https://gelbooru.com/index.php?page=wiki&s=view&id=18780
        Url::parse_with_params(BASE_URL, &[("page", "post"), ("s", "list"), ("q", "index")])
            .unwrap()
    });
}

/// This struct is used to auto initialize the `filename` field for the `Post` struct.
#[derive(Deserialize)]
pub(crate) struct PostInner {
    pub(crate) id: u64,
    pub(crate) md5: String,
    pub(crate) file_url: String,
    pub(crate) tags: String,
    pub(crate) image: PathBuf,
}

impl From<PostInner> for data::field::Post {
    /// `filename` equals to `id` with `image`'s extension.
    /// e.g. `id = 12345`, `image = "test.jpg"`, then `filename = "12345.jpg"`.
    fn from(value: PostInner) -> Self {
        use crate::tool::SetFileStem;

        // make sure only the filename is retained
        let mut filename: PathBuf = value.image.file_name().unwrap().into();
        filename.set_file_stem(value.id.to_string());

        Self {
            id: value.id,
            md5: value.md5,
            file_url: value.file_url,
            tags: value.tags,
            image: value.image,
            filename,
        }
    }
}

/// The data structure for the JSON response from the Gelbooru API.
pub mod data {
    use super::*;

    /// The fields of the [`Json`] response.
    pub mod field {
        use super::*;

        /// The attributes field of the JSON response.
        #[non_exhaustive]
        #[derive(Debug, Deserialize, Serialize)]
        pub struct Attributes {
            /// The number of images in this response. Range: `0..=100`.
            pub limit: u64,
            /// The current index of the first image in this response.
            pub offset: u64,
            /// The total number of images in the gelbooru.
            pub count: u64,
        }

        /// The post field of the JSON response.
        #[non_exhaustive]
        #[derive(Debug, Deserialize, Serialize)]
        #[serde(from = "PostInner")]
        pub struct Post {
            /// The ID of the image.
            pub id: u64,
            /// The MD5 hash of the image.
            pub md5: String,
            /// The URL of the image, which can be used to download the image.
            pub file_url: String,
            /// The tags of the image. Note: these tags are marked by gelbooru.
            pub tags: String,
            /// The original file name of the image.
            pub image: PathBuf,
            /// The filename of the image, which is the same as `id` with the extension of `image`.
            /// We will use this field to save the image.
            pub(crate) filename: PathBuf,
        }
    }

    /// The JSON structure response from the Gelbooru API.
    #[non_exhaustive]
    #[derive(Debug, Deserialize, Serialize)]
    pub struct Json {
        #[serde(rename = "@attributes")]
        /// The attributes of the JSON response.
        pub attributes: field::Attributes,
        /// The posts of the JSON response.
        /// if `attributes.count` is `0`, or `attributes.pid` is out of range,
        /// this field will be `None`.
        pub post: Option<Vec<field::Post>>,
    }
}

/// A Consuming-Builders style function to get the data from the Gelbooru API.
///
/// # Example
///
/// ```rust
/// use reqwest::Client;
/// use booru_dl::api::Getter;
///
/// #[tokio::main]
/// async fn main() -> reqwest::Result<()> {
///     let client = Client::new();
///     let tags = "cat";
///     let limit = 10;
///     let pid = 0;
///
///     let data = Getter::build(&client, &tags, limit, pid)
///         .expect("illegal arguments")
///         .run()
///         .await?;
///
///     Ok(())
/// }
/// ```
pub struct Getter<'a> {
    client: &'a Client,
    tags: &'a str,
    limit: u64,
    pid: u64,
}

impl Getter<'_> {
    /// See <https://gelbooru.com/index.php?page=wiki&s=view&id=18780> for arguments.
    ///
    /// # Errors
    ///
    /// If `tags` is empty, or `limit` is not in the range `1..=100`, this function will return an error.
    pub fn build<'a>(
        client: &'a Client,
        tags: &'a str,
        limit: u64,
        pid: u64,
    ) -> anyhow::Result<Getter<'a>> {
        if tags.is_empty() {
            return Err(anyhow::anyhow!("Tags cannot be empty"));
        }
        // This is gelbooru's limit.
        // see: https://gelbooru.com/index.php?page=wiki&s=view&id=18780
        if !matches!(limit, 1..=100) {
            return Err(anyhow::anyhow!("Limit can only be between 1 and 100"));
        }
        Ok(Getter {
            client,
            tags,
            limit,
            pid,
        })
    }

    /// Send the request to the Gelbooru API and get the JSON response.
    ///
    /// # Errors
    ///
    /// If the request fails, this function will return an error.
    ///
    /// <div class="warning">
    ///
    /// If `limit * pid > 20_000`, the API will return an error.
    ///
    /// See: <https://gelbooru.com/index.php?page=forum&s=view&id=1549>
    ///
    /// </div>
    pub async fn run(self) -> reqwest::Result<data::Json> {
        let mut target_url = url::API_URL.clone();
        target_url.query_pairs_mut().extend_pairs([
            ("tags", self.tags),
            ("limit", &self.limit.to_string()),
            ("pid", &self.pid.to_string()),
        ]);
        self.client.get(target_url).send().await?.json().await
    }
}

/// This helper wraps the [`Getter`] struct and automatically polls the API until the number of images is reached.
///
/// # Example
///
/// See [`Getter#example`] for example usage.
pub struct BatchGetter<'a> {
    client: &'a Client,
    tags: &'a str,
    num_imgs: u64,
}

impl BatchGetter<'_> {
    /// See [`Getter::build`] for arguments.
    ///
    /// # Errors
    ///
    /// If `tags` is empty, or `num_imgs` is 0, this function will return an error.
    pub fn build<'a>(
        client: &'a Client,
        tags: &'a str,
        num_imgs: u64,
    ) -> anyhow::Result<BatchGetter<'a>> {
        if tags.is_empty() {
            return Err(anyhow::anyhow!("Tags cannot be empty"));
        }
        if num_imgs == 0 {
            return Err(anyhow::anyhow!("Number of images cannot be 0"));
        }
        Ok(BatchGetter {
            client,
            tags,
            num_imgs,
        })
    }

    /// Wraps the [`Getter`] struct and automatically polls the API until the number of images is reached.
    ///
    /// If none of the images are found, this function will return an zero capacity vector.
    ///
    /// # Errors
    ///
    /// If the request fails, this function will return an error.
    ///
    /// <div class="warning">
    ///
    /// If `num_imgs > 20_000`, the API will return an error.
    ///
    /// See: <https://gelbooru.com/index.php?page=forum&s=view&id=1549>
    ///
    /// </div>
    pub async fn run(self) -> reqwest::Result<Vec<data::field::Post>> {
        const LIMIT: u64 = 100;

        let Self {
            client,
            tags,
            num_imgs,
        } = self;

        let mut current_pid = 0;
        let data = Getter::build(client, tags, LIMIT, current_pid)
            .unwrap()
            .run()
            .await?;

        let mut post_vec = match data.post {
            Some(post) => post,
            None => return Ok(Vec::with_capacity(0)),
        };
        let total_num: usize = std::cmp::min(num_imgs, data.attributes.count)
            .try_into()
            .expect("total number is too large to convert to `usize`");
        // if `total_num` is 0, then `data.attributes.count` is 0,
        // so `data.post` should be `None` and return early.
        debug_assert_ne!(total_num, 0);

        while post_vec.len() < total_num {
            current_pid += 1;
            let current_post_vec = Getter::build(client, tags, LIMIT, current_pid)
                .unwrap()
                .run()
                .await?
                .post
                .expect(
                    "if `post_vec` is shorter than `total_num`, \
                    then `post` should not be `None`",
                );
            post_vec.extend(current_post_vec);
        }
        post_vec.truncate(total_num);

        Ok(post_vec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_illegal_args() {
        let client = Client::new();

        let resp = Getter::build(&client, "", 100, 0);
        assert!(resp.is_err());

        let resp = Getter::build(&client, "cat", 0, 0);
        assert!(resp.is_err());
    }

    #[tokio::test]
    async fn test_get_api_data() -> reqwest::Result<()> {
        let client = Client::new();
        let tag = "cat";
        let limit = 10;

        let resp = Getter::build(&client, tag, limit, 0).unwrap().run().await?;
        assert_eq!(resp.attributes.limit, limit);
        assert!(resp
            .post
            .expect("if `attributes.limit` is correct, then `post` shouldn't be `None`")[0]
            .tags
            .contains(tag));
        Ok(())
    }

    #[tokio::test]
    async fn test_batch_get_api_data() -> reqwest::Result<()> {
        let client = Client::new();
        let tag = "cat";
        let num_imgs = 101;

        let resp = BatchGetter::build(&client, tag, num_imgs)
            .unwrap()
            .run()
            .await?;
        assert_eq!(resp.len(), usize::try_from(num_imgs).unwrap());

        let tag = "balabala just no exist";
        let resp = BatchGetter::build(&client, tag, num_imgs)
            .unwrap()
            .run()
            .await?;
        assert!(resp.is_empty());
        Ok(())
    }
}
