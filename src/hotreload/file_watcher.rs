use std::{collections::BTreeMap, path::PathBuf};

use crossbeam_channel::unbounded;
use notify::{Event, RecursiveMode, Watcher};

use crate::hotreload::fs_utils::clean_path;

pub struct FileWatcher {
  pub files: BTreeMap<PathBuf, bool>,
  watcher: Option<notify::RecommendedWatcher>,
  list_tx: Vec<crossbeam_channel::Sender<notify::Event>>,
}
type Channel = (
  crossbeam_channel::Sender<notify::Event>,
  crossbeam_channel::Receiver<notify::Event>,
);

impl FileWatcher {
  pub fn new() -> Self {
    Self {
      files: BTreeMap::new(),
      watcher: None,
      list_tx: vec![],
    }
  }
  pub fn is_running(&self) -> bool {
    self.watcher.is_some()
  }
  pub fn new_channel(&mut self) -> Channel {
    let (tx, rx) = unbounded::<notify::Event>();
    self.list_tx.push(tx.clone());
    (tx, rx)
  }
  pub fn add(&mut self, path: PathBuf, recursive: bool) -> &mut Self {
    let path = clean_path(&path);
    let mode = match recursive {
      true => notify::RecursiveMode::Recursive,
      false => notify::RecursiveMode::NonRecursive,
    };

    if !self.files.contains_key(&path) {
      if let Some(watcher) = self.watcher.as_mut() {
        watcher.watch(&path, mode).ok();
      }
      self.files.insert(path.clone(), recursive);
    }
    self
  }

  #[allow(unused)]
  pub fn stop(&mut self) {
    if let Some(watcher) = self.watcher.take() {
      drop(watcher);
    }
  }
  pub fn run(&mut self) -> Option<()> {
    (!self.is_running()).then_some(())?;
    self.watcher = notify::recommended_watcher({
      let list_tx = self.list_tx.clone();
      move |res: notify::Result<Event>| match res {
        Ok(event) => list_tx.iter().for_each(|sender| {
          sender.send(event.clone()).ok();
        }),
        Err(e) => eprintln!("watch error: {:?}", e),
      }
    })
    .ok();
    let watcher = self.watcher.as_mut()?;
    self.files.iter().for_each(|(path, recursive)| {
      watcher
        .watch(
          path,
          recursive
            .then_some(RecursiveMode::Recursive)
            .unwrap_or(RecursiveMode::NonRecursive),
        )
        .ok();
    });

    Some(())
  }
}
