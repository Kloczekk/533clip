use crate::models::clip::Clip;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Clone)]
pub struct ClipStore {
    path: PathBuf,
    inner: std::sync::Arc<RwLock<HashMap<String, Clip>>>,
}

impl ClipStore {
    pub fn open(data_dir: &Path) -> Result<Self, StoreError> {
        std::fs::create_dir_all(data_dir)?;
        let path = data_dir.join("clips.json");
        let clips = if path.exists() {
            let raw = std::fs::read_to_string(&path)?;
            if raw.trim().is_empty() {
                HashMap::new()
            } else {
                let list: Vec<Clip> = serde_json::from_str(&raw)?;
                list.into_iter().map(|c| (c.id.clone(), c)).collect()
            }
        } else {
            HashMap::new()
        };

        Ok(Self {
            path,
            inner: std::sync::Arc::new(RwLock::new(clips)),
        })
    }

    pub fn list(&self) -> Vec<Clip> {
        let mut clips: Vec<Clip> = self.inner.read().values().cloned().collect();
        clips.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        clips
    }

    pub fn get(&self, id: &str) -> Option<Clip> {
        self.inner.read().get(id).cloned()
    }

    pub fn by_file_path(&self, file_path: &str) -> Option<Clip> {
        self.inner
            .read()
            .values()
            .find(|c| c.file_path == file_path)
            .cloned()
    }

    pub fn upsert(&self, clip: Clip) -> Result<Clip, StoreError> {
        {
            let mut guard = self.inner.write();
            guard.insert(clip.id.clone(), clip.clone());
            self.persist(&guard)?;
        }
        Ok(clip)
    }

    pub fn remove_many(&self, ids: &[String]) -> Result<Vec<Clip>, StoreError> {
        let mut removed = Vec::new();
        let mut guard = self.inner.write();
        for id in ids {
            if let Some(clip) = guard.remove(id) {
                if let Some(thumb) = &clip.thumbnail_path {
                    let _ = std::fs::remove_file(thumb);
                }
                let _ = std::fs::remove_file(&clip.file_path);
                removed.push(clip);
            }
        }
        if !removed.is_empty() {
            self.persist(&guard)?;
        }
        Ok(removed)
    }

    pub fn remove(&self, id: &str) -> Result<Option<Clip>, StoreError> {
        let mut guard = self.inner.write();
        if let Some(clip) = guard.remove(id) {
            if let Some(thumb) = &clip.thumbnail_path {
                let _ = std::fs::remove_file(thumb);
            }
            let _ = std::fs::remove_file(&clip.file_path);
            self.persist(&guard)?;
            Ok(Some(clip))
        } else {
            Ok(None)
        }
    }

    pub fn remove_tag_from_all_clips(&self, tag: &str) -> Result<Vec<Clip>, StoreError> {
        let mut guard = self.inner.write();
        let mut updated = Vec::new();
        for clip in guard.values_mut() {
            let before = clip.tags.len();
            clip.tags.retain(|t| t != tag);
            if clip.tags.len() != before {
                updated.push(clip.clone());
            }
        }
        if !updated.is_empty() {
            self.persist(&guard)?;
        }
        Ok(updated)
    }

    pub fn update<F>(&self, id: &str, mut f: F) -> Result<Option<Clip>, StoreError>
    where
        F: FnMut(&mut Clip),
    {
        let mut guard = self.inner.write();
        if let Some(clip) = guard.get_mut(id) {
            f(clip);
            let updated = clip.clone();
            self.persist(&guard)?;
            Ok(Some(updated))
        } else {
            Ok(None)
        }
    }

    fn persist(&self, map: &HashMap<String, Clip>) -> Result<(), StoreError> {
        let mut list: Vec<&Clip> = map.values().collect();
        list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        let owned: Vec<Clip> = list.into_iter().cloned().collect();
        let json = serde_json::to_string_pretty(&owned)?;
        std::fs::write(&self.path, json)?;
        Ok(())
    }
}
