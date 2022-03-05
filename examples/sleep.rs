use core::time::Duration;
use std::time::Instant;

use chrono::time::sleep;
use chrono::Runtime;

fn main() {
    tracing_subscriber::fmt::init();

    let rt = Runtime::new();
    rt.block_on(async {
        let now = Instant::now();
        let handle = chrono::spawn(async {
            println!("Sleeping for 5 seconds!");
            sleep(Duration::from_secs(5)).await;
        });

        let _ = handle.await;

        let later = Instant::now();
        let elapsed = later - now;
        println!(
            "Waking from sleep! {}:{} elapsed",
            elapsed.as_secs(),
            elapsed.subsec_millis()
        );
    })
}
