//! A lightweight hot-function-swapping library for Rust applications on Linux.
//!
//! `hotfnl` lets you replace individual function implementations at runtime without
//! restarting your application. It is *not* full module hot-reloading (HMR) — it only
//! swaps function pointers when a newly compiled dynamic library (`.so`) is available.
//!
//! This is inherently `unsafe`: the loaded library must contain functions with
//! byte-compatible signatures, and no ABI guarantee is enforced. It is intended for
//! development-time iteration loops only.
//!
//! # Feature flags
//!
//! - `default`: enables all hot-reloading machinery and proc-macro expansion.
//! - `prod`: strips all hot-reloading code. Proc macros become pass-through and the
//!   hot-reload subsystem is not compiled, giving zero runtime overhead.
//!
//! # Quick start
//!
//! ```ignore
//! #[hot_main]
//! fn main() {
//!   hotfnl::run!();
//!   loop {
//!     std::thread::sleep(std::time::Duration::from_secs(1));
//!     println!("{}", greet());
//!   }
//! }
//!
//! #[hot_fn]
//! fn greet() -> &'static str {
//!   "hello"
//! }
//! ```
//!
//! When the app is first launched, `hotfnl` scaffolds a hot project, runs it through a
//! wrapper binary, and reloads changed functions on every rebuild.
//!
//! # Conditional compilation
//!
//! Use [`if_hot!`] and [`if_prod!`] to emit code only in the corresponding build mode:
//!
//! ```ignore
//! hotfnl::if_hot! {
//!   println!("this runs only in hot (dev) builds");
//! }
//!
//! hotfnl::if_prod! {
//!   println!("this runs only in prod builds");
//! }
//! ```
//!
//! Use `#[hot_check]` with `#[dev]` and `#[prod]` attributes for item-level conditional
//! compilation without writing `cfg` attributes manually.
mod macros;
pub use hotfnl_proc_macro::*;
pub use inventory;

#[cfg(not(feature = "prod"))]
mod hotreload;
#[cfg(not(feature = "prod"))]
pub use hotreload::*;

/// Emits the body only in hot (non-prod) builds; expands to nothing under `prod`.
#[cfg(not(feature = "prod"))]
#[macro_export]
macro_rules! if_hot {
  ($($el:tt)*) => { $($el)* };
}

/// Emits the body only in prod builds; expands to nothing in hot (non-prod) builds.
#[cfg(not(feature = "prod"))]
#[macro_export]
macro_rules! if_prod {
  ($($el:tt)*) => {};
}
/// Emits the first body in hot (non-prod) builds, and the second body in prod builds.
#[cfg(not(feature = "prod"))]
#[macro_export]
macro_rules! match_hot {
  ({$($hot:tt)*}$(,{$($prod:tt)*})?) => {
    $($hot)*
  };
}

/// Emits the body only in hot (non-prod) builds; expands to nothing under `prod`.
#[cfg(feature = "prod")]
#[macro_export]
macro_rules! if_hot {
  ($($el:tt)*) => {};
}

/// Emits the body only in prod builds; expands to nothing in hot (non-prod) builds.
#[cfg(feature = "prod")]
#[macro_export]
macro_rules! if_prod{
  ($($el:tt)*) => { $($el)* };
}

/// Emits the first body in hot (non-prod) builds, and the second body in prod builds.
#[cfg(feature = "prod")]
#[macro_export]
macro_rules! match_hot {
  ({$($hot:tt)*}$(,{$($prod:tt)*})?) => {
    $($($prod)*)*
  };
}
