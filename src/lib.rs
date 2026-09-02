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
