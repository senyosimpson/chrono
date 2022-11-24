#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;
use panic_probe as _;
use stm32f3 as _;

use chrono::time::{sleep, Duration, Instant};

#[chrono::alloc]
async fn delay() {
    let t = 5;
    defmt::info!("Sleeping for {} seconds!", t);
    sleep(Duration::from_secs(t)).await;
}

#[chrono::main]
fn main() -> ! {
    let now = Instant::now();
    let res = chrono::spawn(delay());
    let handle = match res {
        Ok(handle) => handle,
        Err(_) => panic!("Could not spawn task!"),
    };

    handle.await;

    let later = Instant::now();
    let elapsed = later - now;
    defmt::info!("Woke from sleep! {} seconds elapsed", elapsed.as_secs());
}
