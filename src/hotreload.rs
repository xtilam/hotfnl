//! Internal hot-reload subsystem.
//!
//! This module contains the runtime engine that loads dynamic libraries, patches
//! function pointers, and drives the hot-reload loop. It is only compiled when the
//! `prod` feature is disabled.

mod event;
mod file_watcher;
mod fs_utils;
mod hotfn;
mod hotlib;
mod hotproject;
mod hotproject_server;
mod macro_utils;
mod wrapper;
mod files;

use std::{env::args, os::unix::process::CommandExt, process::Command, sync};

use anyhow::Result;
pub use event::{EventCallbackList, HotLibEvent};
pub use hotfn::HotFn;
pub use hotlib::{HotLib, PatchErr, get_fn_idx, get_fn_list};
pub use hotproject::HotProjectWatcherConfig;
pub use wrapper::*;

pub fn add_cargo_args(args: impl IntoIterator<Item = impl Into<String>>) {
  let projects = &mut HotLib::get_instance_mut().project;
  args.into_iter().for_each(|s| {
    projects.custom_args.push(s.into());
  })
}

/// Boots the hot-reload system with the set of hot-swappable functions gathered from
/// the [`crate::inventory`] registry.
///
/// This is normally invoked by the `#[hot_main]` proc macro.
pub fn boot(is_hot_project: bool, fns: Vec<HotFn>, file_name: &str, project_dir: &str) {
  HotLib::get_instance_mut().on_boot(is_hot_project, fns, file_name, project_dir);
}

/// Returns the shared event registry, used to register lifecycle callbacks.
pub fn get_events() -> sync::Arc<sync::RwLock<HotLibEvent>> {
  HotLib::get_instance().event.clone()
}

/// Starts the hot-reload runtime: scaffolds the hot project on first run, builds it,
/// and spawns the background watcher loop.
pub fn run() -> Result<()> {
  if !HotLib::get_instance().is_hot_project {
    HotLib::get_instance().project.init_hot_project().unwrap();
    hot_run().unwrap();
  }
  HotLib::get_instance_mut().run_watch_lib();

  Ok(())
}

fn hot_run() -> Result<()> {
  HotLib::get_instance()
    .project
    .rebuild_command()
    .spawn()?
    .wait()?;

  let _ = Command::new(files::wrapper::bin_path(&HotLib::get_instance().project))
    .args(args())
    .exec();
  Ok(())
}

/// Returns a fresh, empty event callback list scoped to the shared event registry.
pub fn new_event_list() -> EventCallbackList {
  EventCallbackList::default()
}
