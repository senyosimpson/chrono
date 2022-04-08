use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::ReturnType;

pub(super) fn alloc(mut f: syn::ItemFn) -> TokenStream {
    let mut arg_names: syn::punctuated::Punctuated<syn::Ident, syn::Token![,]> =
        syn::punctuated::Punctuated::new();
    let mut fn_args = f.sig.inputs.clone();

    for arg in fn_args.iter_mut() {
        match arg {
            syn::FnArg::Receiver(_) => {}
            syn::FnArg::Typed(t) => {
                if let syn::Pat::Ident(i) = t.pat.as_mut() {
                    arg_names.push(i.ident.clone());
                    i.mutability = None;
                }
            }
        }
    }

    let fn_name = f.sig.ident.clone();

    let fn_args = f.sig.inputs.clone();
    let visibility = &f.vis;
    let attrs = &f.attrs;

    f.sig.ident = format_ident!("task");

    let impl_ty = {
        match f.sig.output.clone() {
            ReturnType::Default => {
                quote!(impl ::core::future::Future<Output = ()>)
            }
            ReturnType::Type(_, ret) => {
                quote!(impl ::core::future::Future<Output = #ret>)
            }
        }
    };

    let fn_ret = {
        match f.sig.output.clone() {
            ReturnType::Default => {
                quote!(::chrono::task::RawTask<#impl_ty, (), heapless::Arc<::chrono::runtime::RunQueue>>)
            }
            ReturnType::Type(_, ret) => {
                quote!(::chrono::task::RawTask<#impl_ty, #ret, heapless::Arc<::chrono::runtime::RunQueue>>)
            }
        }
    };

    let memory_type = {
        match f.sig.output.clone() {
            ReturnType::Default => {
                quote!(Memory<F, (), heapless::Arc<::chrono::runtime::RunQueue>>)
            }
            ReturnType::Type(_, ret) => {
                quote!(Memory<F, #ret, heapless::Arc<::chrono::runtime::RunQueue>>)
            }
        }
    };

    quote! {
        #(#attrs)*
        #visibility fn #fn_name(#fn_args) -> #fn_ret {
            use ::chrono::task::Memory;

            #f

            type F = #impl_ty;

            static MEMORY: #memory_type = Memory::alloc();
            ::chrono::task::RawTask::new(&MEMORY, move || task(#arg_names))
        }
    }
    .into()
}
