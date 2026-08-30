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
    let expanded = {
      quote! {
        hotfnl::use_hot!();
        #[allow(dead_code)]
        #vis #sig {
          {
            let list_fn: Vec<hotfnl::HotFn> = inventory::iter::<hot::HotFn>
              .into_iter()
              .map(|f| hotfnl::HotFn {
                file_name: f.file_name,
                fn_name: f.fn_name,
                func: f.func,
                ptr: None,
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
    let expanded: _ = {
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
        attr.to_string(),
        &sig.ident.to_string(),
        arg_types
          .iter()
          .map(|ty| (quote! { #ty }).to_string())
          .collect::<Vec<_>>()
          .join(", "),
        ret.to_string()
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
          inventory::submit! {
            hot::HotFn {
              func: unsafe { std::mem::transmute(cb as *const()) },
              fn_name: FN_NAME,
              file_name: FILE_NAME,
            }
          }
          callback_list.read().unwrap()[*IDX as usize](#(#arg_names),*)
        }
      }
    };

    TokenStream::from(expanded)
  }
}


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

    // Đây chính là App
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
    let (_prefix, self_static) = {
      use proc_macro::TokenTree::*;
      use proc_macro::token_stream::IntoIter;

      struct Data {
        iter: IntoIter,
        prefix: String,
        generic: String,
      }

      fn prefix_cb(d: &mut Data) -> Option<()> {
        if let Punct(p) = d.iter.next()? {
          if p.as_char() != '=' {
            return next(d);
          }
        };
        if let Literal(l) = d.iter.next()? {
          d.prefix = l.to_string();
        };
        return next(d);
      }
      fn generic_cb(d: &mut Data) -> Option<()> {
        let mut list = Vec::new();
        let mut open = 0;
        if let Punct(p) = d.iter.next()? {
          if p.as_char() != '=' {
            return next(d);
          }
        };
        loop {
          let token = d.iter.next()?;
          let str = token.to_string();
          match token {
            Punct(p) => {
              match p.as_char() {
                '<' => open += 1,
                '>' => open -= 1,
                _ => {}
              };
            }
            _ => {}
          };
          list.push(str);
          if open == 0 {
            break;
          }
        }
        let generic_str = list.join(" ");
        if !generic_str.is_empty() {
          d.generic = generic_str;
        }
        return next(d);
      }

      fn next(d: &mut Data) -> Option<()> {
        match d.iter.next()? {
          Ident(i) => match i.to_string().as_str() {
            "prefix" => return prefix_cb(d),
            "generic" => return generic_cb(d),
            _ => {}
          },
          _ => {}
        };

        next(d)
      }

      let mut rs = Data {
        iter: attr.clone().into_iter(),
        prefix: String::new(),
        generic: String::from("<>"),
      };
      next(&mut rs);
      (rs.prefix, format!("{}::{}", self_name, rs.generic))
    };
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
        &format!("hot_method_{}", &method_name),
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
        "{}::{}::({})->{}",
        self_name,
        method_name,
        args_types
          .iter()
          .map(|ty| (quote! { #ty }).to_string())
          .collect::<Vec<_>>()
          .join(", "),
        ret.to_string()
      );

      let method_static = format!("{}::{}", self_static, method_hot_name.to_string());
      let mm_static: TokenStream2 = method_static.parse().unwrap();

      method.block = syn::parse_quote!({
        use std::sync::LazyLock;
        static FN_NAME: &'static str = #method_name_str;
        static FILE_NAME: &'static str = file!();
        static IDX: LazyLock<u16> = LazyLock::new(|| hotfnl::get_fn_idx(FN_NAME, FILE_NAME));
        let callback_list = hotfnl::get_fn_list::<fn(#(#args_types),*) -> #ret>();
        inventory::submit! {
          hot::HotFn {
            func: unsafe { std::mem::transmute(#mm_static as *const()) },
            fn_name: FN_NAME,
            file_name: FILE_NAME,
          }
        }
        callback_list.read().unwrap()[*IDX as usize](#(#args_names),*)
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

#[proc_macro_attribute]
pub fn hot_method(_attr: TokenStream, item: TokenStream) -> TokenStream {
  item
}
