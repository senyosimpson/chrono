#![no_std]
#![no_main]
#![feature(type_alias_impl_trait, generic_associated_types)]

use defmt_rtt as _;
use panic_probe as _;
use stm32f3 as _;

use chrono::channel::Channel;
use chrono::mpsc::{self, Receiver, Sender};
use chrono::Runtime;

#[allow(non_upper_case_globals)]
const chan_size: usize = 2;

#[chrono::alloc]
async fn send(tx: Sender<'static, &str, chan_size>) -> u8 {
    defmt::info!("Sending message from task 1");
    tx.send("task 1: fly.io").unwrap();
    5
}

#[chrono::alloc]
async fn receive(rx: Receiver<'static, &str, chan_size>) {
    defmt::info!("Received message: {}", rx.recv().await.unwrap());
}

#[allow(non_upper_case_globals)]
#[cortex_m_rt::entry]
fn main() -> ! {
    let rt = Runtime::new();

    rt.block_on(async {
        static channel: Channel<&str, chan_size> = mpsc::channel();

        let (tx, rx) = mpsc::split(&channel);
        let res = chrono::spawn(send(tx));
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
    loop {
        cortex_m::asm::bkpt();
    }
}
