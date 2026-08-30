mod event;
mod file_watcher;
mod fs_utils;
mod hotfn;
mod hotlib;
mod hotproject;
mod watch_task;
mod wrapper;
use std::{os::unix::process::CommandExt, sync};
mod hotproject_files;
mod log;
mod macro_utils;

use anyhow::Result;
pub use event::{HotLibEvent, EventCallbackList};
pub use hotfn::HotFn;
pub use hotlib::{HotLib, PatchErr, get_fn_idx, get_fn_list};
pub use hotproject::HotProjectWatcherConfig;
pub use wrapper::*;

pub fn reload_lib() {
  HotLib::get_instance()
    .trigger(hotlib::HotLibAction::ReloadLib)
    .ok();
}

pub fn boot(is_hot_project: bool, fns: Vec<HotFn>, file_name: &str, project_dir: &str) {
  HotLib::get_instance_mut().on_boot(is_hot_project, fns, file_name, project_dir);
}
pub fn get_events() -> sync::Arc<sync::RwLock<HotLibEvent>> {
  HotLib::get_instance().event.clone()
}

pub fn run() -> Result<()> {
  if !HotLib::get_instance().is_hot_project {
    HotLib::get_instance().project.init_hot_project()?;
    hot_run()?;
  }
  HotLib::get_instance_mut().run_watch_lib();

  Ok(())
}

fn hot_run() -> Result<()> {
  let _ = HotLib::get_instance().project.wrapper_command(None).exec();
  Ok(())
}

pub fn new_event_list() -> EventCallbackList {
  EventCallbackList::default()
}
