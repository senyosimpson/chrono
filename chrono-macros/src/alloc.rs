use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::ReturnType;

pub(super) fn alloc(f: syn::ItemFn) -> TokenStream {
    let mut arg_names = Vec::new();
    let mut fn_args = f.sig.inputs.clone();

    for arg in fn_args.iter_mut() {
        match arg {
            syn::FnArg::Receiver(_) => {}
            syn::FnArg::Typed(t) => {
                if let syn::Pat::Ident(id) = t.pat.as_mut() {
                    arg_names.push(id.ident.clone());
                    id.mutability = None;
                }
            }
        }
    }

    let inputs = f.sig.inputs.clone();
    let output = f.sig.output.clone();

    let fn_name = f.sig.ident.clone();
    let inner_fn_name = format_ident!("__{}_task", fn_name);
    let mut inner_fn = f;

    let fn_args = inputs;
    let visibility = inner_fn.vis.clone();
    inner_fn.vis = syn::Visibility::Inherited;
    inner_fn.sig.ident = inner_fn_name.clone();

    let impl_ty = {
        match output.clone() {
            ReturnType::Default => {
                quote!(impl ::core::future::Future<Output = ()>)
            }
            ReturnType::Type(_, ret) => {
                quote!(impl ::core::future::Future<Output = #ret>)
            }
        }
    };

    let fn_ret = {
        match output.clone() {
            ReturnType::Default => {
                quote!(::chrono::task::RawTask<#impl_ty, ()>)
            }
            ReturnType::Type(_, ret) => {
                quote!(::chrono::task::RawTask<#impl_ty, #ret>)
            }
        }
    };

    let memory_type = {
        match output {
            ReturnType::Default => {
                quote!(Memory<F, ()>)
            }
            ReturnType::Type(_, ret) => {
                quote!(Memory<F, #ret>)
            }
        }
    };

    quote! {
        #inner_fn

        #visibility fn #fn_name(#fn_args) -> #fn_ret {
            use ::chrono::task::Memory;

            type F = #impl_ty;

            fn launder_tait(task: #fn_ret) -> #fn_ret {
                task
            }

            static MEMORY: #memory_type = Memory::alloc();
            launder_tait(::chrono::task::RawTask::new(&MEMORY, move || #inner_fn_name(#(#arg_names,)*)))
        }
    }
    .into()
}
