use crate::PatchErr;

#[macro_export]
macro_rules! make_fn {
  ($name: ident : $($fn:tt)*) => {
     pub trait $name: $($fn)* + Send + Sync + 'static {}
     impl<T> $name for T where T: $($fn)* + Send + Sync + 'static {}
  };
}

macro_rules! make_hot_lib {
  ($name: ident {$($field: ident: $type: tt,)*}) => {
    #[derive(Default)]
    pub struct $name { $(pub $field: Vec<Box<dyn $type>>,)* }
    impl $name { 
      $(pub fn $field(mut self, callback: impl $type) -> Self { self.$field.push(Box::new(callback)); self })* 
    }
  };
}

make_fn!(FnOnPreBuild: Fn());
make_fn!(FnOnBuildSuccess: Fn());
make_fn!(FnOnBuildError: Fn(i64));
make_fn!(FnOnPrePatch: Fn());
make_fn!(FnOnPatchSuccess: Fn());
make_fn!(FnOnPatchError: Fn(PatchErr));

make_hot_lib!(HotLibEvent {
  on_pre_build: FnOnPreBuild,
  on_build_success: FnOnBuildSuccess,
  on_build_error: FnOnBuildError,
  on_pre_patch: FnOnPrePatch,
  on_patch_success: FnOnPatchSuccess,
  on_patch_error: FnOnPatchError,
});

