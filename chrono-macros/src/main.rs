use proc_macro::TokenStream;
use quote::quote;

pub(super) fn main(f: syn::ItemFn) -> TokenStream {
    let fn_body = f.block;

    let hal_setup = quote! {
        use ::chrono::hal::prelude::*;
        let peripherals = ::chrono::hal::pac::Peripherals::steal();

        // This is a workaround, so that the debugger will not disconnect immediately on asm::wfe();
        // https://github.com/probe-rs/probe-rs/issues/350#issuecomment-740550519
        peripherals.DBGMCU.cr.modify(|_, w| {
            w.dbg_sleep().set_bit();
            w.dbg_standby().set_bit();
            w.dbg_stop().set_bit()
        });


        let mut rcc = peripherals.RCC.constrain();
        let cfg = rcc.cfgr.hclk(1.MHz());
        let mut flash = peripherals.FLASH.constrain();
        let clocks = cfg.freeze(&mut flash.acr);

        // Setup mono timer. Copied from MonoTimer::new() in stm32 hal crate
        let mut core_peripherals = ::chrono::hal::pac::CorePeripherals::steal();
        core_peripherals.DCB.enable_trace();
        core_peripherals.DWT.enable_cycle_counter();
        drop(core_peripherals.DWT);

        // init time driver
        ::chrono::time::driver().init(peripherals.TIM2, clocks, &mut rcc.apb1);
    };

    quote! {
        async fn fut() #fn_body

        #[cortex_m_rt::entry]
        unsafe fn main() -> ! {
            #hal_setup

            static mut RT: ::chrono::Runtime = ::chrono::Runtime::new();
            RT.block_on(fut());

            loop {
                cortex_m::asm::bkpt();
            }
        }
    }
    .into()
}
