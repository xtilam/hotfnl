#[macro_export]
macro_rules! boot {
  ($($el:tt)*) => {{
    Ok::<(), anyhow::Error>(())
  }};
}

#[macro_export]
macro_rules! use_hot {
  () => {};
}

#[macro_export]
macro_rules! reload {
  () => {};
}

#[macro_export]
macro_rules! hot_api {
  ($($el:tt)*) => {};
}

#[macro_export]
macro_rules! else_hot{
  ($($el:tt)*) => {
     $($el)*
  };
}

#[macro_export]
macro_rules! if_hot {
  ($($el:tt)*) => {};
}


pub fn reload_lib() -> anyhow::Result<bool> {
  Ok(true)
}
