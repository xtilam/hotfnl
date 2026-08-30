use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env::args, path::PathBuf};

use crate::hotreload::fs_utils::clean_path;
use crate::{
  HotLib,
  hotreload::{
    fs_utils::{bin_name, link_file, write_file},
    hotproject_files::HotProjectFiles,
  },
};

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct HotProject {
  pub root_dir: PathBuf,
  pub hot_dir: PathBuf,
  pub src_path: PathBuf,
  pub workspace_dir: PathBuf,
  pub bin_path: PathBuf,
  pub is_workspace: bool,
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct HotProjectStore {
  pub project: HotProject,
  pub watch_src: BTreeMap<PathBuf, bool>,
}

impl HotProject {
  pub fn config(&mut self, root_dir: String, src_path: String) {
    let root_dir = PathBuf::from(root_dir);
    let mut bin_path = std::env::current_exe().expect("No current exe path");
    let mut name = bin_name(&bin_path);
    if name.starts_with("hotfnl_") {
      name = name.strip_prefix("hotfnl_").unwrap().to_string();
      bin_path = bin_path.parent().unwrap().join(name.clone());
    }
    self.workspace_dir = bin_path.as_path().ancestors().nth(3).unwrap().to_path_buf();
    self.root_dir = root_dir.clone();
    self.hot_dir = bin_path
      .parent()
      .unwrap()
      .parent()
      .unwrap()
      .join(format!("hotfnl/{}", name));
    self.src_path = self.workspace_dir.join(src_path);
    self.bin_path = bin_path;
    self.is_workspace = self.root_dir != self.workspace_dir;
  }
  pub fn write_cargo_workspace(&self) -> Result<()> {
    use toml::{Value, from_str};
    let mut cargo: Value = {
      let content = std::fs::read_to_string(self.workspace_dir.join("Cargo.toml"))?;
      from_str(&content).unwrap()
    };

    cargo.as_table_mut().map(|t| {
      t.remove("lib");
      t.remove("bin");
    });
    cargo
      .get_mut("workspace")
      .and_then(|v| v.get_mut("members"))
      .and_then(|v| {
        v.is_array()
          .then(|| *v = toml::Value::Array(vec![format!("./{}", self.files().bin_name()).into()]))
      });

    cargo
      .get_mut("workspace")
      .and_then(|v| v.get_mut("dependencies"))
      .and_then(|v| v.is_table().then(|| v.as_table_mut().unwrap()))
      .and_then(|v| {
        Some(v.iter_mut().for_each(|item| {
          item.1.get_mut("path").and_then(|v| {
            v.is_str().then(|| {
              *v = toml::Value::String(
                self
                  .workspace_dir
                  .join(v.as_str().unwrap())
                  .to_string_lossy()
                  .to_string(),
              )
            })
          });
        }))
      });

    cargo
      .get_mut("dependencies")
      .and_then(|v| v.is_table().then(|| v.as_table_mut().unwrap()))
      .and_then(|v| {
        Some(v.iter_mut().for_each(|item| {
          item.1.get_mut("path").and_then(|v| {
            v.is_str().then(|| {
              *v = toml::Value::String(
                self
                  .workspace_dir
                  .join(v.as_str().unwrap())
                  .to_string_lossy()
                  .to_string(),
              )
            })
          });
        }))
      });
    write_file(
      &self.files().workspace().cargo_toml(),
      toml::to_string(&cargo)?.as_str(),
    )?;
    write_file(
      &self.files().workspace().main_rs(),
      "fn main() { println!(\"Hello, world!\"); }",
    )?;
    Ok(())
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
      .and_then(|v| v.get_mut("dependencies"))
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
      let src_path = self.src_path.to_string_lossy();
      let lib = {
        let mut lib = Map::new();
        let name = self.files().lib().name();
        lib.insert(
          "crate-type".into(),
          toml::Value::Array(vec!["cdylib".into(), "rlib".into()]),
        );
        lib.insert("path".into(), src_path.to_string().into());
        lib.insert("name".into(), name.into());
        lib
      };
      let bin: toml::value::Array = vec![
        {
          let mut bin = Map::new();
          bin.insert("name".into(), self.files().hotbin().name().into());
          bin.insert("path".into(), src_path.to_string().into());
          bin.into()
        },
        {
          let mut bin = Map::new();
          let wrapper = self.files().wrapper();
          bin.insert("name".into(), wrapper.name().into());
          bin.insert(
            "path".into(),
            wrapper.src_path().to_string_lossy().to_string().into(),
          );
          bin.into()
        },
      ];

      t.insert("lib".into(), lib.into());
      t.insert("bin".into(), bin.into());
    });

    write_file(
      &self.hot_dir.join("Cargo.toml"),
      toml::to_string(&cargo)?.as_str(),
    )?;

    Ok(())
  }
  pub fn init_hot_project(&self) -> Result<()> {
    std::fs::create_dir_all(&self.files().lib().lib_clone_dir())?;
    std::fs::create_dir_all(&self.files().workspace().cargo_config_dir())?;
    std::fs::create_dir_all(&self.hot_dir)?;

    write_file(
      &self.files().workspace().cargo_config_file(),
      format!(
        "[build]\ntarget-dir = \"{}\"",
        self
          .files()
          .target_dir()
          .parent()
          .unwrap()
          .to_string_lossy()
      )
      .as_str(),
    )?;

    write_file(&self.files().lib().lib_version_txt_path(), "")?;
    write_file(
      &self.hot_dir.join("build.rs"),
      &format!(
        r#" fn main() {{ println!("cargo:rustc-env=HOT_PROJECT_DIR={}"); }}"#,
        self.root_dir.to_string_lossy()
      ),
    )?;

    write_file(
      &self.files().data().project_data_path(),
      toml::to_string(&HotProjectStore {
        project: self.clone(),
        watch_src: HotLib::get_instance().watch_src.clone(),
      })
      .unwrap()
      .as_str(),
    )?;
    write_file(
      &self.files().wrapper().src_path(),
      &format!(
        r#"fn main() {{ hotfnl::app({:?}); }}"#,
        self.files().data().project_data_path()
      ),
    )?;

    self.write_cargo_toml()?;
    write_file(&self.files().data().log_path(), "")?;

    {
      let root_lock = self.root_dir.join("Cargo.lock");
      std::fs::exists(&root_lock).map_or(Ok(()), |is_exists| {
        is_exists
          .then(|| link_file(&root_lock, &self.hot_dir.join("Cargo.lock")))
          .unwrap_or(Ok(()))
      })
    }?;

    if self.is_workspace {
      let workspace_lock = self.workspace_dir.join("Cargo.lock");
      std::fs::exists(&workspace_lock).map_or(Ok(()), |is_exists| {
        is_exists
          .then(|| {
            link_file(
              &workspace_lock,
              &self.hot_dir.parent().unwrap().join("Cargo.lock"),
            )
          })
          .unwrap_or(Ok(()))
      })?;
      self.write_cargo_workspace()?;
    }
    Ok(())
  }

  pub fn wrapper_command(&self, command_args: Option<Vec<String>>) -> Command {
    let mut command = Command::new("cargo");
    let arg = command_args.unwrap_or_else(|| args().skip(1).collect());
    command
      .args(&["run", "--bin", self.files().wrapper().name().as_str(), "--"])
      .args(arg)
      .current_dir(&self.hot_dir);
    command
  }
  pub fn log_command(&self) -> Command {
    let mut command = Command::new("tail");
    command
      .args(&["-f", self.files().data().log_path().to_str().unwrap()])
      .current_dir(&self.hot_dir);
    command
  }
  pub fn bin_command(&self) -> Command {
    let mut command = Command::new("cargo");
    command
      .args(&["run", "--bin", self.files().hotbin().name().as_str(), "--"])
      .args(args().skip(1))
      .current_dir(&self.hot_dir);
    command
  }
  pub fn bin_target_command(&self) -> Command {
    let mut command = Command::new(self.files().hotbin().out_path());
    command.current_dir(&self.files().target_dir());
    command
  }
  pub fn rebuild_command(&self) -> Command {
    let mut command = Command::new("cargo");
    command.args(&["build"]).current_dir(&self.hot_dir);
    command
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
    let mut child = self.rebuild_command().spawn()?;
    let code = child.wait()?;
    code.success().then_some(0).context("rebuild failed")?;
    Ok(())
  }
  pub fn files(&self) -> HotProjectFiles<'_> {
    HotProjectFiles::new(self)
  }
  pub fn read_version(&self) -> u128 {
    std::fs::read_to_string(self.files().lib().lib_version_txt_path())
      .ok()
      .and_then(|s| s.trim().parse::<u128>().ok())
      .unwrap_or(0)
  }
  pub fn write_version(&self, version: u128) -> Result<()> {
    write_file(
      &self.files().lib().lib_version_txt_path(),
      &version.to_string(),
    )
  }
  pub fn clone_lib(&self) {
    let build_version = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap()
      .as_millis();
    std::fs::remove_dir_all(self.files().lib().lib_clone_dir()).ok();
    std::fs::create_dir_all(self.files().lib().lib_clone_dir()).ok();
    std::fs::rename(
      self.files().lib().out_path(),
      self.files().lib().lib_version_path(build_version),
    )
    .ok()
    .and_then(|_| {
      std::fs::write(
        self.files().lib().lib_version_txt_path(),
        build_version.to_string(),
      )
      .ok()
    });
  }
}

pub struct HotProjectWatcherConfig {}
impl HotProjectWatcherConfig {
  fn add_watch(&self, path: &str, recursive: bool) -> &Self {
    let path = clean_path(
      Path::new(&HotLib::get_instance().project.root_dir)
        .join(path)
        .as_path(),
    );
    HotLib::get_instance_mut().watch_src.insert(path, recursive);
    self
  }
  pub fn watch(&self, path: &str) -> &Self {
    self.add_watch(path, false)
  }
  pub fn recursive(&self, path: &str) -> &Self {
    self.add_watch(path, true)
  }
  pub fn use_self_rebuild(&self) -> &Self {
    self
  }
}
