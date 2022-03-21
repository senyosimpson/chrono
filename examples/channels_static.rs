#![feature(type_alias_impl_trait, generic_associated_types)]

use chrono::channel::Receiver;
use chrono::{RawTask};
use chrono::channel::mpsc::{Sender, self};
use chrono::Runtime;
use chrono::runtime::Queue;


#[chrono::alloc]
async fn send(tx: Sender<String>) {
    println!("Sending message from task 1");
    tx.send("task 1: fly.io".into()).unwrap()
}

#[chrono::alloc]
async fn receive(rx: Receiver<String>) {
    println!("Received message: {}", rx.recv().await.unwrap());
}

fn main() {
    // tracing_subscriber::fmt::init();

    let rt = Runtime::new();
    rt.block_on(async {
        let (tx, rx) = mpsc::channel();
        let _ = chrono::spawn(send(tx.clone())).await;
        let _ = chrono::spawn(receive(rx)).await;
    });
    // rt.block_on(async {
    //     let (tx, rx) = mpsc::channel();
    //     chrono::spawn(async {
    //         // const RAW = chrono::task::raw::empty();
    //         // static STORAGE: [ ;1]
    //         let tx = tx.clone();
    //         println!("Sending message from task 1");
    //         tx.send("task 1: fly.io").unwrap()
    //     });

    //     let h1 = chrono::spawn(async move {
    //         println!("Sending message from task 3 after sleeping");
    //         sleep(Duration::from_secs(1)).await;
    //         println!("Done sleeping. Sending message from task 2");
    //         tx.send("handle 2: hello world").unwrap();
    //     });

    //     chrono::spawn(async move {
    //         println!("Received message: {}", rx.recv().await.unwrap());
    //         println!("Received message: {}", rx.recv().await.unwrap());
    //     });

    //     h1.await.unwrap();
    // });

    println!("Finished")
}
