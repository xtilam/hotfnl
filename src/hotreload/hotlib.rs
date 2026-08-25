use anyhow::Result;
use std::{
  collections::BTreeMap,
  panic,
  sync::{Arc, LazyLock, RwLock, RwLockReadGuard, RwLockWriteGuard, mpsc},
};

use crate::hotreload::{
  event::HotLibEvent, file_watcher::FileWatcher, hotfn::HotFn, hotproject::HotProject,
  watch_task::WatchTask,
};

pub struct HotLib {
  pub project: HotProject,
  pub lib: Arc<RwLock<Option<libloading::Library>>>,
  pub functions: Arc<RwLock<Vec<fn()>>>,
  pub functions_dict: BTreeMap<String, u16>,
  pub backup_functions: Vec<fn()>,
  pub tx: mpsc::Sender<HotLibAction>,
  pub event: Arc<RwLock<HotLibEvent>>,
  pub watch_src: FileWatcher,
  pub watch_lib: FileWatcher,

  is_configured: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum HotLibAction {
  ReloadLib,
  Rebuild,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchErr {
  FailedLoadLib,
  FailedCleanLib(String),
  NoGetFunctionsFn,
  ToManyChange,
  OtherError,
}

static INSTANCE: LazyLock<Arc<RwLock<HotLib>>> =
  LazyLock::new(|| Arc::new(RwLock::new(HotLib::new())));

impl HotLib {
  pub fn new() -> Self {
    let tx = WatchTask::new().run();

    Self {
      project: HotProject::default(),
      lib: Arc::new(RwLock::new(None)),
      tx,
      functions_dict: BTreeMap::new(),
      functions: Arc::new(RwLock::new(Vec::new())),
      backup_functions: Vec::new(),
      is_configured: false,
      event: Arc::new(RwLock::new(HotLibEvent::default())),
      watch_src: FileWatcher::new(),
      watch_lib: FileWatcher::new(),
    }
  }
  pub fn rewrite_instance(old_instance: &HotLib) {
    let mut instance = Self::get_instance_mut();
    instance.project = old_instance.project.clone();
    instance.functions_dict = old_instance.functions_dict.clone();
    instance.functions = old_instance.functions.clone();
  }
  pub fn get_instance() -> RwLockReadGuard<'static, Self> {
    INSTANCE.read().unwrap()
  }
  pub fn get_instance_mut() -> RwLockWriteGuard<'static, Self> {
    INSTANCE.write().unwrap()
  }
  pub fn on_boot(&mut self, list_fn: Vec<HotFn>, manifest_dir: &str, src_file: &str) {
    if self.is_configured {
      panic!("HotLib is already configured");
    }
    self.project = HotProject::new(manifest_dir.to_string(), src_file.to_string());
    self.is_configured = true;

    let mut dict = BTreeMap::new();
    let mut functions = Vec::new();
    let mut idx: u16 = 0;
    for f in list_fn {
      let key = Self::to_key(f.fn_name, f.file_name);
      if dict.contains_key(&key) {
        panic!("Duplicate function name: {}", &key);
      }
      println!("config: {} => {} => {:?}", idx, &key, f.func);
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
  pub fn get_lib(&self) -> Result<(libloading::Library, Vec<fn()>), PatchErr> {
    let lib = unsafe { libloading::Library::new(self.project.move_lib()) }
      .map_err(|_| PatchErr::FailedLoadLib)?;

    let get_functions =
      unsafe { lib.get::<unsafe extern "C" fn(&Self) -> Vec<HotFn>>(b"hrl_get_functions") }
        .map_err(|_| PatchErr::NoGetFunctionsFn)?;

    let list_fn = unsafe { get_functions(self) };
    let mut vec_fn = self.backup_functions.clone();
    let hlength = self.project.root_dir.to_string_lossy().len() + 1;
    let mut map_fn = self.functions_dict.clone();

    for f in list_fn {
      let key = Self::to_key(
        f.fn_name,
        f.file_name.chars().skip(hlength).collect::<String>(),
      );
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
  pub fn apply_lib(&self, lib: libloading::Library, list_fn: Vec<fn()>) -> Option<String> {
    *self.functions.write().unwrap() = list_fn;
    let old_lib = self.lib.write().unwrap().take();
    if let Some(e) = old_lib.and_then(|lib| lib.close().err()) {
      return Some(e.to_string());
    }
    self.lib.write().unwrap().replace(lib);
    None
  }
}

pub fn get_fn_idx(fn_name: &'static str, file_name: &'static str) -> u16 {
  let i = HotLib::get_instance();
  let root_dir = i.project.root_dir.to_string_lossy().to_string();
  let file_name = if file_name.starts_with(&root_dir) {
    file_name
      .chars()
      .skip(root_dir.len() + 1)
      .collect::<String>()
  } else {
    file_name.to_string()
  };
  let key = HotLib::to_key(fn_name, file_name);
  println!("get_fn_idx: {}", &key);
  println!("functions_dict: {:?}", i.functions_dict);
  let idx = *i.functions_dict.get(&key).unwrap();
  idx
}

pub fn get_fn_list<T>() -> Arc<RwLock<Vec<T>>> {
  unsafe { std::mem::transmute(HotLib::get_instance().functions.clone()) }
}
