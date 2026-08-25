use std::{
  collections::BTreeMap,
  path::PathBuf,
  sync::{Arc, RwLock, mpsc},
};

use notify::{Event, Watcher};

use crate::hotreload::fs_utils::clean_path;

pub struct FileWatcher {
  pub files: BTreeMap<PathBuf, notify::RecursiveMode>,
  watcher: Option<notify::RecommendedWatcher>,
  on_change: Arc<RwLock<Vec<mpsc::Sender<notify::Event>>>>,
}

impl FileWatcher {
  pub fn new() -> Self {
    Self {
      files: BTreeMap::new(),
      watcher: None,
      on_change: Arc::new(RwLock::new(Vec::new())),
    }
  }
  pub fn is_running(&self) -> bool {
    self.watcher.is_some()
  }
  pub fn add(&mut self, path: PathBuf, recursive: bool) {
    let path = clean_path(&path);
    let mode = match recursive {
      true => notify::RecursiveMode::Recursive,
      false => notify::RecursiveMode::NonRecursive,
    };

    if !self.files.contains_key(&path) {
      if let Some(watcher) = self.watcher.as_mut() {
        watcher.watch(&path, mode).ok();
      }
      self.files.insert(path.clone(), mode);
    }
  }
  pub fn stop(&mut self) {
    if let Some(watcher) = self.watcher.take() {
      drop(watcher);
    }
  }
  pub fn run(&mut self) -> Option<()> {
    (!self.is_running()).then_some(())?;

    self.watcher = notify::recommended_watcher({
      let on_change = self.on_change.clone();
      move |res: notify::Result<Event>| match res {
        Ok(event) => {
          on_change
            .read()
            .map(|senders| {
              for sender in senders.iter() {
                let _ = sender.send(event.clone());
              }
            })
            .ok();
        }
        Err(e) => eprintln!("watch error: {:?}", e),
      }
    })
    .ok();

    let watcher = self.watcher.as_mut()?;
    self.files.iter().for_each(|(path, mode)| {
      watcher.watch(path, *mode).ok();
    });
    Some(())
  }
}
