#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;
use panic_probe as _;
use stm32f3 as _;

use chrono::channel::Channel;
use chrono::mpsc::{self, Receiver, Sender};

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

#[chrono::main]
async fn main() -> ! {
    static CHANNEL: Channel<&str, chan_size> = mpsc::channel();

    let (tx, rx) = mpsc::split(&CHANNEL);
    let res = chrono::spawn(send(tx));
    let h1 = match res {
        Ok(handle) => handle,
        Err(_) => panic!("Could not spawn task!"),
    };

    let res = chrono::spawn(receive(rx));
    let h2 = match res {
        Ok(handle) => handle,
        Err(_) => panic!("Could not spawn task!"),
    };

    let _ = h1.await;
    let _ = h2.await;

    defmt::info!("Success!");
}
