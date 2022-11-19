use proc_macro::TokenStream;

mod alloc;
mod main;

#[proc_macro_attribute]
pub fn alloc(args: TokenStream, item: TokenStream) -> TokenStream {
    let f = syn::parse_macro_input!(item);
    let args = syn::parse_macro_input!(args);
    alloc::alloc(args, f)
}

#[proc_macro_attribute]
pub fn main(_: TokenStream, item: TokenStream) -> TokenStream {
    let f = syn::parse_macro_input!(item);
    main::main(f)
}
