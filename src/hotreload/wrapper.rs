//! The wrapper binary's orchestration logic.
//!
//! The generated wrapper binary drives the hot-reload loop: it runs the app, watches
//! source files, triggers rebuilds, and snapshots updated libraries.

use crate::hotreload::{
  file_watcher::FileWatcher,
  hotproject::{HotProject, HotProjectState, HotProjectStoreData},
  macro_utils::bselect,
};

use anyhow::{Result, bail};
use postcard::to_allocvec;
use std::{
  os::unix::net::UnixDatagram,
  path::PathBuf,
  process::{Child, Stdio},
  time::Duration,
};

pub fn app(data_path: &str) {
  let server = HotProjectServer::new(data_path).unwrap();
  server.run().unwrap();
}

pub struct HotProjectServer {
  watch_src: FileWatcher,
  project: HotProject,
  fd: UnixDatagram,
  sock_path: String,
}

impl HotProjectServer {
  pub fn new(data_path: &str) -> Result<Self> {
    let store = {
      let path = PathBuf::from(data_path);
      toml::de::from_str::<HotProjectStoreData>(&std::fs::read_to_string(path)?)?
    };
    let mut watch_src = FileWatcher::new();
    watch_src.files = store.watch_src;
    let sock_path = store.project.files().data().project_sock_path();

    let fd = UnixDatagram::unbound()?;
    Ok(HotProjectServer {
      watch_src,
      project: store.project,
      fd,
      sock_path: sock_path.to_string_lossy().to_string(),
    })
  }
  fn send_state(&self, state: HotProjectState) -> Option<()> {
    let data = to_allocvec(&state).ok()?;
    self.fd.send_to(data.as_slice(), &self.sock_path).ok()?;
    Some(())
  }
  fn run(mut self) -> Result<()> {
    let (_, src_change_rx) = self.watch_src.new_channel();
    self.watch_src.run();
    println!(
      "HotProjectServer is running... {}",
      self
        .watch_src
        .files
        .iter()
        .map(|f| format!("\r\n => {} : {}", f.0.to_string_lossy(), f.1))
        .collect::<Vec<_>>()
        .join("")
    );

    let mut is_src_changed = false;
    let mut restart_count = 1;
    let mut rebuild_task = None::<Child>;
    let mut app_task = None::<Child>;

    let mut cargo_watcher = FileWatcher::new();
    for f in {
      let mut files = vec![self.project.root_dir.join("Cargo.toml")];
      if self.project.is_workspace {
        files.push(self.project.workspace_dir.join("Cargo.toml"));
        files.push(self.project.workspace_dir.join("Cargo.lock"));
      } else {
        files.push(self.project.root_dir.join("Cargo.lock"));
      }
      files
    } {
      if !std::fs::exists(&f).unwrap() {
        panic!("{:?} not exists", f);
      };
      cargo_watcher.files.insert(f.clone(), false);
    }
    let (_, cargo_watch_rx) = cargo_watcher.new_channel();
    cargo_watcher.run();

    loop {
      use HotProjectState::*;
      bselect!(
        [recv(cargo_watch_rx), |evt| {
          if let Ok(evt) = evt
            && evt.kind.is_modify()
          {
            if let Some(mut child) = rebuild_task.take() {
              child.kill().ok();
              child.wait().ok();
            };
            if let Some(mut child) = app_task.take() {
              child.kill().ok();
              child.wait().ok();
            };
            bail!("{:?} changed", evt.paths);
          };
        }],
        [recv(src_change_rx), |evt| {
          if !is_src_changed
            && let Ok(evt) = evt
            && evt.kind.is_modify()
            && evt
              .paths
              .iter()
              .find(|p| p.extension().is_some_and(|ext| ext == "rs"))
              .is_some()
          {
            self.send_state(SourceChanged);
            is_src_changed = true;
          };
        }],
        [default(Duration::from_millis(100)), {
          if is_src_changed {
            if let Some(mut child) = rebuild_task.take() {
              child.kill().ok();
              child.wait().ok();
              continue;
            };
            self.send_state(Rebuilding);
            rebuild_task = self
              .project
              .rebuild_command()
              .stdout(Stdio::null())
              .stderr(Stdio::null())
              .spawn()
              .ok();
            is_src_changed = false;
            continue;
          };

          if let Some(child) = rebuild_task.as_mut() {
            if let Ok(Some(status)) = child.try_wait() {
              if status.success()
                && let Some(version) = self.project.clone_lib()
              {
                restart_count = 2;
                self.send_state(BuildSuccess(version));
              } else {
                self.send_state(BuildFailed);
              }
              rebuild_task.take();
            }
            continue;
          };

          match app_task.as_mut() {
            Some(child) => {
              if let Ok(Some(_)) = child.try_wait() {
                app_task.take();
              }
            }
            None => {
              if restart_count > 0 {
                restart_count -= 1;
                app_task = self
                  .project
                  .bin_target_command()
                  .stdin(Stdio::inherit())
                  .stdout(Stdio::inherit())
                  .stderr(Stdio::inherit())
                  .spawn()
                  .ok();
              }
            }
          }
        }]
      );
    }
  }
}
