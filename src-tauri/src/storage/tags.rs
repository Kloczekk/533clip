use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TagStoreError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TagRegistry {
    known_tags: Vec<String>,
}

#[derive(Clone)]
pub struct TagRegistryStore {
    path: PathBuf,
}

impl TagRegistryStore {
    pub fn open(data_dir: &Path) -> Result<Self, TagStoreError> {
        std::fs::create_dir_all(data_dir)?;
        Ok(Self {
            path: data_dir.join("tags.json"),
        })
    }

    fn load(&self) -> Result<TagRegistry, TagStoreError> {
        if !self.path.exists() {
            return Ok(TagRegistry::default());
        }
        let raw = std::fs::read_to_string(&self.path)?;
        if raw.trim().is_empty() {
            return Ok(TagRegistry::default());
        }
        Ok(serde_json::from_str(&raw)?)
    }

    fn save(&self, registry: &TagRegistry) -> Result<(), TagStoreError> {
        let json = serde_json::to_string_pretty(registry)?;
        std::fs::write(&self.path, json)?;
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<String>, TagStoreError> {
        Ok(self.load()?.known_tags)
    }

    pub fn ensure_tag(&self, tag: &str) -> Result<(), TagStoreError> {
        let mut registry = self.load()?;
        let normalized = tag.trim().to_lowercase();
        if normalized.is_empty() {
            return Ok(());
        }
        if !registry.known_tags.iter().any(|t| t == &normalized) {
            registry.known_tags.push(normalized);
            registry.known_tags.sort();
            self.save(&registry)?;
        }
        Ok(())
    }

    pub fn remove_tag(&self, tag: &str) -> Result<(), TagStoreError> {
        let mut registry = self.load()?;
        let before = registry.known_tags.len();
        registry.known_tags.retain(|t| t != tag);
        if registry.known_tags.len() != before {
            self.save(&registry)?;
        }
        Ok(())
    }
}

pub fn merge_tags_from_clips(known: Vec<String>, clip_tags: impl Iterator<Item = String>) -> Vec<String> {
    let mut set: BTreeSet<String> = known.into_iter().map(|t| t.to_lowercase()).collect();
    for t in clip_tags {
        let n = t.trim().to_lowercase();
        if !n.is_empty() {
            set.insert(n);
        }
    }
    set.into_iter().collect()
}
