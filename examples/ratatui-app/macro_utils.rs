pub mod macro_utils {
  macro_rules! bselect{
    ($([$action:tt($tx:expr), $(|$name: tt|)? {$($value:tt)*}]$(,)?)*) => {
      crossbeam_channel::select! { $( $action($tx) $(->$name)? => {$($value)*},)* }
    };
  }

  pub(crate) use bselect;
}
