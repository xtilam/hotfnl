use std::sync::{Arc, RwLock, mpsc};

use crate::{HotLib, PatchErr, hotreload::hotlib::HotLibAction};

pub struct WatchTask {
  is_reload: bool,
  is_rebuild: bool,
  is_running: bool,
  load_result: Option<(libloading::Library, Vec<fn()>)>,
}

impl WatchTask {
  pub fn new() -> Self {
    Self {
      is_reload: false,
      is_rebuild: false,
      is_running: false,
      load_result: None,
    }
  }
  pub fn run(self) -> mpsc::Sender<HotLibAction> {
    let (tx, rx) = mpsc::channel();
    let this = Arc::new(RwLock::new(self));
    std::thread::spawn({
      let this = this.clone();
      move || {
        for res in rx {
          use HotLibAction::*;
          match res {
            Rebuild => {
              this.write().unwrap().is_rebuild = true;
            }
            ReloadLib => {
              this.write().unwrap().is_reload = true;
            }
          }
          Self::delay_handler(this.clone());
        }
      }
    });

    tx
  }
  fn delay_handler(this: Arc<RwLock<Self>>) {
    if this.read().unwrap().is_running {
      return;
    }
    this.write().unwrap().is_running = true;
    std::thread::spawn({
      let this = this.clone();
      move || {
        std::thread::sleep(std::time::Duration::from_millis(200));
        Self::handler(this);
      }
    });
  }

  fn handler(this: Arc<RwLock<Self>>) {
    if this.read().unwrap().is_rebuild {
      this.write().unwrap().is_rebuild = false;
      let is_watch_mode = HotLib::get_instance().project.is_watch_mode;
      if is_watch_mode {
        // i.
        // let rs = i.project.rebuild_lib();
        // (i.on_rebuild_result)(rs);
      } else {
      //   i.project.rebuild().map(|_| i.project.restart()).ok();
      }
      return Self::handler(this);
    }

    if this.read().unwrap().is_reload {
      this.write().unwrap().is_reload = false;
      let old_lib = this.write().unwrap().load_result.take();
      if let Some((lib, _)) = old_lib {
        if let Some(_) = lib.close().err() {
          Self::restart();
        }
      }
      let result = HotLib::get_instance().get_lib();
      if let Some(err) = result.as_ref().err() {
        use PatchErr::*;
        match err {
          FailedCleanLib(_) | ToManyChange => {
            Self::restart();
          }
          FailedLoadLib | NoGetFunctionsFn | OtherError => {
            let evt = HotLib::get_instance().event.clone();
            evt.read().unwrap().on_pre_patch.iter().for_each(|f| f());
            evt
              .read()
              .unwrap()
              .on_patch_error
              .iter()
              .for_each(|f| f(err.clone()));
          }
        }
      } else {
        this.write().unwrap().load_result = result.ok();
      }

      std::thread::sleep(std::time::Duration::from_millis(100));
      return Self::handler(this);
    }

    let load_result = this.write().unwrap().load_result.take();

    if let Some((lib, list_fn)) = load_result {
      std::thread::spawn(move || {
        let event = HotLib::get_instance().event.clone();
        event.read().unwrap().on_pre_patch.iter().for_each(|f| f());
        if let Some(err) = HotLib::get_instance().apply_lib(lib, list_fn) {
          let err = PatchErr::FailedCleanLib(err);
          event
            .read()
            .unwrap()
            .on_patch_error
            .iter()
            .for_each(|f| f(err.clone()));
        } else {
          event
            .read()
            .unwrap()
            .on_patch_success
            .iter()
            .for_each(|f| f());
        }
      });
      return Self::handler(this);
    };

    this.write().unwrap().is_running = false;
  }
  fn restart() {
    HotLib::get_instance().project.restart();
  }
}
