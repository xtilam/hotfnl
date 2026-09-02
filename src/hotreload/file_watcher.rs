//! A lightweight filesystem watcher wrapping the `notify` crate.

use std::{collections::BTreeMap, path::PathBuf};

use crossbeam_channel::unbounded;
use notify::{Event, RecursiveMode, Watcher};

use crate::hotreload::fs_utils::clean_path;

/// Watches a set of filesystem paths and fans out change events to multiple channels.
pub struct FileWatcher {
  /// Paths to watch, mapped to whether each is watched recursively.
  pub files: BTreeMap<PathBuf, bool>,
  watcher: Option<notify::RecommendedWatcher>,
  list_tx: Vec<crossbeam_channel::Sender<notify::Event>>,
}
type Channel = (
  crossbeam_channel::Sender<notify::Event>,
  crossbeam_channel::Receiver<notify::Event>,
);

impl FileWatcher {
  /// Creates an empty watcher.
  pub fn new() -> Self {
    Self {
      files: BTreeMap::new(),
      watcher: None,
      list_tx: vec![],
    }
  }

  /// Returns whether the watcher has been started.
  pub fn is_running(&self) -> bool {
    self.watcher.is_some()
  }

  /// Registers and returns a new event channel that will receive watch events.
  pub fn new_channel(&mut self) -> Channel {
    let (tx, rx) = unbounded::<notify::Event>();
    self.list_tx.push(tx.clone());
    (tx, rx)
  }

  /// Adds a path to watch, optionally recursively.
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

  /// Stops watching and drops the underlying watcher.
  #[allow(unused)]
  pub fn stop(&mut self) {
    if let Some(watcher) = self.watcher.take() {
      drop(watcher);
    }
  }

  /// Starts the watcher for all registered paths. Returns `None` if it is already
  /// running or could not be started.
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
