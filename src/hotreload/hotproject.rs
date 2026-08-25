use anyhow::{Context, Result};
use crossterm::terminal::disable_raw_mode;
use crossterm::{execute, terminal};
use notify::event::CreateKind;
use notify::{EventKind, Watcher};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

use crate::hotreload::fs_utils::{bin_name, link_file, write_file};
use crate::hotreload::hotlib::{HotLib, HotLibAction};
#[derive(Default)]
pub struct HotProject {
  pub root_dir: PathBuf,
  pub hot_dir: PathBuf,
  pub src_path: PathBuf,
  pub bin_path: PathBuf,
  pub is_watch_mode: bool,
  pub version: u16,
}

impl HotProject {
  pub fn new(root_dir: String, src_path: String) -> Self {
    let root_dir = PathBuf::from(root_dir);
    let bin_path = std::env::current_exe().expect("No current exe path");
    Self {
      root_dir: root_dir.clone(),
      hot_dir: root_dir
        .join("target")
        .join(format!("hotfnl/{}", bin_name(&bin_path))),
      src_path: root_dir.join(src_path),
      bin_path,
      is_watch_mode: false,
      version: 1,
    }
  }
  pub fn write_cargo_toml(&self) -> Result<()> {
    use toml::{Value, from_str, map::Map};
    let mut cargo: Value = {
      let content = std::fs::read_to_string(self.root_dir.join("Cargo.toml"))?;
      from_str(&content).unwrap()
    };
    cargo
      .get_mut("dependencies")
      .and_then(|v| v.is_table().then(|| v.as_table_mut().unwrap()))
      .and_then(|v| {
        Some(v.iter_mut().for_each(|item| {
          item.1.get_mut("path").and_then(|v| {
            v.is_str().then(|| {
              *v = toml::Value::String(
                self
                  .root_dir
                  .join(v.as_str().unwrap())
                  .to_string_lossy()
                  .to_string(),
              )
            })
          });
        }))
      });

    cargo
      .get_mut("workspace")
      .and_then(|v| v.get_mut("members"))
      .and_then(|v| v.is_array().then(|| *v = toml::Value::Array(vec![])));

    cargo.as_table_mut().map(|t| {
      t.remove("lib");
      let src_path = self.src_path.to_string_lossy().to_string();
      let mut map = Map::new();
      let mut bin_hot = Map::new();
      bin_hot.insert("name".into(), self.hot_bin_name().into());
      bin_hot.insert("path".into(), src_path.clone().into());
      let bin: toml::value::Array = vec![bin_hot.into()];
      map.insert(
        "crate-type".into(),
        toml::Value::Array(vec!["cdylib".into(), "rlib".into()]),
      );
      map.insert("path".into(), src_path.into());
      let name = self.lib_name(0);
      let v = &name[3..name.len() - 3];
      map.insert("name".into(), v.to_string().into());
      t.insert("lib".into(), map.into());
      t.insert("bin".into(), bin.into());
    });

    write_file(
      &self.hot_dir.join("Cargo.toml"),
      toml::to_string(&cargo)?.as_str(),
    )?;
    Ok(())
  }
  pub fn init_hot_project(&self) -> Result<()> {
    let cargo_config_path = self.hot_dir.join(".cargo/config.toml");
    let target_dir = self.target_dir();
    write_file(
      &cargo_config_path,
      format!("[build]\ntarget-dir = \"{}\"", target_dir.to_string_lossy()).as_str(),
    )?;

    write_file(
      &self.hot_dir.join("build.rs"),
      &format!(
        r#"
fn main() {{
  println!("cargo:rustc-env=ROOT_PROJECT={}");
}}"#,
        self.root_dir.to_string_lossy()
      ),
    )?;
    write_file(
      &self.wait_rebuild_path(),
      &format!(
        r#"
fn main() {{
  let data = {:?};
  let _ = hotfnl::app(data);
}}"#,
        toml::ser::to_string(&self).unwrap().as_str()
      ),
    )?;

    link_file(
      &self.root_dir.join("Cargo.lock"),
      &self.hot_dir.join("Cargo.lock"),
    )?;

    self.write_cargo_toml()?;
    write_file(&self.log_path(), "")?;
    Ok(())
  }
  pub fn target_dir(&self) -> PathBuf {
    self
      .bin_path
      .parent()
      .unwrap()
      .parent()
      .unwrap()
      .to_path_buf()
  }
  pub fn wait_rebuild_path(&self) -> PathBuf {
    self
      .hot_dir
      .join("src/bin")
      .join(format!("wait_rebuild_{}.rs", self.bin_name()))
  }
  pub fn lib_name(&self, version: u16) -> String {
    format!("lib{}_{}_hotfnl.so", self.bin_name(), version)
  }
  pub fn log_path(&self) -> PathBuf {
    self
      .bin_path
      .parent()
      .unwrap()
      .join(format!("{}_hotfnl.log", self.bin_name()))
  }
  pub fn lib_cur_ver_path(&self) -> PathBuf {
    self
      .bin_path
      .parent()
      .unwrap()
      .join(self.lib_name(self.version))
  }
  pub fn move_lib(&self) -> PathBuf {
    let cur_lib_path = self.lib_cur_ver_path();
    let lib_path = self.lib_path();
    if lib_path.exists() {
      std::fs::remove_file(cur_lib_path.as_path()).ok();
      std::fs::rename(lib_path, cur_lib_path.as_path()).ok();
    };
    cur_lib_path
  }
  pub fn lib_path_version(&self, version: u16) -> PathBuf {
    self.bin_path.parent().unwrap().join(self.lib_name(version))
  }
  pub fn lib_path(&self) -> PathBuf {
    self.lib_path_version(0)
  }
  pub fn lib_clone(&self) -> PathBuf {
    self.lib_path_version(self.version)
  }
  pub fn hot_bin_name(&self) -> String {
    format!("{}_hotfnl", self.bin_name())
  }
  pub fn watch_build(&self) -> Result<()> {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = notify::recommended_watcher(tx)?;
    self.watch_src.iter().for_each(|src| {
      watcher
        .watch(src, notify::RecursiveMode::Recursive)
        .unwrap();
    });
    std::thread::spawn(move || {
      for res in &rx {
        match res {
          Ok(event) => {
            matches!(event.kind, EventKind::Modify(_)).then(|| {
              HotLib::get_instance().trigger(HotLibAction::Rebuild).ok();
            });
          }
          Err(e) => println!("watch error: {:?}", e),
        }
      }
      drop(watcher);
    });
    Ok(())
  }
  pub fn watch_lib(&self) -> Result<()> {
    let lib_path = self.lib_path();
    let target_dir = lib_path.parent().unwrap().to_path_buf();
    println!("watch_lib: {:?}", lib_path);
    std::fs::remove_file(&lib_path).ok();
    std::thread::spawn(move || {
      let (tx, rx) = std::sync::mpsc::channel();
      let mut watcher = notify::recommended_watcher(tx).unwrap();
      watcher
        .watch(&target_dir, notify::RecursiveMode::NonRecursive)
        .unwrap();
      for res in &rx {
        match res {
          Ok(event) => {
            matches!(event.kind, EventKind::Create(CreateKind::File)).then(|| {
              event.paths.into_iter().for_each(|p| {
                if p == lib_path {
                  HotLib::get_instance().trigger(HotLibAction::ReloadLib).ok();
                }
              })
            });
          }
          Err(e) => println!("watch error: {:?}", e),
        }
      }
    });
    Ok(())
  }
  pub fn run_wait_rebuild(&self) {
    let _ = Command::new("cargo")
      .args(&[
        "run",
        "--bin",
        self
          .wait_rebuild_path()
          .file_stem()
          .unwrap()
          .to_str()
          .unwrap(),
      ])
      .current_dir(&self.hot_dir)
      .exec();
  }
  pub fn rebuild_lib(&self) -> Result<()> {
    let mut child = Command::new("cargo")
      .args(&["build", "--lib"])
      .current_dir(&self.hot_dir)
      .spawn()?;
    let code = child.wait()?;
    code.success().then_some(0).context("rebuild lib failed")?;
    Ok(())
  }
  pub fn rebuild(&self) -> Result<()> {
    let mut child = Command::new("cargo")
      .args(&["build"])
      .current_dir(&self.root_dir)
      .spawn()?;
    let code = child.wait()?;
    code.success().then_some(0).context("rebuild failed")?;
    Ok(())
  }
  pub fn restart(&self) {
    disable_raw_mode().ok();
    execute!(std::io::stdout(), terminal::LeaveAlternateScreen).ok();

    let _ = Command::new("cargo")
      .args(&["run", self.src_path.to_str().unwrap()])
      .current_dir(&self.root_dir)
      .exec();
  }
  pub fn run(self) -> Result<()> {
    self.init_hot_project()?;
    self.watch_lib()?;
    self.watch_build()?;
    {
      let mut i = HotLib::get_instance_mut();
      i.project = self;
    }
    WatchTask::new().run();
    Ok(())
  }
  pub fn bin_name(&self) -> String {
    self
      .bin_path
      .file_stem()
      .unwrap()
      .to_str()
      .unwrap()
      .to_string()
  }
  pub fn watch_src(&mut self, into: String) {
    let path = self.root_dir.join(into);
    if path.exists() {
      self.watch_src.push(path);
    } else {
      println!("watch_src: {:?} not exists", path);
    }
  }
}
