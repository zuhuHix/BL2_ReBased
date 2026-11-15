//! Owner: lane W2-ASSETS. Texture file caches: the port of `assets::TfcStore` (`src/assets.cpp`). A streamed mip's
//! bytes live in `<root>/<cache name>.tfc` at the offset its record gives; the cache name is the texture's
//! `TextureFileCacheName` property.

use std::io::{Read as _, Seek as _, SeekFrom};
use std::path::{Path, PathBuf};

use crate::texture::{AssetError, check};

/// The largest stored mip the store reads (the C++ bound).
const MAX_STORED: u32 = 512 * 1024 * 1024;

/// The `.tfc` files of one directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TfcStore {
    root: PathBuf,
}

impl TfcStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        TfcStore { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `stored_bytes` bytes at `offset` of cache `cache_name`, with the C++ checks in its order: a cache name that is
    /// not empty, not `None` and holds no `/`, `\` or `:`; `.tfc` added unless the name ends in it; the file opens;
    /// the range lies inside the file; at most 512 MiB.
    pub fn read(
        &self,
        cache_name: &str,
        offset: u32,
        stored_bytes: u32,
    ) -> Result<Vec<u8>, AssetError> {
        check(
            !cache_name.is_empty()
                && cache_name != "None"
                && !cache_name.contains(['/', '\\', ':']),
            AssetError::TfcName,
        )?;
        let filename = if cache_name.ends_with(".tfc") {
            cache_name.to_string()
        } else {
            format!("{cache_name}.tfc")
        };
        let mut file =
            std::fs::File::open(self.root.join(filename)).map_err(|_| AssetError::TfcOpen)?;
        // The C++ opens at the end and takes the position as the size; a size it cannot get fails the range check.
        let total = file.metadata().map_err(|_| AssetError::TfcRange)?.len();
        check(
            u64::from(offset) + u64::from(stored_bytes) <= total,
            AssetError::TfcRange,
        )?;
        check(stored_bytes <= MAX_STORED, AssetError::TfcTooLarge)?;
        let mut payload = vec![0u8; stored_bytes as usize];
        if stored_bytes > 0 {
            file.seek(SeekFrom::Start(u64::from(offset)))
                .map_err(|_| AssetError::TfcRead)?;
            file.read_exact(&mut payload)
                .map_err(|_| AssetError::TfcRead)?;
        }
        Ok(payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caches_are_read_by_name_and_range() {
        let dir = std::env::temp_dir().join(format!("ow-assets-tfc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Cache.tfc"), [0u8, 1, 2, 3, 4, 5]).unwrap();
        let store = TfcStore::new(&dir);
        assert_eq!(store.read("Cache", 2, 3), Ok(vec![2, 3, 4]));
        assert_eq!(store.read("Cache.tfc", 6, 0), Ok(vec![]));
        assert_eq!(store.read("Cache", 4, 3), Err(AssetError::TfcRange));
        assert_eq!(store.read("Missing", 0, 0), Err(AssetError::TfcOpen));
        for bad in ["", "None", "a/b", "a\\b", "C:x"] {
            assert_eq!(store.read(bad, 0, 0), Err(AssetError::TfcName), "{bad:?}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
