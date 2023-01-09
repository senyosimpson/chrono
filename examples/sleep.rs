#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;
use panic_probe as _;
use stm32f3 as _;

use chrono::time::{sleep, Duration, Instant};

#[chrono::alloc(size = 2)]
async fn delay(duration: Duration) {
    sleep(duration).await;
}

#[chrono::main]
fn main() -> ! {
    let now = Instant::now();

    let h1 = chrono::spawn(delay(Duration::from_secs(5))).unwrap();
    let h2 = chrono::spawn(delay(Duration::from_secs(1))).unwrap();

    h2.await;
    h1.await;

    let later = Instant::now();
    let elapsed = later - now;
    defmt::info!("Woke from sleep! {} seconds elapsed", elapsed.as_secs());
}
