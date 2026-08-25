use anyhow::Result;
use std::{
  hash::{DefaultHasher, Hash, Hasher}, path::{Component, Path, PathBuf},
};

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
pub fn bin_name(bin_path: &PathBuf) -> String {
  bin_path.file_stem().unwrap().to_str().unwrap().to_string()
}

pub fn hash(data: &[u8]) -> u64 {
  let mut h = DefaultHasher::new();
  data.hash(&mut h);
  h.finish()
}

pub fn write_file(path: &PathBuf, content: &str) -> Result<()> {
  std::fs::read_to_string(path)
    .ok()
    .map_or(Some(true), |c| {
      (hash(c.as_bytes()) != hash(content.as_bytes())).then_some(true)
    })
    .map(|_| -> Result<()> {
      println!("write_file: {:?}", path);
      std::fs::create_dir_all(path.parent().unwrap())?;
      std::fs::write(path, content)?;
      Ok(())
    })
    .unwrap_or(Ok(()))
}

pub fn link_file(src: &PathBuf, dst: &PathBuf) -> Result<()> {
  if dst.exists() {
    std::fs::remove_file(dst)?;
  }
  std::os::unix::fs::symlink(src, dst)?;
  Ok(())
}
