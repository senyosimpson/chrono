use proc_macro::TokenStream;
use quote::quote;

pub(crate) fn main(f: syn::ItemFn) -> TokenStream {
    let fn_body = f.block;

    quote! {
        async fn fut() #fn_body

        #[cortex_m_rt::entry]
        unsafe fn main() -> ! {
            ::chrono::init();

            static mut RT: ::chrono::Runtime = ::chrono::Runtime::new();
            RT.block_on(fut());

            loop {
                cortex_m::asm::bkpt();
            }
        }
    }
    .into()
}
