mod macros;
pub use hotfnl_proc_macro::*;
pub use inventory;

#[cfg(not(feature = "prod"))]
mod hotreload;
#[cfg(not(feature = "prod"))]
pub use hotreload::*;

#[cfg(not(feature = "prod"))]
#[macro_export]
macro_rules! if_hot {
  ($($el:tt)*) => { $($el)* };
}

#[cfg(not(feature = "prod"))]
#[macro_export]
macro_rules! if_prod {
  ($($el:tt)*) => {};
}

#[cfg(feature = "prod")]
#[macro_export]
macro_rules! if_hot {
  ($($el:tt)*) => {};
}

#[cfg(feature = "prod")]
#[macro_export]
macro_rules! if_prod{
  ($($el:tt)*) => { $($el)* };
}
