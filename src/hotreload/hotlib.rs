use anyhow::Result;
use std::{
  collections::BTreeMap,
  panic,
  path::PathBuf,
  sync::{Arc, LazyLock, OnceLock, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use crate::{
  HotLibEvent,
  hotreload::{
    file_watcher::FileWatcher, hotfn::HotFn, hotproject::HotProject,
    macro_utils::macro_utils::bselect,
  },
};

pub struct HotLib {
  pub project: HotProject,
  pub lib: Arc<RwLock<Option<libloading::Library>>>,
  pub functions: Arc<RwLock<Vec<fn()>>>,
  pub functions_dict: BTreeMap<String, u16>,
  pub backup_functions: Vec<fn()>,
  pub tx: crossbeam_channel::Sender<HotLibAction>,
  pub event: Arc<RwLock<HotLibEvent>>,
  pub is_hot_project: bool,
  pub watch_src: BTreeMap<PathBuf, bool>,
  is_configured: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum HotLibAction {
  ReloadLib,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchErr {
  FailedLoadLib,
  FailedCleanLib(String),
  NoGetFunctionsFn,
  ToManyChange,
  OtherError,
}

static DEFAULT_INSTANCE: OnceLock<Arc<Option<Arc<RwLock<HotLib>>>>> = OnceLock::new();

static INSTANCE: LazyLock<Arc<RwLock<HotLib>>> = LazyLock::new(|| {
  DEFAULT_INSTANCE
    .get_or_init(|| Arc::new(None))
    .as_ref()
    .as_ref()
    .cloned()
    .unwrap_or_else(|| Arc::new(RwLock::new(HotLib::new())))
});

impl HotLib {
  pub fn new() -> Self {
    // println!("HotLib::new()");
    Self {
      is_hot_project: false,
      project: HotProject::default(),
      lib: Arc::new(RwLock::new(None)),
      tx: crossbeam_channel::unbounded().0,
      functions_dict: BTreeMap::new(),
      functions: Arc::new(RwLock::new(Vec::new())),
      backup_functions: Vec::new(),
      is_configured: false,
      event: Arc::new(RwLock::new(HotLibEvent::default())),
      watch_src: BTreeMap::new(),
    }
  }
  pub fn rewrite_instance(old_instance: Arc<RwLock<HotLib>>) {
    DEFAULT_INSTANCE.get_or_init(|| Arc::new(Some(old_instance.clone())));
  }
  pub fn get_instance() -> RwLockReadGuard<'static, Self> {
    INSTANCE.read().unwrap()
  }
  pub fn get_instance_mut() -> RwLockWriteGuard<'static, Self> {
    INSTANCE.write().unwrap()
  }
  pub fn on_boot(
    &mut self,
    is_hot_project: bool,
    list_fn: Vec<HotFn>,
    src_file: &str,
    manifest_dir: &str,
  ) {
    if self.is_configured {
      panic!("HotLib is already configured");
    }
    self.is_hot_project = is_hot_project;
    self
      .project
      .config(manifest_dir.to_string(), src_file.to_string());
    self.is_configured = true;

    let mut dict = BTreeMap::new();
    let mut functions = Vec::new();
    let mut idx: u16 = 0;
    for f in list_fn {
      let key = Self::to_key(f.fn_name, f.file_name);
      if dict.contains_key(&key) {
        panic!("Duplicate function name: {}", &key);
      }
      dict.insert(key, idx);
      functions.push(f.func);
      idx += 1;
    }

    self.functions_dict = dict;
    self.backup_functions = functions.clone();
    self.functions = Arc::new(RwLock::new(functions));
  }
  pub fn to_key(fn_name: impl Into<String>, file_name: impl Into<String>) -> String {
    format!("{}:{}", file_name.into(), fn_name.into())
  }
  pub fn trigger(&self, action: HotLibAction) -> Result<()> {
    self.tx.send(action)?;
    Ok(())
  }
  pub fn get_lib(&self, lib_path: PathBuf) -> Result<(libloading::Library, Vec<fn()>), PatchErr> {
    let lib = unsafe { libloading::Library::new(lib_path) }.map_err(|_| PatchErr::FailedLoadLib)?;
    let get_functions = unsafe {
      lib.get::<unsafe extern "C" fn(Arc<RwLock<Self>>) -> Vec<HotFn>>(b"hrl_get_functions")
    }
    .map_err(|_| PatchErr::NoGetFunctionsFn)?;

    let list_fn = unsafe { get_functions(INSTANCE.clone()) };
    let mut vec_fn = self.backup_functions.clone();
    let mut map_fn = self.functions_dict.clone();

    for f in list_fn {
      let key = Self::to_key(f.fn_name, f.file_name);
      if let Some(idx) = map_fn.get(&key) {
        vec_fn[*idx as usize] = f.func;
        map_fn.remove(&key);
      } else {
        lib
          .close()
          .map_err(|e| PatchErr::FailedCleanLib(e.to_string()))?;
        return Err(PatchErr::ToManyChange);
      }
    }

    if !map_fn.is_empty() {
      lib
        .close()
        .map_err(|e| PatchErr::FailedCleanLib(e.to_string()))?;
      return Err(PatchErr::ToManyChange);
    }

    return Ok((lib, vec_fn));
  }
  pub fn apply_lib(&self, lib: libloading::Library, list_fn: Vec<fn()>) -> Option<PatchErr> {
    self
      .event
      .read()
      .unwrap()
      .on_clean_up
      .iter()
      .for_each(|f| f());

    let old_lib = self.lib.write().unwrap().take();
    if let Some(e) = old_lib.and_then(|lib| lib.close().err()) {
      return Some(PatchErr::FailedCleanLib(e.to_string()));
    }
    *self.functions.write().unwrap() = list_fn;
    self.lib.write().unwrap().replace(lib);
    None
  }
  pub fn run_watch_lib(&mut self) {
    let (tx, hot_rx) = crossbeam_channel::unbounded();
    let lib_version = self.project.files().lib().lib_version_txt_path();
    self.tx = tx.clone();
    std::thread::spawn({
      let tx = tx.clone();
      move || {
        let mut watch_lib = FileWatcher::new();
        watch_lib.add(lib_version.clone(), false);
        let (_, watch_build_rx) = watch_lib.new_channel();
        watch_lib.run();
        loop {
          bselect!(
            [recv(watch_build_rx), |evt| {
              if let Ok(event) = evt {
                if event.kind.is_modify() {
                  tx.send(HotLibAction::ReloadLib).ok();
                }
              }
            }],
            [recv(hot_rx), |action| {
              action.map(|action| {
                match action {
                  HotLibAction::ReloadLib => {
                    let version = std::fs::read_to_string(&lib_version)
                      .ok()
                      .and_then(|v| v.parse::<u128>().ok())?;
                    let lib_path = Self::get_instance()
                      .project
                      .files()
                      .lib()
                      .lib_version_path(version);
                    let evt = Self::get_instance().event.clone();
                    evt.read().unwrap().on_pre_patch.iter().for_each(|f| f());
                    match Self::get_instance().get_lib(lib_path) {
                      Ok((lib, list_fn)) => {
                        if let Some(_) = Self::get_instance().apply_lib(lib, list_fn) {
                          std::process::exit(0);
                        }
                        evt
                          .read()
                          .unwrap()
                          .on_patch_success
                          .iter()
                          .for_each(|f| f());
                      }
                      Err(err) => match err {
                        PatchErr::FailedLoadLib
                        | PatchErr::NoGetFunctionsFn
                        | PatchErr::OtherError => {
                          evt
                            .read()
                            .unwrap()
                            .on_patch_error
                            .iter()
                            .for_each(|f| f(err.clone()));
                        }
                        PatchErr::ToManyChange | PatchErr::FailedCleanLib(_) => {
                          std::process::exit(0);
                        }
                      },
                    };
                  }
                };
                Some(())
              }).ok();
            }]
          );
        }
      }
    });
  }
}

pub fn get_fn_idx(fn_name: &'static str, file_name: &'static str) -> u16 {
  let i = HotLib::get_instance();
  let key = HotLib::to_key(fn_name, file_name);
  let idx = *i.functions_dict.get(&key).unwrap();
  idx
}

pub fn get_fn_list<T>() -> Arc<RwLock<Vec<T>>> {
  unsafe { std::mem::transmute(HotLib::get_instance().functions.clone()) }
}
