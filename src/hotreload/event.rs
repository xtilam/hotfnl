//! Lifecycle event and callback system for the hot-reload engine.
//!
//! Provides typed callback lists (`on_pre_patch`, `on_patch_success`, `on_patch_error`,
//! `on_clean_up`) and scoped callback listeners that unregister on drop.

use crate::{HotLib, PatchErr, hotreload::macro_utils::make_fn};
use std::{
  collections::BTreeSet,
  sync::{Arc, RwLock},
};

macro_rules! make_hot_lib {
  ($name: ident {$($field: ident: $type: tt,)*}) => {
    /// A scoped listener over a set of lifecycle callbacks.
    ///
    /// Holds the set of registered callback pointers and unregisters them when dropped.
    pub struct EventCallbackList {
      /// The shared event registry this listener is bound to.
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
    /// Identifies the kind of a lifecycle callback for bookkeeping.
    pub enum CallbackType {
      $($type,)*
    }

    /// The shared registry of lifecycle callbacks.
    #[derive(Default)]
    pub struct $name {
      pub last_callback: Option<(usize, CallbackType)>,
      $(pub $field: Vec<Box<dyn $type>>,)*
    }
    impl $name {
      $(pub fn $field(&mut self, callback: impl $type) -> &mut Self {
        self.$field.push(Box::new(callback));
        self.last_callback = Some((self.$field.last().unwrap() as *const _ as usize, CallbackType::$type));
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
  /// Runs `callback` immediately, then registers it as an `on_patch_success` callback.
  pub fn on_boot(&mut self, callback: impl FnOnPatchSuccess) -> &mut Self {
    callback();
    self.on_patch_success(callback)
  }
}
impl EventCallbackList {
  /// Runs `callback` immediately, then registers it as an `on_patch_success` callback.
  pub fn on_boot(&mut self, callback: impl FnOnPatchSuccess) -> &mut Self {
    callback();
    self.on_patch_success(callback)
  }
}
