use crate::{HotLib, PatchErr, hotreload::macro_utils::macro_utils::make_fn};
use std::{
  collections::BTreeSet,
  sync::{Arc, RwLock},
};

macro_rules! make_hot_lib {
  ($name: ident {$($field: ident: $type: tt,)*}) => {
    pub struct EventCallbackList {
      pub event: Arc<RwLock<$name>>,
      $(pub $field: BTreeSet<usize>,)*
    }
    impl Drop for EventCallbackList {
      fn drop(&mut self) {
        let mut event = self.event.write().unwrap();
        $({
          event.$field.retain(|fn_ptr| {
            let ptr = fn_ptr as *const _ as usize;
            !self.$field.contains(&ptr)
          });
          // println!("Callbacks: {} => {:?}", stringify!($field), event.$field.iter().map(|ptr| ptr as *const _ as usize).collect::<Vec<usize>>());
        })*
      }
    }
    impl Default for EventCallbackList {
      fn default() -> Self {
        Self {
          event: HotLib::get_instance().event.clone(),
          $($field: BTreeSet::new(),)*
        }
      }
    }
    impl EventCallbackList {
      $(pub fn $field(&mut self, callback: impl $type) -> &mut Self {
        self.event.write().unwrap().$field(callback);
        let (ptr, _) = self.event.write().unwrap().last_callback.take().unwrap();
        self.$field.insert(ptr);
        self
      })*
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum CallbackType {
      $($type,)*
    }
    #[derive(Default)]
    pub struct $name {
      pub last_callback: Option<(usize, CallbackType)>,
    $(pub $field: Vec<Box<dyn $type>>,)* }
    impl $name {
      $(pub fn $field(&mut self, callback: impl $type) -> &mut Self {
        self.$field.push(Box::new(callback));
        self.last_callback = Some((self.$field.last().unwrap() as *const _ as usize, CallbackType::$type));
        // println!("Registered callback for {}: {:?}", stringify!($field), self.last_callback);
        self
      })*
      pub fn store_callback(&mut self, list: &mut EventCallbackList) -> &mut Self {
        self.last_callback.take().map(|(ptr, cb_type)| {
          match cb_type {
            $(CallbackType::$type => {
              list.$field.insert(ptr);
            })*
          }
        });
        self
      }
    }
  };
}

make_fn!(FnOnPrePatch: Fn());
make_fn!(FnOnCleanUp: Fn());
make_fn!(FnOnPatchSuccess: Fn());
make_fn!(FnOnPatchError: Fn(PatchErr));
make_hot_lib!(HotLibEvent {
  on_pre_patch: FnOnPrePatch,
  on_patch_success: FnOnPatchSuccess,
  on_patch_error: FnOnPatchError,
  on_clean_up: FnOnCleanUp,
});

impl HotLibEvent {
  pub fn on_boot(&mut self, callback: impl FnOnPatchSuccess) -> &mut Self {
    callback();
    self.on_patch_success(callback)
  }
}
impl EventCallbackList {
  pub fn on_boot(&mut self, callback: impl FnOnPatchSuccess) -> &mut Self {
    callback();
    self.on_patch_success(callback)
  }
}
