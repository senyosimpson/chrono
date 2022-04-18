#![feature(type_alias_impl_trait, generic_associated_types)]
#![no_std]

use chrono::channel::mpsc::{self, ChannelCell, Receiver, Sender};
use chrono::Runtime;
use chrono::runtime::SpawnError;

const CHAN_SIZE: usize = 32;
static CHANNEL: ChannelCell<&str, CHAN_SIZE> = ChannelCell::new();

#[chrono::alloc]
async fn send(tx: Sender<'static, &str, CHAN_SIZE>) -> u8 {
    // println!("Sending message from task 1");
    tx.send("task 1: fly.io").unwrap();
    5
}

#[chrono::alloc]
async fn receive(rx: Receiver<'static, &str, CHAN_SIZE>) {
    // println!("Received message: {}", rx.recv().await.unwrap());
}

fn main() {
    let rt = Runtime::new();
    rt.block_on(async {
        let channel = CHANNEL.set(mpsc::Channel::new());
        let (tx, rx) = mpsc::split(channel);
        let res = chrono::spawn(send(tx.clone()));
        let handle = match res {
            Ok(handle) => handle,
            Err(_) => panic!()

        };
        let output = handle.await;

        // let _ = chrono::spawn(receive(rx)).await;
    });
}
