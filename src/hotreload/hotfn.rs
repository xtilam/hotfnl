use std::sync::{Arc, RwLock};

#[derive(Debug, Clone)]
pub struct HotFn {
  pub file_name: &'static str,
  pub fn_name: &'static str,
  pub func: fn(),
  pub ptr: Option<Arc<RwLock<fn()>>>,
}
