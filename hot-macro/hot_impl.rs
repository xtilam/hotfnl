use proc_macro::{Span, TokenStream};
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, quote};
use syn::{FnArg, ItemImpl, Pat, ReturnType, parse_macro_input};

macro_rules! token_err {
  ($($el:tt)*) => {
    syn::Error::new_spanned($($el)*)
      .to_compile_error()
      .into()
  };
}
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
pub fn hot_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
  #[cfg(feature = "prod")]
  return item;

  #[cfg(not(feature = "prod"))]
  {
    let mut input = parse_macro_input!(item as ItemImpl);
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

    let mut body = Vec::new();
    let self_param: Pat = syn::parse_quote!(self);
    let (prefix, self_static) = {
      let mut self_static = self_name.to_string();
      let mut prefix = "";
      //
      // type IterType = std::vec::IntoIter<proc_macro::TokenTree>;
      // let mut iter: Iterator<proc_macro::TokenTree> = attr.clone().into_iter();
      // let next = |iter: IterType| -> Option<IterType> {
      //   let token = iter.next()?;
      //   match token {
      //     proc_macro::TokenTree::Ident(ident) => match ident.to_string().as_str() {
      //       "prefix" => Some(iter),
      //       _ => Some(iter),
      //     },
      //     _ => Some(iter),
      //   }
      // };
      // match next(iter) {
      //   Some(iter) => {
      //     next(iter);
      //   }
      //   None => {}
      // };
      // let next_prefix = || -> Option<()> {
      //   let token = attr.clone().into_iter().next()?;
      //   next()
      // // };
      // next(next);
      (prefix, self_static)
    };
    // let self_static = match attr.clone().to_string().as_str() {
    //   "" => self_name.to_string(),
    //   name => {
    //     format!("{}::{}", self_name, name)
    //   }
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

      println!(
        "method: {}, args_types: {:?}, args_names: {}",
        method_name,
        args_types
          .iter()
          .map(|x| x.to_token_stream().to_string())
          .collect::<Vec<_>>()
          .join(", "),
        args_names
          .iter()
          .map(|x| x.to_token_stream().to_string())
          .collect::<Vec<_>>()
          .join(", ")
      );
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
      println!(
        "method_static: {}, mm_static: {:?}",
        method_static, mm_static
      );

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

      body.push(syn::ImplItem::Fn(method_clone));
    }
    println!("body: {:?}", body.len());
    for item in body {
      input.items.push(item);
    }

    quote! { #input }.into()
  }
}

#[proc_macro_attribute]
pub fn hot_method(_attr: TokenStream, item: TokenStream) -> TokenStream {
  item
}
