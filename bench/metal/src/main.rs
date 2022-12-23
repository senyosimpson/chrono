use std::time::Duration;

use std::io;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::task::JoinSet;
use tokio::time::sleep;

async fn blacksmith(id: u8) -> Result<(), io::Error> {
    println!("conn {id} starting");
    let messages = ["hello chrono", "chrono hello"];
    let mut stream = TcpStream::connect("192.168.69.1:7777").await?;
    let mut i = 0;
    let mut buf = [0u8; 12];

    loop {
        i += 1;

        let choice = if i % 2 == 0 { 0 } else { 1 };
        let message = messages[choice];

        stream.write_all(message.as_bytes()).await?;
        stream.read(&mut buf).await?;

        sleep(Duration::from_secs(1)).await;
        println!("conn {}: {}", id, std::str::from_utf8(&buf).unwrap());
    }
}

#[tokio::main]
async fn main() -> Result<(), io::Error> {
    let blacksmiths = 32;
    println!("Creating {blacksmiths} blacksmiths");
    let mut set = JoinSet::new();

    for id in 0..blacksmiths {
        set.spawn(blacksmith(id));
    }

    set.join_next().await;

    Ok(())
}
