use std::io;
use std::time::Duration;

use clap::Parser;
use rand::Rng;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::task::JoinSet;
use tokio::time::sleep;

/// A load testing tool for Chrono
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Number of concurrent workers
    #[arg(long, default_value_t = 1)]
    workers: u8,
}

async fn worker(id: u8, message: Vec<u8>) -> Result<(), io::Error> {
    println!("Worker {id} starting");

    let mut stream = TcpStream::connect("192.168.69.1:7777").await?;
    let mut buf = [0u8; 12];

    loop {
        stream.write_all(&message).await?;
        stream.read(&mut buf).await?;
        println!("Worker {}: {}", id, std::str::from_utf8(&buf).unwrap());

        sleep(Duration::from_secs(1)).await;
    }
}

#[tokio::main]
async fn main() -> Result<(), io::Error> {
    let args = Args::parse();
    let workers = args.workers;

    println!("Creating {workers} workers");
    let mut set = JoinSet::new();

    for id in 0..workers {
        let mut rng = rand::thread_rng();
        let message: Vec<u8> = (0..64).map(|_| rng.gen_range(0..255)).collect();

        set.spawn(worker(id + 1, message));
    }

    set.join_next().await;

    Ok(())
}
