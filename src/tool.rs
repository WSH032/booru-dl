//! Some useful tools for the project.
//!
//! Usually, you don't need to use this module directly.
//! [`crate::scheduler`] will automatically use these tools.

use std::ffi::OsString;
use std::num::NonZeroUsize;
use std::sync::LazyLock;
use std::thread::available_parallelism;

/// The number of CPUs available to the program.
/// You can consider this as cache of [`std::thread::available_parallelism`].
pub static NUM_CPUS: LazyLock<NonZeroUsize> =
    LazyLock::new(|| available_parallelism().unwrap_or(NonZeroUsize::new(1).unwrap()));

/// Modify the file stem of the path.
pub(crate) trait SetFileStem {
    fn set_file_stem(&mut self, stem: impl Into<OsString>);
}

impl SetFileStem for std::path::PathBuf {
    fn set_file_stem(&mut self, stem: impl Into<OsString>) {
        let mut stem: OsString = stem.into();
        let extension = self.extension();
        if let Some(extension) = extension {
            stem.push(".");
            stem.push(extension);
        }

        self.set_file_name(stem);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_file_stem() {
        let mut path = std::path::PathBuf::from("/tmp/test.txt");
        path.set_file_stem("test2");
        assert_eq!(path, std::path::PathBuf::from("/tmp/test2.txt"));

        let mut path = std::path::PathBuf::from("test.txt");
        path.set_file_stem("test2");
        assert_eq!(path, std::path::PathBuf::from("test2.txt"));
    }
}
