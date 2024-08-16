//! Utils for hashing files using various digest algorithms.
//!
//! Usually, you don't need to use this module directly.
//! [`crate::scheduler`] will automatically hash the files
//! to check if the file is already downloaded.

use std::cmp::min;
use std::path::Path;

use digest::Digest;
use tokio::io::AsyncReadExt;

const DEFAULT_BUF_SIZE: usize = 2 * 1024 * 1024; // 2MB

/** Hash a file using the specified digest algorithm.

# Example

```rust
use std::io::Write;

use booru_dl::hash::hash_file;

type Md5Hasher = md5::Md5;

#[tokio::main]
async fn main() {
    // We create a temporary file to demonstrate
    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(b"The quick brown fox jumps over the lazy dog")
        .unwrap();
    file.flush()
        .unwrap();

    let hash = hash_file::<Md5Hasher>(file.path()).await.unwrap();
    // see: https://en.wikipedia.org/wiki/MD5#MD5_hashes
    assert_eq!(hash, "9e107d9d372bb6826bd81d3542a419d6");

    // clean up the temporary file
    file.close().unwrap();
}
```

# Errors

I/O error when reading the file.
*/
pub async fn hash_file<D: Digest + std::marker::Send + 'static>(
    filepath: impl AsRef<Path>,
) -> std::io::Result<String> {
    let mut file = tokio::fs::File::open(filepath).await?;
    let file_size = file.metadata().await?.len();

    let buf_size = min(DEFAULT_BUF_SIZE, file_size.try_into().unwrap());

    // TODO: We recreate and drop a new buffer each time,
    // which leads to performance degradation;
    // perhaps we need to use a memory pool to reuse these buffers.
    let hasher = Box::new(D::new());
    let buf = vec![u8::default(); buf_size].into_boxed_slice();
    let mut cell = Option::Some((hasher, buf));

    let hash = loop {
        let (mut hasher, mut buf) = cell.take().unwrap();

        let n = file.read(&mut buf).await?;
        if n == 0 {
            break hasher;
        }
        cell.replace(
            tokio_rayon::spawn(move || {
                hasher.update(&buf.as_mut()[0..n]);
                (hasher, buf)
            })
            .await,
        );
    }
    .finalize();

    Ok(base16ct::lower::encode_string(&hash))
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[tokio::test]
    async fn test_md5_hash_file() {
        type Md5Hasher = md5::Md5;

        // see: https://en.wikipedia.org/wiki/MD5#MD5_hashes

        let mut file = tempfile::NamedTempFile::new().unwrap();
        let hash = hash_file::<Md5Hasher>(&file).await.unwrap();
        assert_eq!(hash, "d41d8cd98f00b204e9800998ecf8427e");

        file.write_all(b"The quick brown fox jumps over the lazy dog")
            .unwrap();
        file.flush().unwrap();
        let hash = hash_file::<Md5Hasher>(&file).await.unwrap();
        assert_eq!(hash, "9e107d9d372bb6826bd81d3542a419d6");

        file.close().unwrap();
    }
}
