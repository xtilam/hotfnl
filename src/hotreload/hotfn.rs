//! Definition of a hot-swappable function handle.

use std::sync::{Arc, RwLock};

/// Describes a single hot-swappable function.
///
/// Instances are produced by the `#[hot_fn]` / `#[hot_method]` proc macros and collected
/// via the [`crate::inventory`] registry at compile time.
#[derive(Debug, Clone)]
pub struct HotFn {
  /// Source file the function was defined in.
  pub file_name: &'static str,
  /// Function name (including its type-signature key).
  pub fn_name: &'static str,
  /// The raw function pointer, cast to the generic `fn()` type.
  pub func: fn(),
  /// Optional live pointer used when the function is swapped.
  pub ptr: Option<Arc<RwLock<fn()>>>,
}
