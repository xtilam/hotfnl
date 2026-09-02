//! Core hot-reload engine: library loading, function pointer patching, and the watch
//! loop that drives reloads.

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
    file_watcher::FileWatcher, hotfn::HotFn, hotproject::HotProject, macro_utils::bselect,
  },
};

/// The central hot-reload engine.
///
/// This is a process-wide singleton holding the currently loaded dynamic library, the
/// registry of hot-swappable function pointers, and the background watch loop.
pub struct HotLib {
  /// Metadata about the generated hot project and its file layout.
  pub project: HotProject,
  /// The currently loaded dynamic library, if any.
  pub lib: Arc<RwLock<Option<libloading::Library>>>,
  /// The active function-pointer table, patched on each reload.
  pub functions: Arc<RwLock<Vec<fn()>>>,
  /// Maps a function key (`file:name`) to its index in [`Self::functions`].
  pub functions_dict: BTreeMap<String, u16>,
  /// The original function pointers captured at boot, used as a baseline for patching.
  pub backup_functions: Vec<fn()>,
  /// Channel used to send actions to the background watch loop.
  pub tx: crossbeam_channel::Sender<HotLibAction>,
  /// The shared event registry for lifecycle callbacks.
  pub event: Arc<RwLock<HotLibEvent>>,
  /// Whether the app is already running as a hot project (as opposed to a fresh build).
  pub is_hot_project: bool,
  /// Source paths watched for changes, mapping each path to whether it is watched
  /// recursively.
  pub watch_src: BTreeMap<PathBuf, bool>,
  is_configured: bool,
}

/// Actions that can be sent to the hot-reload watch loop.
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum HotLibAction {
  /// Reload the dynamic library and patch function pointers.
  ReloadLib,
}

/// Errors that can occur while loading and applying a patched library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchErr {
  /// The dynamic library could not be opened.
  FailedLoadLib,
  /// An existing library could not be closed/unloaded.
  FailedCleanLib(String),
  /// The library does not export the required `hrl_get_functions` symbol.
  NoGetFunctionsFn,
  /// The new library changed the set of functions (added or removed entries).
  ToManyChange,
  /// An unspecified error occurred.
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
  /// Creates a fresh, unconfigured hot-reload engine.
  pub fn new() -> Self {
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
  /// Replaces the singleton instance with a provided one.
  ///
  /// This is used by the `hrl_get_functions` export to hand the library's instance back
  /// to this process after a reload.
  pub fn rewrite_instance(old_instance: Arc<RwLock<HotLib>>) {
    DEFAULT_INSTANCE.get_or_init(|| Arc::new(Some(old_instance.clone())));
  }

  /// Returns a read guard to the singleton instance.
  pub fn get_instance() -> RwLockReadGuard<'static, Self> {
    INSTANCE.read().unwrap()
  }

  /// Returns a write guard to the singleton instance.
  pub fn get_instance_mut() -> RwLockWriteGuard<'static, Self> {
    INSTANCE.write().unwrap()
  }

  /// Configures the engine with the boot-time function list and project metadata.
  ///
  /// # Panics
  /// Panics if the engine is already configured, or if the function list contains
  /// duplicate keys.
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
  /// Builds the unique key (`file:name`) used to identify a hot function.
  pub fn to_key(fn_name: impl Into<String>, file_name: impl Into<String>) -> String {
    format!("{}:{}", file_name.into(), fn_name.into())
  }

  /// Sends an action to the background watch loop.
  pub fn trigger(&self, action: HotLibAction) -> Result<()> {
    self.tx.send(action)?;
    Ok(())
  }

  /// Loads a dynamic library at `lib_path` and resolves its updated function table.
  ///
  /// The library must export `hrl_get_functions`, returning the `HotFn` list. The
  /// resolved functions are merged into a copy of the backup pointers; the library is
  /// closed if the new function set does not exactly match the known keys.
  pub fn get_lib(&self, lib_path: PathBuf) -> Result<(libloading::Library, Vec<fn()>), PatchErr> {
    // SAFETY: `Library::new` safely wraps a dynamic library load; symbol resolution and
    // the `hrl_get_functions` call below are inherently unsafe by nature of FFI, but the
    // signature is generated by the `use_hot!` macro and is fixed across the build.
    let lib = unsafe { libloading::Library::new(lib_path) }.map_err(|_| PatchErr::FailedLoadLib)?;
    let get_functions = unsafe {
      lib.get::<unsafe extern "C" fn(Arc<RwLock<Self>>) -> Vec<HotFn>>(b"hrl_get_functions")
    }
    .map_err(|_| PatchErr::NoGetFunctionsFn)?;

    // SAFETY: Calls the FFI symbol resolved above. The function returns owned `HotFn`
    // values whose `func` fields are raw function pointers cast to `fn()`.
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
  /// Swaps in a newly loaded library and its function table.
  ///
  /// Runs `on_clean_up` callbacks, unloads the previous library, replaces the
  /// function-pointer table, and stores the new library. Returns `Some(PatchErr)` if the
  /// previous library could not be closed.
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
  /// Spawns the background watch loop.
  ///
  /// The loop watches the `lib_version.txt` file and, on modification, reloads the
  /// library and patches function pointers. It terminates the process if the patch
  /// cannot be applied safely (a state it cannot recover from in place).
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
              action
                .map(|action| {
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
                })
                .ok();
            }]
          );
        }
      }
    });
  }
}

/// Returns the table index for a hot function identified by `fn_name` and `file_name`.
///
/// # Panics
/// Panics if the function is not registered.
pub fn get_fn_idx(fn_name: &'static str, file_name: &'static str) -> u16 {
  let i = HotLib::get_instance();
  let key = HotLib::to_key(fn_name, file_name);
  let idx = *i.functions_dict.get(&key).unwrap();
  idx
}

/// Returns the active function-pointer table, typed as `Vec<T>`.
///
/// The caller must ensure that `T` matches the actual stored function-pointer type.
///
/// # Safety
/// The table is stored as `Vec<fn()>`; transmuting to `Vec<T>` for a different `T` is
/// [unsound][transmute-def] and may cause undefined behavior.
///
/// [transmute-def]: https://doc.rust-lang.org/std/mem/fn.transmute.html
pub fn get_fn_list<T>() -> Arc<RwLock<Vec<T>>> {
  // SAFETY: `T` is expected to be a function-pointer type whose layout matches the
  // stored `fn()` pointers. Callers must uphold this invariant; see the safety docs.
  unsafe { std::mem::transmute(HotLib::get_instance().functions.clone()) }
}
