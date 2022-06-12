#![no_std]
#![no_main]
#![feature(type_alias_impl_trait, generic_associated_types)]

use defmt_rtt as _;
use panic_probe as _;
use stm32f3 as _;

use chrono::time::{sleep, Duration, Instant};
use chrono::Runtime;

#[chrono::alloc]
async fn delay() {
    let t = 5;
    defmt::info!("Sleeping for {} seconds!", t);
    sleep(Duration::from_secs(t)).await;
}

#[cortex_m_rt::entry]
fn main() -> ! {
    let rt = Runtime::new();
    rt.block_on(async {
        let now = Instant::now();
        let res = chrono::spawn(delay());
        let handle = match res {
            Ok(handle) => handle,
            Err(_) => panic!("Could not spawn task!"),
        };

        handle.await;

        let later = Instant::now();
        let elapsed = later - now;
        defmt::info!("Woke from sleep! {} elapsed", elapsed.as_secs());
    });

    defmt::info!("Success!");
    loop {
        cortex_m::asm::bkpt();
    }
}
