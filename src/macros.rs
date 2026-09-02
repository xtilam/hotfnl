#[macro_export]
macro_rules! run {
  ($($el:tt)*) => {
    hotfnl::if_hot! {
      hotfnl::run().ok();
    }
  };
}

#[macro_export]
macro_rules! use_event {
  ($($body:tt)*) => {
    hotfnl::if_hot! {{
      hotfnl::get_events()
        .write()
        .as_deref_mut()
        .map(|e| {
          e.$($body)*;
        })
        .ok();
    }}
  };
}

#[macro_export]
macro_rules! watch {
  ($($body: tt)*) => {
    hotfnl::if_hot! {{
      let w= hotfnl::HotProjectWatcherConfig{};
      w.$($body)*;
    }}
  };
}

#[macro_export]
macro_rules! use_local_event {
  (|$name:ident| $value:expr) => {
    hotfnl::if_hot! {
      let mut $name = hotfnl::new_event_list();
      $value;
    }
  };
}

#[macro_export]
macro_rules! use_hot {
  () => {
    hotfnl::if_hot! {
      pub mod hot {
        #[derive(Debug)]
        pub struct HotFn {
          pub func: fn(),
          pub fn_name: &'static str,
          pub file_name: &'static str,
        }
      }
      hotfnl::inventory::collect!(hot::HotFn);
      #[unsafe(no_mangle)]
      pub extern "C" fn hrl_get_functions(lib: std::sync::Arc<std::sync::RwLock<hotfnl::HotLib>>) -> Vec<hotfnl::HotFn> {
        let mut list_fn: Vec<hotfnl::HotFn> = vec![];
        hotfnl::inventory::iter::<hot::HotFn>().for_each(|f| {
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
    }
  };
}
