pub use hotfnl_proc_macro::*;
pub use inventory;

#[cfg(debug_assertions)]
mod hotreload;
#[cfg(debug_assertions)]
pub use hotreload::*;
mod macros;

#[cfg(debug_assertions)]
#[macro_export]
macro_rules! if_hot {
  ($($el:tt)*) => {
    $($el)*
  };
}

#[cfg(debug_assertions)]
#[macro_export]
macro_rules! if_prod {
  ($($el:tt)*) => {};
}

#[cfg(debug_assertions)]
#[macro_export]
macro_rules! match_hot {
  ({$($hot:tt)*}$(,{$($prod:tt)*})?) => {
    $($hot)*
  };
}

#[cfg(not(debug_assertions))]
#[macro_export]
macro_rules! match_hot {
  ({$($hot:tt)*}$(,{$($prod:tt)*})?) => {
    $($prod)*
  };
}

#[cfg(not(debug_assertions))]
#[macro_export]
macro_rules! if_prod {
  ($($el:tt)*) => {
    $($el)*
  };
}

#[cfg(not(debug_assertions))]
#[macro_export]
macro_rules! if_hot {
  ($($el:tt)*) => {};
}
