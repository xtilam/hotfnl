use crate::{HotLib, hotreload::macro_utils::make_fn};
make_fn!(OnHotLibEvent: Fn(HotLibEvents));

#[derive(Default)]
pub struct HotLibEvent {
  events: Vec<Box<dyn OnHotLibEvent>>,
}

impl HotLibEvent {
  pub fn push(&mut self, callback: impl OnHotLibEvent) -> usize {
    self.events.push(Box::new(callback));
    let usize_ptr = self.events.last().unwrap();
    usize_ptr as *const _ as *const () as usize
  }
  pub fn remove(&mut self, ptr: usize) {
    self.events.retain(|cb| {
      let ptr_cb = cb as *const _ as *const () as usize;
      // println!("Current callback ptr 2: {:?}", (ptr_cb, ptr));
      ptr_cb != ptr
    });
  }
  pub fn trigger(&self, event_type: HotLibEvents) {
    for callback in &self.events {
      callback(event_type.clone());
    }
  }
  pub fn global(callback: impl OnHotLibEvent) {
    HotLib::get_instance().event.write().unwrap().push(callback);
  }
  pub fn local(callback: impl OnHotLibEvent) -> HotLibEventLocal {
    HotLibEventLocal::new(callback)
  }
}

pub struct HotLibEventLocal(usize);

impl HotLibEventLocal {
  pub fn new(callback: impl OnHotLibEvent) -> Self {
    Self(
      HotLib::get_instance()
        .event
        .clone()
        .write()
        .unwrap()
        .push(callback),
    )
  }
}
impl Drop for HotLibEventLocal {
  fn drop(&mut self) {
    HotLib::get_instance()
      .event
      .clone()
      .write()
      .unwrap()
      .remove(self.0);
  }
}

#[derive(Debug, Clone)]
pub enum HotLibEvents {
  SourceChanged,
  StartRebuild,
  PatchSuccess,
  PatchError,
  CleanUp,
  BuildSuccess,
  BuildFailed,
}
// use crate::{HotLib, PatchErr, hotreload::macro_utils::make_fn};
// use std::{
//   collections::BTreeSet,
//   sync::{Arc, RwLock},
// };
//
// macro_rules! make_hot_lib {
//   ($name: ident {$($field: ident: $type: tt,)*}) => {
//     /// A scoped listener over a set of lifecycle callbacks.
//     ///
//     /// Holds the set of registered callback pointers and unregisters them when dropped.
//     pub struct EventCallbackList {
//       /// The shared event registry this listener is bound to.
//       pub event: Arc<RwLock<$name>>,
//       $(pub $field: BTreeSet<usize>,)*
//     }
//     impl Drop for EventCallbackList {
//       fn drop(&mut self) {
//         let mut event = self.event.write().unwrap();
//         $({
//           event.$field.retain(|fn_ptr| {
//             let ptr = fn_ptr as *const _ as usize;
//             !self.$field.contains(&ptr)
//           });
//         })*
//       }
//     }
//     impl Default for EventCallbackList {
//       fn default() -> Self {
//         Self {
//           event: HotLib::get_instance().event.clone(),
//           $($field: BTreeSet::new(),)*
//         }
//       }
//     }
//     impl EventCallbackList {
//       $(pub fn $field(&mut self, callback: impl $type) -> &mut Self {
//         self.event.write().unwrap().$field(callback);
//         let (ptr, _) = self.event.write().unwrap().last_callback.take().unwrap();
//         self.$field.insert(ptr);
//         self
//       })*
//     }
//     #[derive(Debug, Clone, Copy, PartialEq, Eq)]
//     /// Identifies the kind of a lifecycle callback for bookkeeping.
//     #[allow(clippy::enum_variant_names)]
//     pub enum CallbackType {
//       $($type,)*
//     }
//
//     /// The shared registry of lifecycle callbacks.
//     #[derive(Default)]
//     pub struct $name {
//       pub last_callback: Option<(usize, CallbackType)>,
//       $(pub $field: Vec<Box<dyn $type>>,)*
//     }
//     impl $name {
//       $(pub fn $field(&mut self, callback: impl $type) -> &mut Self {
//         self.$field.push(Box::new(callback));
//         self.last_callback = Some((self.$field.last().unwrap() as *const _ as usize, CallbackType::$type));
//         self
//       })*
//       pub fn store_callback(&mut self, list: &mut EventCallbackList) -> &mut Self {
//         self.last_callback.take().map(|(ptr, cb_type)| {
//           match cb_type {
//             $(CallbackType::$type => {
//               list.$field.insert(ptr);
//             })*
//           }
//         });
//         self
//       }
//     }
//   };
// }
//
// make_fn!(FnOnPreRebuild: Fn());
// make_fn!(FnOnRebuildSuccess: Fn());
// make_fn!(FnOnRebuildError: Fn());
// make_fn!(FnOnPrePatch: Fn());
// make_fn!(FnOnCleanUp: Fn());
// make_fn!(FnOnPatchSuccess: Fn());
// make_fn!(FnOnPatchError: Fn(PatchErr));
//
// make_hot_lib!(HotLibEvent {
//   on_source_changed: FnOnSourceChanged,
//   on_pre_patch: FnOnPrePatch,
//   on_patch_success: FnOnPatchSuccess,
//   on_patch_error: FnOnPatchError,
//   on_clean_up: FnOnCleanUp,
//   on_pre_rebuild: FnOnPreRebuild,
//   on_rebuild_success: FnOnRebuildSuccess,
//   on_rebuild_error: FnOnRebuildError,
// });

// impl HotLibEvent {
//   /// Runs `callback` immediately, then registers it as an `on_patch_success` callback.
//   pub fn on_boot(&mut self, callback: impl FnOnPatchSuccess) -> &mut Self {
//     callback();
//     self.on_patch_success(callback)
//   }
// }
//
// impl EventCallbackList {
//   /// Runs `callback` immediately, then registers it as an `on_patch_success` callback.
//   pub fn on_boot(&mut self, callback: impl FnOnPatchSuccess) -> &mut Self {
//     callback();
//     self.on_patch_success(callback)
//   }
// }
