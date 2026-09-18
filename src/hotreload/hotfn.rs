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
}

/// Describes the compile-time layout fingerprint of a `#[hot_layout]` struct.
///
/// Instances are produced by the `#[hot_layout]` proc macro and collected via the
/// [`crate::inventory`] registry at compile time.
#[derive(Debug, Clone)]
pub struct HotLayout {
  /// Source file the struct was defined in.
  pub file_name: &'static str,
  /// Struct name.
  pub struct_name: &'static str,
  /// Content hash of the struct definition, computed at compile time.
  pub hash: u64,
}
