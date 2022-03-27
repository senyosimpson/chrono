use proc_macro::TokenStream;
use quote::{format_ident, quote};

pub(super) fn alloc(mut f: syn::ItemFn) -> TokenStream {
    let mut arg_names: syn::punctuated::Punctuated<syn::Ident, syn::Token![,]> =
        syn::punctuated::Punctuated::new();
    let mut fn_args = f.sig.inputs.clone();

    for arg in fn_args.iter_mut() {
        // if let syn::FnArg::Typed(t) = arg {

        // }
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

    let impl_ty = quote!(impl ::core::future::Future);

    quote! {
        #(#attrs)*
        #visibility fn #fn_name(#fn_args) -> ::chrono::task::RawTask<#impl_ty, ::chrono::runtime::Queue> {
            use ::chrono::task::Memory;
            #f

            type F = #impl_ty;

            static MEMORY: Memory<F, ::chrono::runtime::Queue> = Memory::alloc();
            ::chrono::task::RawTask::new(&MEMORY, move || task(#arg_names))
        }
    }.into()
}
