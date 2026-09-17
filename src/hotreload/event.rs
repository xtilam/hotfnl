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


#[allow(clippy::derivable_impls)]
impl Default for HotLibEvents {
  fn default() -> Self {
    HotLibEvents::PatchSuccess
  }
}
