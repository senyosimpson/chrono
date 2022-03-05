use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use chrono::io::AsyncReadExt;
use chrono::net::TcpStream;
use chrono::Runtime;

fn main() {
    tracing_subscriber::fmt::init();

    let rt = Runtime::new();
    rt.block_on(async {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let mut stream = TcpStream::connect(addr).await.unwrap();

        let handle = chrono::spawn(async move {
            let mut buf = vec![0; 1024];
            let n = stream
                .read(&mut buf)
                .await
                .expect("failed to read data from socket");
            println!("Received message: {}", String::from_utf8(buf).unwrap());
            n
        });

        let n = handle.await.unwrap();
        println!("Read {} bytes", n)
    })
}
