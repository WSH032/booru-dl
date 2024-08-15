use std::path::PathBuf;
use std::sync::LazyLock;

use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};

pub mod url {
    use super::*;

    pub const BASE_URL: &str = "https://gelbooru.com/index.php";

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

    pub static POST_URL: LazyLock<Url> = LazyLock::new(|| {
        // see: https://gelbooru.com/index.php?page=wiki&s=view&id=18780
        Url::parse_with_params(BASE_URL, &[("page", "post"), ("s", "list"), ("q", "index")])
            .unwrap()
    });
}

#[derive(Deserialize)]
pub(crate) struct PostInner {
    pub(crate) id: u64,
    pub(crate) md5: String,
    pub(crate) file_url: String,
    pub(crate) tags: String,
    pub(crate) image: PathBuf,
}

impl From<PostInner> for data::field::Post {
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

pub mod data {
    use super::*;

    pub mod field {
        use super::*;

        #[non_exhaustive]
        #[derive(Debug, Deserialize, Serialize)]
        pub struct Attributes {
            pub limit: u64,
            pub offset: u64,
            pub count: u64,
        }

        #[non_exhaustive]
        #[derive(Debug, Deserialize, Serialize)]
        #[serde(from = "PostInner")]
        pub struct Post {
            pub id: u64,
            pub md5: String,
            pub file_url: String,
            pub tags: String,
            pub image: PathBuf,
            pub(crate) filename: PathBuf,
        }
    }

    #[non_exhaustive]
    #[derive(Debug, Deserialize, Serialize)]
    pub struct Json {
        #[serde(rename = "@attributes")]
        pub attributes: field::Attributes,
        /// if `attributes.count` is 0, or `attributes.pid` is out of range,
        /// this field will be `None`.
        pub post: Option<Vec<field::Post>>,
    }

    pub struct Getter<'a> {
        client: &'a Client,
        tags: &'a str,
        limit: u64,
        pid: u64,
    }

    impl Getter<'_> {
        /// if `limit * pid > 20_000`, the API will return an error.
        /// see: <https://gelbooru.com/index.php?page=forum&s=view&id=1549>
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

        pub async fn run(self) -> reqwest::Result<Json> {
            let mut target_url = url::API_URL.clone();
            target_url.query_pairs_mut().extend_pairs([
                ("tags", self.tags),
                ("limit", &self.limit.to_string()),
                ("pid", &self.pid.to_string()),
            ]);
            self.client.get(target_url).send().await?.json().await
        }
    }

    pub struct BatchGetter<'a> {
        client: &'a Client,
        tags: &'a str,
        num_imgs: u64,
    }

    impl BatchGetter<'_> {
        /// if `num_imgs > 20_000`, the API will return an error.
        /// see: <https://gelbooru.com/index.php?page=forum&s=view&id=1549>
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

        pub async fn run(self) -> reqwest::Result<Vec<field::Post>> {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_illegal_args() {
        let client = Client::new();

        let resp = data::Getter::build(&client, "", 100, 0);
        assert!(resp.is_err());

        let resp = data::Getter::build(&client, "cat", 0, 0);
        assert!(resp.is_err());
    }

    #[tokio::test]
    async fn test_get_api_data() -> reqwest::Result<()> {
        let client = Client::new();
        let tag = "cat";
        let limit = 10;

        let resp = data::Getter::build(&client, tag, limit, 0)
            .unwrap()
            .run()
            .await?;
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

        let resp = data::BatchGetter::build(&client, tag, num_imgs)
            .unwrap()
            .run()
            .await?;
        assert_eq!(resp.len(), usize::try_from(num_imgs).unwrap());

        let tag = "balabala just no exist";
        let resp = data::BatchGetter::build(&client, tag, num_imgs)
            .unwrap()
            .run()
            .await?;
        assert!(resp.is_empty());
        Ok(())
    }
}
