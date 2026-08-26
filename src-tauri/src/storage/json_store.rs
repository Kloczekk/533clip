use crate::models::clip::Clip;
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
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
        let list: Vec<Clip> = super::load_json_or_default(&path)?;
        let clips: HashMap<String, Clip> = list.into_iter().map(|c| (c.id.clone(), c)).collect();

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

    pub fn remove_missing_files(&self) -> Result<usize, StoreError> {
        let mut guard = self.inner.write();
        let missing: Vec<String> = guard
            .values()
            .filter(|clip| !Path::new(&clip.file_path).is_file())
            .map(|clip| clip.id.clone())
            .collect();
        for id in &missing {
            if let Some(clip) = guard.remove(id) {
                if let Some(thumb) = &clip.thumbnail_path {
                    let _ = std::fs::remove_file(thumb);
                }
            }
        }
        if !missing.is_empty() {
            self.persist(&guard)?;
        }
        Ok(missing.len())
    }

    pub fn thumbnail_paths(&self) -> HashSet<PathBuf> {
        self.inner
            .read()
            .values()
            .filter_map(|clip| clip.thumbnail_path.as_ref())
            .map(PathBuf::from)
            .collect()
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

    /// Applies `f` to every clip in `ids` under a single write lock and
    /// persists once, instead of once per id. Use for bulk actions (multi-tag,
    /// multi-move) so selecting many clips doesn't rewrite clips.json N times.
    pub fn update_many<F>(&self, ids: &[String], mut f: F) -> Result<Vec<Clip>, StoreError>
    where
        F: FnMut(&mut Clip),
    {
        let mut guard = self.inner.write();
        let mut updated = Vec::new();
        for id in ids {
            if let Some(clip) = guard.get_mut(id) {
                f(clip);
                updated.push(clip.clone());
            }
        }
        if !updated.is_empty() {
            self.persist(&guard)?;
        }
        Ok(updated)
    }

    fn persist(&self, map: &HashMap<String, Clip>) -> Result<(), StoreError> {
        let mut list: Vec<&Clip> = map.values().collect();
        list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        let owned: Vec<Clip> = list.into_iter().cloned().collect();
        let json = serde_json::to_string_pretty(&owned)?;
        super::atomic_write(&self.path, &json)?;
        Ok(())
    }
}
