use std::{
  env::args,
  path::PathBuf,
  process::{Child, Stdio},
  time::Duration,
};

use crate::hotreload::{
  file_watcher::FileWatcher, hotproject::HotProjectStore, macro_utils::macro_utils::bselect,
};

pub fn app(data_path: &str) {
  let action_run = args().nth(1).unwrap_or("".to_string());
  let data = {
    let path = PathBuf::from(data_path);
    toml::de::from_str::<HotProjectStore>(&std::fs::read_to_string(path).unwrap()).unwrap()
  };

  match action_run.as_str() {
    "--watch" | "-w" => run_watch(data),
    _ => run_app(data),
  };
}

fn run_app(data: HotProjectStore) {
  let project = data.project;
  let version_file = project.files().lib().lib_version_txt_path();
  let mut app_version = project.read_version();
  print_section(&format!(
    "Run cargo run --bin {} -- [--watch/-w] to rebuild on change",
    project.files().bin_name()
  ));
  loop {
    project
      .bin_target_command()
      .stdin(Stdio::inherit())
      .stdout(Stdio::inherit())
      .stderr(Stdio::inherit())
      .spawn()
      .ok()
      .map(|mut child| {
        child.wait().ok()?;
        Some(())
      });

    print_section("Application exited, waiting for changes...");

    let mut watcher = FileWatcher::new();
    watcher.add(version_file.clone(), false);
    let (_, version_rx) = watcher.new_channel();
    watcher.run();

    loop {
      let current_version = project.read_version();
      if current_version != app_version {
        app_version = current_version;
        break;
      }
      version_rx.recv().ok();
    }
  }
}

fn run_watch(data: HotProjectStore) {
  let project = data.project;
  let mut watch_src = FileWatcher::new();
  watch_src.files = data.watch_src;
  let (_, src_change_rx) = watch_src.new_channel();
  watch_src.run();
  let mut is_src_changed = false;
  let mut rebuild_task: Option<Child> = None;
  project.clone_lib();

  loop {
    bselect!(
      [recv(src_change_rx), |evt| {
        evt
          .map(|e| {
            if e.kind.is_modify() && !is_src_changed {
              is_src_changed = true;
            }
          })
          .ok();
      }],
      [default(Duration::from_millis(100)), {
        if is_src_changed {
          if let Some(mut child) = rebuild_task.take() {
            child.kill().ok();
            child.wait().ok();
            continue;
          };
          print_section("REBUILDING...");
          rebuild_task = project
            .rebuild_command()
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .ok();
          is_src_changed = false;
        };

        if let Some(child) = rebuild_task.as_mut() {
          if let Ok(Some(status)) = child.try_wait() {
            status.success().then(|| project.clone_lib());
            rebuild_task.take();
          }
        }
      }]
    );
  }
}

pub fn print_section(title: &str) {
  static LINE: &str = "==============================";
  print!("{}\r\n{}\r\n{}\r\n", LINE, title, LINE);
}
