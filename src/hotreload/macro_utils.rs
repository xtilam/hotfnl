//! Internal helper macros.
//!
//! `bselect!` is a shorthand for `crossbeam_channel::select!`, and `make_fn!` generates
//! blanket trait implementations for callback types.

macro_rules! bselect {
  ($([$action:tt($tx:expr), $(|$name: tt|)? {$($value:tt)*}]$(,)?)*) => {
    crossbeam_channel::select! { $( $action($tx) $(->$name)? => {$($value)*},)* }
  };
}

macro_rules! make_fn {
  ($name: ident : $($fn:tt)*) => {
    pub trait $name: $($fn)* + Send + Sync + 'static {}
    impl<T> $name for T where T: $($fn)* + Send + Sync + 'static {}
  };
}
pub(crate) use bselect;
pub(crate) use make_fn;
