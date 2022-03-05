use core::time::Duration;

use chrono::channel::mpsc;
use chrono::time::sleep;
use chrono::Runtime;

fn main() {
    tracing_subscriber::fmt::init();

    let rt = Runtime::new();
    rt.block_on(async {
        let (tx, rx) = mpsc::channel();

        let h1 = chrono::spawn(async {
            let tx = tx.clone();
            println!("Sending message from handle 1");
            tx.send("hello").unwrap()
        });

        let h2 = chrono::spawn(async move {
            println!("Sending message from handle one after sleeping");
            sleep(Duration::from_secs(1)).await;
            println!("Done sleeping. Sending message from handle one");
            tx.send("hello world").unwrap();
            println!("Sent message!");
        });

        let h3 = chrono::spawn(async move {
            println!("Received message: {}", rx.recv().await.unwrap());
            println!("Received message: {}", rx.recv().await.unwrap());
        });

        let _ = chrono::join!(h1, h2, h3);
    });

    println!("Finished")
}
