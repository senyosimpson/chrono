use crate::hal::delay::Delay;
use crate::hal::pac;
use crate::hal::prelude::*;
use crate::hal::spi::Spi;
use crate::net::stack;
use crate::net::devices::Enc28j60;
use crate::time;

pub fn init() {
    defmt::debug!("Initialising system");

    let peripherals = unsafe { pac::Peripherals::steal() };

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
    let mut core_peripherals = unsafe { pac::CorePeripherals::steal() };
    core_peripherals.DCB.enable_trace();
    core_peripherals.DWT.enable_cycle_counter();
    drop(core_peripherals.DWT);

    // init time driver
    defmt::debug!("Initialised time driver");
    time::driver().init(peripherals.TIM2, clocks, &mut rcc.apb1);

    #[cfg(feature = "networking")]
    {
        const KB: u16 = 1024; // bytes
        const RX_BUF_SIZE: u16 = 7 * KB;
        const MAC_ADDR: [u8; 6] = [0x2, 0x3, 0x4, 0x5, 0x6, 0x7];

        let mut gpioa = peripherals.GPIOA.split(&mut rcc.ahb);

        // SPI
        let mut ncs = gpioa
            .pa4
            .into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper);
        if let Err(_) = ncs.set_high() {
            panic!("Failed to drive ncs pin high");
        }

        let sck = gpioa
            .pa5
            .into_af_push_pull(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrl);
        let mosi =
            gpioa
                .pa7
                .into_af_push_pull(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrl);
        let miso =
            gpioa
                .pa6
                .into_af_push_pull(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrl);

        let spi = Spi::new(
            peripherals.SPI1,
            (sck, miso, mosi),
            1.MHz(),
            clocks,
            &mut rcc.apb2,
        );

        // ENC28J60
        let mut reset = gpioa
            .pa3
            .into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper);
        if let Err(_) = reset.set_high() {
            panic!("Failed to drive reset pin high");
        }

        let mut delay = Delay::new(core_peripherals.SYST, clocks);
        let enc28j60 = match enc28j60::Enc28j60::new(
            spi,
            ncs,
            enc28j60::Unconnected,
            reset,
            &mut delay,
            RX_BUF_SIZE,
            MAC_ADDR,
        ) {
            Ok(d) => d,
            Err(_) => panic!("Could not initialise driver"),
        };

        delay.delay_ms(100_u8);

        defmt::debug!("Initialised ethernet device");

        let device = Enc28j60::new(enc28j60);
        stack().init(device);
    }
}
