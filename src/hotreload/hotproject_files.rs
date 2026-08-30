use std::path::PathBuf;

use crate::hotreload::hotproject::HotProject;

trait FileRoot<'a> {
  fn files(&'a self) -> HotProjectFiles<'a>;
}

macro_rules! use_prefix {
  ($name: ident {
    $($mname: ident : |$($arg_name: ident: $arg_type: ty $(,)?)*| $(-> $ret: ty)? $value: block,)*
  }$(,[$methodw:ident, $typew:ty]$(,)?)*) => {
    paste::paste! {
      pub struct $name<'a> {
        project: &'a HotProject,
      }
      impl<'a> FileRoot<'a> for $name<'a> {
        fn files(&'a self) -> HotProjectFiles<'a> {
          HotProjectFiles { project: self.project }
        }
      }
      impl<'a> $name<'a> {
        $(
          pub fn $methodw(&self) -> $typew<'a> {
            $typew { project: self.project }
          }
        )*
      }
      impl<'a> $name<'a> {
        $(
          pub fn $mname($($arg_name: $arg_type,)*)
            $(->$ret)? $value
        )*
      }
    }
  };
}

impl<'a> HotProjectFiles<'a> {
  pub fn new(project: &'a HotProject) -> Self {
    Self { project }
  }
}

use_prefix!(
  HotProjectFiles {
    target_dir: |self: &Self| -> PathBuf { self.project.bin_path.parent().unwrap().to_path_buf() },
    data_dir: |self: &Self| -> PathBuf { self.target_dir().join("hotfnl").join(self.bin_name()) },
    bin_name: |self: &Self| -> String {
      self
        .project
        .bin_path
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .to_string()
    },
  },
  [src, HotSrc],
  [data, HotData],
  [wrapper, HotWrapperBin],
  [lib, HotTargetLib],
  [hotbin, HotTargetHotBin]
);

use_prefix!(HotSrc {
  cargo_toml: |self: &Self| -> PathBuf { self.project.hot_dir.join("Cargo.toml") },
  cargo_lock: |self: &Self| -> PathBuf { self.project.hot_dir.join("Cargo.lock") },
  cargo_config_dir: |self: &Self| -> PathBuf { self.project.hot_dir.join(".cargo") },
  cargo_config_file: |self: &Self| -> PathBuf { self.cargo_config_dir().join("config.toml") },
});

use_prefix!(HotWrapperBin {
  name: |self: &Self| -> String { format!("hotfnlw_{}", self.files().bin_name()) },
  src_name: |self: &Self| -> String { format!("wrapper.rs") },
  src_path: |self: &Self| -> PathBuf { self.project.hot_dir.join(self.src_name()) },
  bin_path: |self: &Self| -> PathBuf { self.files().target_dir().join(self.name()) },
});
use_prefix!(HotTargetLib {
  name: |self: &Self| -> String { format!("hotfnl_{}", self.files().bin_name()) },
  out_name: |self: &Self| -> String { format!("lib{}.so", self.name()) },
  out_path: |self: &Self| -> PathBuf { self.files().target_dir().join(self.out_name()) },
  lib_clone_dir: |self: &Self| -> PathBuf { self.files().data_dir().join("lib") },
  lib_version_path: |self: &Self, build_time: u128| -> PathBuf {
    self.lib_clone_dir().join(format!("lib_{}.so", build_time))
  },
  lib_version_txt_path: |self: &Self| -> PathBuf {
    self.files().data_dir().join("lib_version.txt")
  },
});

use_prefix!(HotTargetHotBin {
  name: |self: &Self| -> String { format!("hotfnl_{}", self.files().bin_name()) },
  out_name: |self: &Self| -> String { format!("hotfnl_{}", self.files().bin_name()) },
  out_path: |self: &Self| -> PathBuf { self.files().target_dir().join(self.out_name()) },
});

use_prefix!(HotData {
  log_path: |self: &Self| -> PathBuf { self.files().data_dir().join("hotfnl.log") },
  project_data_path: |self: &Self| -> PathBuf { self.files().data_dir().join("project_data.toml") },
});
