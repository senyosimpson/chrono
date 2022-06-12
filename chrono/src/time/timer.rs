use core::cell::RefCell;
use core::ops::{Deref, DerefMut};

use stm32f3xx_hal::pac::{self, interrupt, CorePeripherals, Peripherals};
use stm32f3xx_hal::prelude::*;
use stm32f3xx_hal::timer::{Event, MonoTimer, Timer as HardwareTimer};

use super::duration::Duration;

pub(crate) static mut TIMER: Timer = Timer::new();

pub struct Timer {
    initialised: bool,
    inner: Option<RefCell<Inner>>,
}

pub struct Inner {
    timer: HardwareTimer<pac::TIM2>,
    monotimer: MonoTimer,
}

unsafe impl Sync for Timer {}

pub fn timer() -> &'static mut Timer {
    unsafe { &mut TIMER }
}

impl Timer {
    pub const fn new() -> Timer {
        Timer {
            inner: None,
            initialised: false,
        }
    }

    pub fn init(&mut self) {
        self.inner = Some(RefCell::new(Inner::new()));
        self.initialised = true;
    }

    pub fn start(&mut self, deadline: Duration) {
        // Grab the deadline earlier because it also borrows the timer
        let deadline = deadline.as_micros().microseconds();

        let mut inner = self.inner.as_ref().unwrap().borrow_mut();
        // Change to a smaller unit of time
        inner.timer.start(deadline);
    }

    pub fn ticks_per_second(&self) -> u32 {
        assert!(
            self.initialised,
            "initialise timer before usage via to .init()"
        );

        let inner = self.inner.as_ref().unwrap().borrow();
        let freq = inner.monotimer.frequency();
        freq.0
    }

    pub fn handle_interrupt(&self) {
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
    pub fn new() -> Inner {
        // TODO: This should actually take in peripherals since we will have more
        // than one at some point
        let peripherals = Peripherals::take().unwrap();

        // This is a workaround, so that the debugger will not disconnect immediately on asm::wfe();
        // https://github.com/probe-rs/probe-rs/issues/350#issuecomment-740550519
        peripherals.DBGMCU.cr.modify(|_, w| {
            w.dbg_sleep().set_bit();
            w.dbg_standby().set_bit();
            w.dbg_stop().set_bit()
        });

        let mut core_peripherals = CorePeripherals::take().unwrap();

        let mut rcc = peripherals.RCC.constrain();
        let cfg = rcc.cfgr.hclk(1.MHz());
        let mut flash = peripherals.FLASH.constrain();
        let clocks = cfg.freeze(&mut flash.acr);

        let mut timer = HardwareTimer::new(peripherals.TIM2, clocks, &mut rcc.apb1);
        // Can remove this and just do the setup manually for DWT
        let monotimer = MonoTimer::new(core_peripherals.DWT, clocks, &mut core_peripherals.DCB);

        // Enable timer interrupts on the chip itself
        unsafe {
            cortex_m::peripheral::NVIC::unmask(timer.interrupt());
        }
        // Enable timer interrupt
        timer.enable_interrupt(Event::Update);

        Inner { timer, monotimer }
    }
}

impl Deref for Inner {
    type Target = HardwareTimer<pac::TIM2>;

    fn deref(&self) -> &Self::Target {
        &self.timer
    }
}

impl DerefMut for Inner {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.timer
    }
}

/// Set up the interrupt for the timer
#[interrupt]
fn TIM2() {
    unsafe { TIMER.handle_interrupt() };
}
