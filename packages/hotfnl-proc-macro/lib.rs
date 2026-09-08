//! Procedural macros for [`hotfnl`](https://docs.rs/hotfnl).
//!
//! Provides `#[hot_main]`, `#[hot_fn]`, `#[hot_impl]`, `#[hot_method]`, and
//! `#[hot_check]`. When the `prod` feature is enabled, all hot-reload macros become
//! pass-through no-ops that emit the original item unchanged. `#[hot_check]` always
//! rewrites regardless of the feature flag.

use proc_macro::TokenStream;

#[cfg(not(feature = "prod"))]
macro_rules! token_err {
  ($($el:tt)*) => {
    syn::Error::new_spanned($($el)*)
      .to_compile_error()
      .into()
  };
}

#[cfg(not(feature = "prod"))]
macro_rules! token_unwrap {
  ($value: expr, |$name: ident| $take: expr, $err: literal) => {{
    let $name = $value;
    let value = $take;
    match value {
      Some(v) => v,
      None => return token_err!($name, $err),
    }
  }};
}

/// Wraps `main` to bootstrap the hot-reload system.
///
/// Collects all `#[hot_fn]`-annotated functions, boots the engine, and runs the original
/// body. A no-op under the `prod` feature.
#[proc_macro_attribute]
pub fn hot_main(_attr: TokenStream, item: TokenStream) -> TokenStream {
  #[cfg(feature = "prod")]
  return item;
  #[cfg(not(feature = "prod"))]
  {
    use proc_macro::TokenStream;
    use quote::quote;
    use syn::{ItemFn, parse_macro_input};
    let (is_hot_project, env) = std::env::var("HOT_PROJECT_DIR")
      .map(|env| (true, quote! {#env}))
      .unwrap_or((false, quote! {env!("CARGO_MANIFEST_DIR")}));
    let input = parse_macro_input!(item as ItemFn);
    let vis = &input.vis;
    let sig = &input.sig;
    let body = &input.block;
    let attrs = &input.attrs;
    let expanded = {
      quote! {
        pub mod hot {
          #[derive(Debug)]
          pub struct HotFn {
            pub func: fn(),
            pub fn_name: &'static str,
            pub file_name: &'static str,
          }
        }
        hotfnl::inventory::collect!(crate::hot::HotFn);
        #[unsafe(no_mangle)]
        pub extern "C" fn hrl_get_functions(lib: std::sync::Arc<std::sync::RwLock<hotfnl::HotLib>>) -> Vec<hotfnl::HotFn> {
          let mut list_fn: Vec<hotfnl::HotFn> = vec![];
          hotfnl::inventory::iter::<hot::HotFn>().for_each(|f| {
            list_fn.push(hotfnl::HotFn {
              file_name: f.file_name,
              fn_name: f.fn_name,
              func: f.func,
            });
          });
          hotfnl::HotLib::rewrite_instance(lib);
          list_fn
        }
        #(#attrs)*
        #[allow(dead_code)]
        #vis #sig {
          {
            let list_fn: Vec<hotfnl::HotFn> = hotfnl::inventory::iter::<hot::HotFn>
              .into_iter()
              .map(|f| hotfnl::HotFn {
                file_name: f.file_name,
                fn_name: f.fn_name,
                func: f.func,
              })
              .collect();
            hotfnl::boot(#is_hot_project, list_fn, file!(), #env);
          }
          #body
        }
      }
    };
    TokenStream::from(expanded)
  }
}
/// Wraps a free function so it becomes hot-patchable.
///
/// Generates a wrapper that dispatches the call through the hot function-pointer table,
/// letting its implementation be swapped at runtime. A no-op under the `prod` feature.
#[proc_macro_attribute]
pub fn hot_fn(attr: TokenStream, item: TokenStream) -> TokenStream {
  #[cfg(feature = "prod")]
  {
    let _ = attr;
    item
  }

  #[cfg(not(feature = "prod"))]
  {
    use proc_macro::TokenStream;
    use quote::quote;
    use syn::{FnArg, ItemFn, Pat, ReturnType, parse_macro_input};
    let input = parse_macro_input!(item as ItemFn);
    let vis = &input.vis;
    let sig = &input.sig;
    let body = &input.block;
    let expanded = {
      let mut arg_names = Vec::new();
      let mut arg_types = Vec::new();

      for arg in &sig.inputs {
        match arg {
          FnArg::Typed(arg) => {
            let pat = match &*arg.pat {
              Pat::Ident(pat) => &pat.ident,
              _ => {
                return syn::Error::new_spanned(
                  &arg.pat,
                  "#[hot] currently only supports named arguments",
                )
                .to_compile_error()
                .into();
              }
            };
            arg_names.push(pat);
            arg_types.push(&arg.ty);
          }

          FnArg::Receiver(receiver) => {
            return syn::Error::new_spanned(
              receiver,
              "#[hot] does not support methods with self yet",
            )
            .to_compile_error()
            .into();
          }
        }
      }

      let ret = match &sig.output {
        ReturnType::Default => {
          quote! { () }
        }
        ReturnType::Type(_, ty) => {
          quote! { #ty }
        }
      };

      let fn_name = format!(
        "{}::{}::({})->{}",
        attr,
        sig.ident,
        arg_types
          .iter()
          .map(|ty| (quote! { #ty }).to_string())
          .collect::<Vec<_>>()
          .join(", "),
        ret
      );

      quote! {
        #[unsafe(no_mangle)]
        #vis #sig {
          use std::sync::{LazyLock};
          static FN_NAME: &'static str = #fn_name;
          static FILE_NAME: &'static str = file!();
          fn cb(#(#arg_names: #arg_types),*) -> #ret #body
          static IDX: LazyLock<u16> = LazyLock::new(|| hotfnl::get_fn_idx(FN_NAME, FILE_NAME));
          let callback_list = hotfnl::get_fn_list::<fn(#(#arg_types),*) -> #ret>();
          hotfnl::inventory::submit! {
            crate::hot::HotFn {
              // SAFETY: `cb` is a valid function pointer of exactly this signature; the
              // generic `fn()` erasure is cast back to the concrete type when dispatched.
              func: unsafe { std::mem::transmute(cb as *const()) },
              fn_name: FN_NAME,
              file_name: FILE_NAME,
            }
          }
          let guard = callback_list.read().unwrap();
          let result = guard[*IDX as usize](#(#arg_names),*);
          drop(guard);
          result
        }
      }
    };

    TokenStream::from(expanded)
  }
}

/// Marks an `impl` block as containing hot-patchable methods.
///
/// Used together with `#[hot_method]`; rewrites hot methods so they dispatch through the
/// hot function table. A no-op under the `prod` feature.
#[proc_macro_attribute]
pub fn hot_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
  #[cfg(feature = "prod")]
  {
    let _attr = attr;
    item
  }

  #[cfg(not(feature = "prod"))]
  {
    use proc_macro::Span;
    use proc_macro2::TokenStream as TokenStream2;
    use quote::{ToTokens, quote};
    use syn::{FnArg, ItemImpl, Pat, ReturnType, parse_macro_input};
    let mut input = parse_macro_input!(item as ItemImpl);
    let mut input_hot = {
      let self_ty = input.self_ty.to_token_stream().to_string();
      let generic = input.generics.to_token_stream().to_string();
      let where_clause = input
        .generics
        .where_clause
        .as_ref()
        .map(|wc| wc.to_token_stream().to_string())
        .unwrap_or_default();
      let impl_clone = format!("impl {} {} {} {{}}", generic, self_ty, where_clause);
      syn::parse_str::<ItemImpl>(&impl_clone).unwrap()
    };

    let self_name = token_unwrap!(
      input.self_ty.clone(),
      |input| input
        .to_token_stream()
        .into_iter()
        .next()
        .and_then(|t| match t {
          proc_macro2::TokenTree::Ident(ident) => Some(ident),
          _ => None,
        }),
      "Expected an identifier for the self type in the impl block"
    );

    let self_param: Pat = syn::parse_quote!(self);
    // let (_prefix, self_static) = {
    //   use proc_macro::TokenTree::*;
    //   use proc_macro::token_stream::IntoIter;
    //   struct Data {
    //     iter: IntoIter,
    //     prefix: String,
    //     generic: String,
    //   }
    //   let mut rs = Data {
    //     iter: attr.clone().into_iter(),
    //     prefix: String::new(),
    //     generic: String::from("<>"),
    //   };
    //   impl Data {
    //     fn generic_cb(&mut self) -> Option<()> {
    //       if let Punct(p) = self.iter.next()?
    //         && p.as_char() == '='
    //         && let Punct(p) = self.iter.next()?
    //         && p.as_char() == '<'
    //       {
    //         let mut list = vec!["<".to_string()];
    //         let mut open = 1;
    //         while open > 0 {
    //           let token = self.iter.next()?;
    //           list.push(token.to_string());
    //           if let Punct(p) = token {
    //             match p.as_char() {
    //               '<' => open += 1,
    //               '>' => open -= 1,
    //               _ => {}
    //             };
    //           };
    //         }
    //         self.generic = list.join(" ");
    //       };
    //       self.next()
    //     }
    //     fn prefix_cb(&mut self) -> Option<()> {
    //       if let Punct(p) = self.iter.next()?
    //         && p.as_char() == '='
    //         && let Literal(l) = self.iter.next()?
    //       {
    //         self.prefix = l.to_string();
    //       };
    //       self.next()
    //     }
    //     fn next(&mut self) -> Option<()> {
    //       let i = self.iter.next()?;
    //       match i.to_string().as_str() {
    //         "prefix" => self.prefix_cb(),
    //         "generic" => self.generic_cb(),
    //         _ => self.next(),
    //       }
    //     }
    //   }
    //   rs.next();
    //   (rs.prefix, format!("{}::{}", self_name, rs.generic))
    // };
    for item in &mut input.items {
      let syn::ImplItem::Fn(method) = item else {
        continue;
      };
      let is_hot = method
        .attrs
        .iter()
        .any(|attr| attr.path().is_ident("hot_method"));

      if !is_hot {
        continue;
      }

      let method_name = method.sig.ident.to_string();
      let mut method_clone = method.clone();
      let mut args_types = Vec::new();
      let mut args_names = Vec::new();

      method_clone.vis = syn::Visibility::Inherited;
      method_clone.sig.ident = syn::Ident::new(
        &format!("hot_method_{}", method_name),
        Span::call_site().into(),
      );
      let method_hot_name = &method_clone.sig.ident;
      method.sig.inputs.iter().for_each(|arg| match arg {
        FnArg::Typed(arg) => {
          args_types.push(arg.ty.clone());
          args_names.push(*arg.pat.clone());
        }
        FnArg::Receiver(receiver) => {
          args_names.push(self_param.clone());
          match &receiver.reference {
            Some(_) => {
              if receiver.mutability.is_some() {
                args_types.insert(0, syn::parse_quote!(&mut Self));
              } else {
                args_types.insert(0, syn::parse_quote!(&Self));
              }
            }
            None => {
              args_types.insert(0, syn::parse_quote!(Self));
            }
          }
        }
      });

      let ret = match &method.sig.output {
        ReturnType::Default => {
          quote! { () }
        }
        ReturnType::Type(_, ty) => {
          quote! { #ty }
        }
      };
      let method_name_str = format!(
        "{}::{}::{}::({})->{}",
        attr,
        self_name,
        method_name,
        args_types
          .iter()
          .map(|ty| (quote! { #ty }).to_string())
          .collect::<Vec<_>>()
          .join(", "),
        ret
      );

      let method_static = format!("{}::{}", self_name, method_hot_name);
      let mm_static: TokenStream2 = method_static.parse().unwrap();

      method.block = syn::parse_quote!({
        use std::sync::LazyLock;
        static FN_NAME: &'static str = #method_name_str;
        static FILE_NAME: &'static str = file!();
        static IDX: LazyLock<u16> = LazyLock::new(|| hotfnl::get_fn_idx(FN_NAME, FILE_NAME));
        let callback_list = hotfnl::get_fn_list::<fn(#(#args_types),*) -> #ret>();
        hotfnl::inventory::submit! {
          crate::hot::HotFn {
            func: unsafe { std::mem::transmute(#mm_static as *const()) },
            fn_name: FN_NAME,
            file_name: FILE_NAME,
          }
        }
        let guard = callback_list.read().unwrap();
        let result = guard[*IDX as usize](#(#args_names),*);
        drop(guard);
        result
      });

      method_clone
        .attrs
        .retain(|a| !a.path().is_ident("hot_method"));
      input_hot.items.push(syn::ImplItem::Fn(method_clone));
    }
    let rs = quote! {
      #input_hot
      #input
    };

    rs.into()
  }
}

/// Marks an individual method within a `#[hot_impl]` block as hot-patchable.
///
/// A no-op under the `prod` feature.
#[proc_macro_attribute]
pub fn hot_method(_attr: TokenStream, item: TokenStream) -> TokenStream {
  item
}

/// Transforms `#[dev]` and `#[prod]` attributes into proper `cfg` feature gates.
///
/// `#[dev]` is rewritten to `#[cfg(not(feature = "prod"))]` and `#[prod]` is rewritten
/// to `#[cfg(feature = "prod")]`. Works on any item: functions, structs, enums,
/// individual fields, variants, and `impl` blocks.
///
/// Unlike the other hot-reload macros, `#[hot_check]` always runs regardless of the
/// `prod` feature flag, since its job is to produce correct conditional compilation
/// attributes for both modes.
///
/// # Examples
///
/// ```ignore
/// #[hot_check]
/// #[dev]
/// fn debug_only_function() {
///   // This only exists in hot (non-prod) builds
/// }
///
/// #[hot_check]
/// struct Config {
///   #[prod]
///   production_setting: bool,
///   #[dev]
///   debug_flag: bool,
/// }
/// ```
#[proc_macro_attribute]
pub fn hot_check(_attr: TokenStream, item: TokenStream) -> TokenStream {
  use quote::quote;
  use syn::visit_mut::{self, VisitMut};
  use syn::{Attribute, Item, parse_macro_input};

  struct DevCleaner {
    dev: Attribute,
    prod: Attribute,
  }
  impl Default for DevCleaner {
    fn default() -> Self {
      DevCleaner {
        dev: syn::parse_quote!(#[cfg(not(feature = "prod"))]),
        prod: syn::parse_quote!(#[cfg(feature = "prod")]),
      }
    }
  }
  impl VisitMut for DevCleaner {
    fn visit_attribute_mut(&mut self, i: &mut syn::Attribute) {
      if i.path().is_ident("dev") {
        *i = self.dev.clone();
      } else if i.path().is_ident("prod") {
        *i = self.prod.clone();
      }

      visit_mut::visit_attribute_mut(self, i);
    }
  }

  let mut input = parse_macro_input!(item as Item);
  let mut cleaner = DevCleaner::default();
  cleaner.visit_item_mut(&mut input);
  quote!(#input).into()
}
/// Marks an item as only being compiled in non-prod builds.
#[proc_macro_attribute]
pub fn dev(_attr: TokenStream, item: TokenStream) -> TokenStream {
  use quote::{quote};
  let item = syn::parse_macro_input!(item as syn::Item);
  quote! {
    #[cfg(not(feature = "prod"))]
    #item
  }.into()
}
