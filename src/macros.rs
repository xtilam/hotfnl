//! User-facing entry-point macros for the hot-reload runtime.
/// Starts the hot-reload runtime (`hotfnl::run()`).
///
/// This is a no-op in `prod` builds.
#[macro_export]
macro_rules! run {
  ($($el:tt)*) => {
    hotfnl::if_hot! {
      hotfnl::run().ok();
    }
  };
}

/// Registers lifecycle event callbacks on the shared event registry.
///
/// No-op in `prod` builds.
#[macro_export]
macro_rules! use_event {
  ($($body: tt)*) => {
    hotfnl::if_hot! {{
      use hotfnl::EventType::*;
      hotfnl::HotLibEvent::global($($body)*);
    }}
  };
}

/// Configures additional source paths to watch for changes.
///
/// No-op in `prod` builds.
#[macro_export]
macro_rules! watch {
  ($($body:tt)*) => {
    hotfnl::if_hot! {{
      let w = hotfnl::HotProjectWatcherConfig {};
      w.$($body)*;
    }}
  };
}

/// Creates a scoped event callback list that unregisters its callbacks when it goes out
/// of scope.
///
/// No-op in `prod` builds.
#[macro_export]
macro_rules! use_local_event {
  ($name:ident, $($body:tt)*) => {
    hotfnl::if_hot! {
      use hotfnl::EventType::*;
      let $name = hotfnl::HotLibEvent::local($($body)*);
    }
  };
}

/// Configures additional cargo arguments to pass to the hot-reload runtime.
///
/// No-op in `prod` builds.
#[macro_export]
macro_rules! use_cargo_args {
  ($($body:tt)*) => {
    hotfnl::if_hot! {{
      hotfnl::add_cargo_args([$($body)*]);
    }}
  };
}
