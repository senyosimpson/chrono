#![no_std]
#![no_main]
#![feature(type_alias_impl_trait, generic_associated_types)]

use defmt_rtt as _;
use panic_probe as _;
use stm32f3 as _;

use chrono::mpsc::{self, Channel};
use chrono::mpsc::bounded::{Sender, Receiver};
use chrono::Runtime;

const CHAN_SIZE: usize = 32;
static CHANNEL: Channel<&str, CHAN_SIZE> = mpsc::bounded::channel();

#[chrono::alloc]
async fn send(tx: Sender<'static, &str, CHAN_SIZE>) -> u8 {
    defmt::info!("Sending message from task 1");
    tx.send("task 1: fly.io").await.unwrap();
    5
}

#[chrono::alloc]
async fn receive(rx: Receiver<'static, &str, CHAN_SIZE>) {
    defmt::info!("Received message: {}", rx.recv().await.unwrap());
}

#[cortex_m_rt::entry]
fn main() -> ! {
    let rt = Runtime::new();

    rt.block_on(async {
        let (tx, rx) = mpsc::bounded::split(&CHANNEL);
        let res = chrono::spawn(send(tx.clone()));
        let handle = match res {
            Ok(handle) => handle,
            Err(_) => panic!("Could not spawn task!"),
        };
        let _output = handle.await;

        let res = chrono::spawn(receive(rx));
        let handle = match res {
            Ok(handle) => handle,
            Err(_) => panic!("Could not spawn task!"),
        };
        let _output = handle.await;
    });

    defmt::info!("Success!");
    loop { cortex_m::asm::bkpt(); }
}
