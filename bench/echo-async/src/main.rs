#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;
use panic_probe as _;
use stm32f3 as _;

use chrono::io::{AsyncRead, AsyncWrite};
use chrono::net::TcpSocket;

#[chrono::alloc]
async fn netd() {
    chrono::net::stack().start().await
}

#[chrono::alloc(size = 32)]
async fn handle_tcp_conn() {
    loop {
        let (mut tx_buffer, mut rx_buffer) = chrono::net::buffer::<64>();
        let mut socket = TcpSocket::new(&mut tx_buffer, &mut rx_buffer);

        socket.listen(7777).expect("Failed to listen");
        socket.accept().await.expect("Failed to accept connection");

        loop {
            let mut buf = [0; 64];
            let bytes = match socket.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    defmt::debug!("Read {} bytes", n);
                    n
                }
                Err(e) => panic!("Read error: {}", e),
            };

            let output = core::str::from_utf8(&buf).unwrap();
            defmt::debug!("Message: {}", output);

            match socket.write(&buf[..bytes]).await {
                Ok(n) => defmt::debug!("Wrote {} bytes", n),
                Err(e) => panic!("Write error: {}", e),
            }
        }
    }
}

#[chrono::main]
async fn main() -> ! {
    let stack = chrono::spawn(netd()).expect("Could not spawn net daemon");
    for _ in 0..32 {
        chrono::spawn(handle_tcp_conn()).unwrap();
    }

    stack.await;
}
