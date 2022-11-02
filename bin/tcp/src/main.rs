#![no_std]
#![no_main]
#![feature(type_alias_impl_trait, generic_associated_types)]

use defmt_rtt as _;
use panic_probe as _;
use stm32f3 as _;

use chrono::io::{AsyncRead, AsyncWrite};
use chrono::net::TcpListener;

#[chrono::alloc]
async fn netd() {
    chrono::net::stack().start().await
}

#[chrono::main]
async fn main() -> ! {
    // chrono::init();

    // Start networking daemon
    let _ = chrono::spawn(netd());

    let (mut tx_buffer, mut rx_buffer) = chrono::net::buffer::<4096>();
    let mut listener = TcpListener::new(&mut tx_buffer, &mut rx_buffer);
    if listener.bind(7777).is_err() {
        panic!("Failed to bind TCP listener")
    }

    // TODO: Error handling
    loop {
        let (mut stream, _) = listener
            .accept()
            .await
            .expect("Failed to accept connection");

        let mut buf = [0; 1024];
        match stream.read(&mut buf).await {
            Ok(n) => defmt::debug!("Read {} bytes", n),
            Err(e) => panic!("Read error: {}", e),
        }

        let output = core::str::from_utf8(&buf).unwrap();
        defmt::debug!("Message: {}", output);
    }
}
