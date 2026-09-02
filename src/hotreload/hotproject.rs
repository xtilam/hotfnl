//! Hot-project scaffolding: generates the Cargo project, builds it, and holds path and
//! rebuild logic for the hot-reload machinery.

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

/// Metadata describing the generated hot project and how it maps to the user's project.
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct HotProject {
  /// Root directory of the user's crate.
  pub root_dir: PathBuf,
  /// Directory where the generated hot project lives.
  pub hot_dir: PathBuf,
  /// Absolute path to the user's main source file.
  pub src_path: PathBuf,
  /// The workspace root directory.
  pub workspace_dir: PathBuf,
  /// Path to the user's application binary.
  pub bin_path: PathBuf,
  /// Whether the user's crate is part of a Cargo workspace.
  pub is_workspace: bool,
}

/// Persisted state of the hot project, written to disk between runs.
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct HotProjectStore {
  pub project: HotProject,
  pub watch_src: BTreeMap<PathBuf, bool>,
}

impl HotProject {
  /// Computes the project layout from the crate root and source path.
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
  /// Rewrites the workspace `Cargo.toml` used by the hot project, renaming the package
  /// and rewriting path dependencies to be absolute.
  pub fn write_cargo_workspace(&self) -> Result<()> {
    use toml::{Value, from_str};
    let mut cargo: Value = {
      let content = std::fs::read_to_string(self.workspace_dir.join("Cargo.toml"))?;
      from_str(&content).unwrap()
    };

    cargo.as_table_mut().map(|t| {
      t.remove("lib");
      t.remove("bin");
      t.get_mut("package")
        .and_then(|v| v.as_table_mut())
        .map(|v| {
          let name = v.get("name");
          v.insert(
            "name".into(),
            format!("hotfnl_{}", name.unwrap().as_str().unwrap()).into(),
          );
        });
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
      .and_then(|v| v.as_table_mut())
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
  /// Rewrites the user's `Cargo.toml` for the hot project: it configures the crate to
  /// build both the hot binary and the dynamic library, injects the wrapper binary, and
  /// rewrites path dependencies to be absolute.
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
  /// Scaffolds the entire hot project on disk: creates directories, the rewrite
  /// `Cargo.toml`, the wrapper source, build script, and persisted project data.
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
        r#"fn main() {{ println!("cargo:rustc-env=HOT_PROJECT_DIR={}"); }}"#,
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

  /// Builds a `Command` that runs the wrapper binary via `cargo run --bin
  /// <wrapper> -- <args>`.
  pub fn wrapper_command(&self, command_args: Option<Vec<String>>) -> Command {
    let mut command = Command::new("cargo");
    let arg = command_args.unwrap_or_else(|| args().skip(1).collect());
    command
      .args(&["run", "--bin", self.files().wrapper().name().as_str(), "--"])
      .args(arg)
      .current_dir(&self.hot_dir);
    command
  }

  /// Builds a `Command` that runs the hot application binary directly.
  pub fn bin_target_command(&self) -> Command {
    let mut command = Command::new(self.files().hotbin().out_path());
    command.current_dir(&self.files().target_dir());
    command
  }

  /// Builds a `Command` that compiles the hot project with `cargo build`.
  pub fn rebuild_command(&self) -> Command {
    let mut command = Command::new("cargo");
    command.args(&["build"]).current_dir(&self.hot_dir);
    command
  }

  /// Runs `cargo build` in the hot project and waits for completion.
  pub fn rebuild(&self) -> Result<()> {
    let mut child = self.rebuild_command().spawn()?;
    let code = child.wait()?;
    code.success().then_some(0).context("rebuild failed")?;
    Ok(())
  }

  /// Returns the computed hot-project file layout.
  pub fn files(&self) -> HotProjectFiles<'_> {
    HotProjectFiles::new(self)
  }

  /// Reads the current library version number (defaults to `0` if unreadable).
  pub fn read_version(&self) -> u128 {
    std::fs::read_to_string(self.files().lib().lib_version_txt_path())
      .ok()
      .and_then(|s| s.trim().parse::<u128>().ok())
      .unwrap_or(0)
  }

  /// Snapshots the built library into a versioned clone and updates the version file.
  ///
  /// Silently ignores I/O failures; used after a successful rebuild in the watch loop.
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

/// Configuration for additional source paths to watch for changes.
///
/// Obtained via the `watch!` macro. Paths added here are watched by the rebuild loop so
/// that edits trigger a reload.
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

  /// Watches `path` (non-recursively) for changes.
  pub fn watch(&self, path: &str) -> &Self {
    self.add_watch(path, false)
  }

  /// Watches `path` recursively for changes.
  pub fn recursive(&self, path: &str) -> &Self {
    self.add_watch(path, true)
  }
}
