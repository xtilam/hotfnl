use std::path::{Path, PathBuf};

macro_rules! name_fn {
  ($name: ident, |$p: ident $(,$($an: ident: $at: ty $(,)?)*)?| $($body:tt)*) => {
    #[allow(dead_code)]
    pub fn $name($p: &crate::hotreload::hotproject::HotProject $($(,$an: $at)*)?) -> String {
      $($body)*
    }
  }
}
macro_rules! file_fn {
  ($name: ident, |$p: ident $(,$($an: ident: $at: ty $(,)?)*)?| $($body:tt)*) => {
    #[allow(dead_code)]
    pub fn $name($p: &crate::hotreload::hotproject::HotProject $($(,$an: $at)*)?) -> PathBuf {
      $($body)*
    }
  }
}
macro_rules! cfn {
  ($name: ident, |$(_)? $($an: ident: $at: ty $(,)?)*| $(-> $ret: ty)? { $($body:tt)* }) => {
    #[allow(dead_code)]
    pub fn $name($($an: $at)*) $(-> $ret)? {
      $($body)*
    }
  }
}

fn to_name(path: &Path) -> String {
  path.file_stem().unwrap().to_string_lossy().to_string()
}

name_fn!(bin_name, |p| to_name(&p.bin_path));
file_fn!(target_dir, |p| p.bin_path.parent().unwrap().to_path_buf());
file_fn!(data_dir, |p| target_dir(p).join("hotfnl").join(bin_name(p)));

pub mod wrapper {
  use super::*;
  name_fn!(name, |p| format!("hotfnlw_{}", bin_name(p)));
  cfn!(src_name, |_| -> String { "wrapper.rs".to_string() });
  file_fn!(src_path, |p| p.hot_dir.join(src_name()));
  file_fn!(bin_path, |p| target_dir(p).join(name(p)));
}

pub mod lib {
  use super::*;
  name_fn!(name, |p| format!("hotfnl_{}", bin_name(p)));
  name_fn!(out_name, |p| format!("lib{}.so", name(p)));
  file_fn!(out_path, |p| target_dir(p).join(out_name(p)));
  file_fn!(lib_clone_dir, |p| data_dir(p).join("lib"));
  file_fn!(lib_version_path, |p, build_time: u128| {
    lib_clone_dir(p).join(format!("lib_{}.so", build_time))
  });
}

pub mod hotbin {
  use super::*;
  name_fn!(name, |p| format!("hotfnl_{}", bin_name(p)));
  name_fn!(out_name, |p| format!("hotfnl_{}", bin_name(p)));
  file_fn!(out_path, |p| target_dir(p).join(out_name(p)));
}

pub mod data {
  use super::*;
  file_fn!(log_path, |p| data_dir(p).join("hotfnl.log"));
  file_fn!(project_data_path, |p| data_dir(p).join("project_data.toml"));
  file_fn!(project_state_path, |p| data_dir(p).join("project_state.toml"));
  file_fn!(project_sock_path, |p| data_dir(p).join("project.sock"));
}

pub mod src {
  use super::*;
  file_fn!(cargo_toml, |p| p.hot_dir.join("Cargo.toml"));
  file_fn!(cargo_lock, |p| p.hot_dir.join("Cargo.lock"));
  file_fn!(cargo_config_dir, |p| {
    let mut path = p.hot_dir.clone();
    if p.is_workspace {
      path = path.parent().unwrap().to_path_buf();
    }
    path.join(".cargo")
  });
  file_fn!(cargo_config_file, |p| cargo_config_dir(p).join("config.toml"));
}

pub mod workspace {
  use super::*;
  file_fn!(dir, |p| {
    if p.is_workspace {
      p.hot_dir.parent().unwrap().to_path_buf()
    } else {
      p.hot_dir.clone()
    }
  });
  file_fn!(cargo_toml, |p| dir(p).join("Cargo.toml"));
  file_fn!(cargo_lock, |p| dir(p).join("Cargo.lock"));
  file_fn!(cargo_config_dir, |p| dir(p).join(".cargo"));
  file_fn!(cargo_config_file, |p| cargo_config_dir(p).join("config.toml"));
  file_fn!(main_rs, |p| dir(p).join("src").join("main.rs"));
}