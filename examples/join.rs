#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;
use panic_probe as _;
use stm32f3 as _;

use chrono::channel::Channel;
use chrono::mpsc::{self, Receiver, Sender};
use chrono::time::{sleep, Duration};

#[allow(non_upper_case_globals)]
const chan_size: usize = 2;

#[chrono::alloc]
async fn send1(tx: Sender<'static, &str, chan_size>) {
    defmt::info!("Sending message from task 1");
    tx.send("hello").unwrap();
}

#[chrono::alloc]
async fn send2(tx: Sender<'static, &str, chan_size>) {
    defmt::info!("Sending message from handle one after sleeping");
    sleep(Duration::from_secs(1)).await;
    defmt::info!("Done sleeping. Sending message from handle one");
    tx.send("hello world").unwrap();
    defmt::info!("Sent message!")
}

#[chrono::alloc]
async fn receive(rx: Receiver<'static, &str, chan_size>) {
    defmt::info!("Received message: {}", rx.recv().await.unwrap());
    defmt::info!("Received message: {}", rx.recv().await.unwrap());
}

#[chrono::main]
async fn main() {
    static CHANNEL: Channel<&str, chan_size> = mpsc::channel();
    let (tx, rx) = mpsc::split(&CHANNEL);

    let h1 = chrono::spawn(send1(tx.clone())).unwrap();
    let h2 = chrono::spawn(send2(tx)).unwrap();
    let h3 = chrono::spawn(receive(rx)).unwrap();

    let _ = chrono::join!(h1, h2, h3);

    defmt::info!("Finished")
}
