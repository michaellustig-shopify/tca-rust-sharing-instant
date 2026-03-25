//! ┌─────────────────────────────────────────────────────┐
//! │  FILE STORAGE KEY                                    │
//! │  Persistence backed by the local file system          │
//! ├─────────────────────────────────────────────────────┤
//! │                                                      │
//! │  FileStorageKey<V: Serialize + DeserializeOwned>     │
//! │    ├── load()      → read JSON from file             │
//! │    ├── save()      → write JSON to file              │
//! │    └── subscribe() → watch file for changes          │
//! │                                                      │
//! ├─────────────────────────────────────────────────────┤
//! │  WHY: Mirrors Swift's FileStorageKey. Useful for     │
//! │  settings and configuration that should persist       │
//! │  locally without requiring network connectivity.     │
//! │                                                      │
//! │  TESTED BY: tests/file_storage_key_tests.rs          │
//! │                                                      │
//! │  CHANGELOG:                                          │
//! │  • v0.1.0 — Initial FileStorageKey                   │
//! │                                                      │
//! │  HISTORY: git log --oneline --follow -- crates/sharing-instant/src/keys/file_storage_key.rs │
//! └─────────────────────────────────────────────────────┘

use crate::error::{Result, SharingInstantError};
use crate::shared_key::{SaveContext, SharedKey};
use crate::shared_reader_key::{LoadContext, SharedReaderKey, SharedSubscriber};
use crate::subscription::SharedSubscription;
use serde::{de::DeserializeOwned, Serialize};
use std::path::PathBuf;

/// File system persistence key.
///
/// Stores values as JSON files on disk. Mirrors Swift's `FileStorageKey`.
///
/// # Example
///
/// ```ignore
/// use sharing_instant::keys::file_storage_key::FileStorageKey;
/// use sharing_instant::shared::Shared;
///
/// let key = FileStorageKey::new("/tmp/settings.json");
/// let shared = Shared::new(Settings::default(), key);
/// ```
pub struct FileStorageKey<V: Serialize + DeserializeOwned + Send + Sync + Clone + 'static> {
    path: PathBuf,
    _phantom: std::marker::PhantomData<V>,
}

impl<V: Serialize + DeserializeOwned + Send + Sync + Clone + 'static> FileStorageKey<V> {
    /// Create a new file storage key at the given path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<V: Serialize + DeserializeOwned + Send + Sync + Clone + 'static> SharedReaderKey
    for FileStorageKey<V>
{
    type Value = V;
    type Id = String;

    fn id(&self) -> String {
        format!("file:{}", self.path.display())
    }

    fn load(&self, _context: LoadContext<V>) -> Result<Option<V>> {
        if !self.path.exists() {
            return Ok(None);
        }

        let contents = std::fs::read_to_string(&self.path)
            .map_err(|e| SharingInstantError::KeyError(e.to_string()))?;

        let value: V = serde_json::from_str(&contents)
            .map_err(|e| SharingInstantError::SerializationError(e.to_string()))?;

        Ok(Some(value))
    }

    fn subscribe(
        &self,
        _context: LoadContext<V>,
        _subscriber: SharedSubscriber<V>,
    ) -> SharedSubscription {
        // TODO: File watching via notify crate
        SharedSubscription::empty()
    }
}

impl<V: Serialize + DeserializeOwned + Send + Sync + Clone + 'static> SharedKey
    for FileStorageKey<V>
{
    fn save(&self, value: &V, _context: SaveContext) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| SharingInstantError::KeyError(e.to_string()))?;
        }

        let json = serde_json::to_string_pretty(value)
            .map_err(|e| SharingInstantError::SerializationError(e.to_string()))?;

        std::fs::write(&self.path, json)
            .map_err(|e| SharingInstantError::KeyError(e.to_string()))?;

        Ok(())
    }
}
