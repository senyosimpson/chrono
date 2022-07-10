use core::cell::RefCell;

use super::duration::Duration;
use crate::hal::prelude::*;
use crate::hal::pac::{self, interrupt, TIM2};
use crate::hal::rcc::{self, Clocks};
use crate::hal::timer::{Event, Timer};

pub(crate) static mut DRIVER: Driver = Driver::new();

/// Driver for timers
pub struct Driver {
    initialised: bool,
    inner: Option<RefCell<Inner>>,
}

struct Inner {
    timer: Timer<pac::TIM2>,
}

pub fn driver() -> &'static mut Driver {
    unsafe { &mut DRIVER }
}

// Safe since we are in a single-threaded environment
unsafe impl Sync for Driver {}

impl Driver {
    pub const fn new() -> Driver {
        Driver {
            inner: None,
            initialised: false,
        }
    }

    pub fn init(&mut self, tim: TIM2, clocks: Clocks, apb: &mut <TIM2 as rcc::RccBus>::Bus) {
        self.inner = Some(RefCell::new(Inner::new(tim, clocks, apb)));
        self.initialised = true;
    }

    pub fn start(&mut self, deadline: Duration) {
        assert!(
            self.initialised,
            "initialise timer before usage via to .init()"
        );

        let mut inner = self.inner.as_ref().unwrap().borrow_mut();
        let deadline = deadline.as_micros().microseconds();
        inner.timer.start(deadline);
    }

    pub fn handle_interrupt(&mut self) {
        cortex_m::interrupt::free(|_cs| {
            defmt::debug!("INTERRUPT");
            let mut inner = self.inner.as_ref().unwrap().borrow_mut();
            inner.timer.clear_event(Event::Update);
            inner.timer.stop();
            cortex_m::asm::sev();
        })
    }


}

impl Inner {
    pub fn new(tim: TIM2, clocks: Clocks, apb: &mut <TIM2 as rcc::RccBus>::Bus) -> Inner {
        // TODO: This should actually take in peripherals since we will have more
        // than one at some point
        // let peripherals = Peripherals::take().unwrap();

        // This is a workaround, so that the debugger will not disconnect immediately on asm::wfe();
        // https://github.com/probe-rs/probe-rs/issues/350#issuecomment-740550519
        // peripherals.DBGMCU.cr.modify(|_, w| {
        //     w.dbg_sleep().set_bit();
        //     w.dbg_standby().set_bit();
        //     w.dbg_stop().set_bit()
        // });

        // let mut core_peripherals = CorePeripherals::take().unwrap();

        // let mut rcc = peripherals.RCC.constrain();
        // let cfg = rcc.cfgr.hclk(1.MHz());
        // let mut flash = peripherals.FLASH.constrain();
        // let clocks = cfg.freeze(&mut flash.acr);

        let mut timer = Timer::new(tim, clocks, apb);

        // Enable timer interrupts on the chip itself
        unsafe {
            cortex_m::peripheral::NVIC::unmask(timer.interrupt());
        }
        // Enable timer interrupt
        timer.enable_interrupt(Event::Update);

        Inner { timer }
    }
}

/// Set up the interrupt for the timer
#[interrupt]
fn TIM2() {
    unsafe { DRIVER.handle_interrupt() };
}
