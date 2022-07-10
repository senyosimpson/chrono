use proc_macro::TokenStream;

mod alloc;
mod main;

#[proc_macro_attribute]
pub fn alloc(_: TokenStream, item: TokenStream) -> TokenStream {
    let f: syn::ItemFn = syn::parse(item).expect("Could not parse input tokenstream");
    alloc::alloc(f)
}

#[proc_macro_attribute]
pub fn main(_: TokenStream, item: TokenStream) -> TokenStream {
    let f: syn::ItemFn = syn::parse(item).expect("Could not parse input tokenstream");
    main::main(f)
}