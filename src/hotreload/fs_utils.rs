//! Filesystem helpers used by the hot-reload machinery.

use anyhow::Result;
use std::{
  hash::{DefaultHasher, Hash, Hasher},
  path::{Component, Path, PathBuf},
};

/// Normalizes a path by resolving `.` and `..` components literally.
pub fn clean_path(path: &Path) -> PathBuf {
  let mut result = PathBuf::new();
  for component in path.components() {
    match component {
      Component::CurDir => {}
      Component::ParentDir => {
        result.pop();
      }
      component => result.push(component.as_os_str()),
    }
  }
  result
}

/// Returns the file stem of a binary path as a [`String`].
pub fn bin_name(bin_path: &PathBuf) -> String {
  bin_path.file_stem().unwrap().to_str().unwrap().to_string()
}

/// Computes a 64-bit hash of `data` (used to avoid rewriting unchanged files).
pub fn hash(data: &[u8]) -> u64 {
  let mut h = DefaultHasher::new();
  data.hash(&mut h);
  h.finish()
}

/// Writes `content` to `path` only if it differs from the current contents, creating
/// parent directories as needed.
pub fn write_file(path: &PathBuf, content: &str) -> Result<()> {
  std::fs::read_to_string(path)
    .ok()
    .map_or(Some(true), |c| {
      (hash(c.as_bytes()) != hash(content.as_bytes())).then_some(true)
    })
    .map(|_| -> Result<()> {
      std::fs::create_dir_all(path.parent().unwrap())?;
      std::fs::write(path, content)?;
      Ok(())
    })
    .unwrap_or(Ok(()))
}

/// Creates a symlink at `dst` pointing to `src`, replacing any existing `dst`.
pub fn link_file(src: &PathBuf, dst: &PathBuf) -> Result<()> {
  if dst.exists() {
    std::fs::remove_file(dst)?;
  }
  std::os::unix::fs::symlink(src, dst)?;
  Ok(())
}
