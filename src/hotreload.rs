mod event;
mod file_watcher;
mod fs_utils;
mod hotfn;
mod hotlib;
mod hotproject;
mod wait_rebuild;
mod watch_task;

pub use hotfn::HotFn;
pub use hotlib::{HotLib, PatchErr, get_fn_idx, get_fn_list};
pub use wait_rebuild::*;

pub fn reload_lib() {
  HotLib::get_instance()
    .trigger(hotlib::HotLibAction::ReloadLib)
    .ok();
}
pub fn boot(fns: Vec<HotFn>, file_name: &str, project_dir: &str) {
  HotLib::get_instance_mut().on_boot(fns, file_name, project_dir);
}

#[macro_export]
macro_rules! use_hot {
  () => {
    pub mod hot {
      #[derive(Debug)]
      pub struct HotFn {
        pub func: fn(),
        pub fn_name: &'static str,
        pub file_name: &'static str,
      }
    }
    inventory::collect!(hot::HotFn);
    #[unsafe(no_mangle)]
    pub extern "C" fn hrl_get_functions(lib: &hotfnl::HotLib) -> Vec<hotfnl::HotFn> {
      let mut list_fn: Vec<hotfnl::HotFn> = vec![];
      inventory::iter::<hot::HotFn>().for_each(|f| {
        list_fn.push(hotfnl::HotFn {
          file_name: f.file_name,
          fn_name: f.fn_name,
          func: f.func,
          ptr: None,
        });
      });
      hotfnl::HotLib::rewrite_instance(lib);
      list_fn
    }
  };
}

#[macro_export]
macro_rules! hot_api {
  ($($el:tt)*) => {
    hotfnl::$($el)*
  };
}

#[macro_export]
macro_rules! if_hot {
  ($($el:tt)*) => {
     $($el)*
  };
}

#[macro_export]
macro_rules! else_hot {
  ($($el:tt)*) => {};
}

#[macro_export]
macro_rules! collect {
  () => {{
    let list_fn: Vec<hotfnl::HotFn> = inventory::iter::<hot::HotFn>
      .into_iter()
      .map(|f| hotfnl::HotFn {
        file_name: f.file_name,
        fn_name: f.fn_name,
        func: f.func,
        ptr: None,
      })
      .collect();
    hotfnl::boot(list_fn, file!(), env!("CARGO_MANIFEST_DIR"));
  }};
}

// #[macro_export]
// macro_rules! hot {
//   ($pub:vis fn $name: ident ($($name_arg:ident: $args:ty),*) -> $ret:ty $body:block $($prefix:tt)*) => {
//     #[unsafe(no_mangle)]
//     $pub fn $name($($name_arg: $args),*) -> $ret {
//       use std::sync::{Arc, RwLock, LazyLock};
//       type FnType = fn($($args),*) -> $ret;
//       static mut PTR: LazyLock<Arc<RwLock<FnType>>> = LazyLock::new(|| {
//         return hotfnl::get_function_ptr::<FnType>(FN_NAME, FILE_NAME)
//       });
//       static FN_NAME: &'static str = concat!(stringify!(fn $($prefix)*$name($($args),*) ->$ret));
//       static FILE_NAME: &'static str = concat!(file!());
//
//       fn cb($($name_arg: $args),*) -> $ret $body
//       inventory::submit! {
//         hot::HotFn {
//           name: FN_NAME,
//           func: unsafe { std::mem::transmute(cb as FnType) },
//           fn_name: FN_NAME,
//           file_name: FILE_NAME,
//         }
//       }
//       unsafe { PTR.read().unwrap()($($name_arg),*) }
//     }
//   };
// }
