#![no_std]
#![no_main]
#![feature(type_alias_impl_trait, generic_associated_types)]

use defmt_rtt as _;
use panic_probe as _;
use stm32f3 as _;

use chrono::io::{AsyncRead, AsyncWrite};
use chrono::net::TcpSocket;

#[chrono::alloc]
async fn netd() {
    chrono::net::stack().start().await
}

#[chrono::alloc]
async fn conn1() {
    loop {
        let (mut tx_buffer, mut rx_buffer) = chrono::net::buffer::<1024>();
        let mut socket = TcpSocket::new(&mut tx_buffer, &mut rx_buffer);

        socket
            .listen(7777)
            .expect("Failed to listen");

        socket
            .accept()
            .await
            .expect("Failed to accept connection");

        loop {
            let mut buf = [0; 1024];
            match socket.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => defmt::debug!("Read {} bytes", n),
                Err(e) => panic!("Read error: {}", e),
            }

            let output = core::str::from_utf8(&buf).unwrap();
            defmt::debug!("Message: {}", output);
        }
    }
}

#[chrono::alloc]
async fn conn2() {
    loop {
        let (mut tx_buffer, mut rx_buffer) = chrono::net::buffer::<1024>();
        let mut socket = TcpSocket::new(&mut tx_buffer, &mut rx_buffer);

        socket
            .listen(7777)
            .expect("Failed to listen");

        socket
            .accept()
            .await
            .expect("Failed to accept connection");

        loop {
            let mut buf = [0; 1024];
            match socket.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => defmt::debug!("Read {} bytes", n),
                Err(e) => panic!("Read error: {}", e),
            }

            let output = core::str::from_utf8(&buf).unwrap();
            defmt::debug!("Message: {}", output);
        }
    }
}

#[chrono::alloc]
async fn conn3() {
    loop {
        let (mut tx_buffer, mut rx_buffer) = chrono::net::buffer::<1024>();
        let mut socket = TcpSocket::new(&mut tx_buffer, &mut rx_buffer);

        socket
            .listen(7777)
            .expect("Failed to listen");

        socket
            .accept()
            .await
            .expect("Failed to accept connection");

        loop {
            let mut buf = [0; 1024];
            match socket.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => defmt::debug!("Read {} bytes", n),
                Err(e) => panic!("Read error: {}", e),
            }

            let output = core::str::from_utf8(&buf).unwrap();
            defmt::debug!("Message: {}", output);
        }
    }

}

#[chrono::main]
async fn main() -> ! {
    let _ = chrono::spawn(netd()).expect("Could not spawn net daemon");
    let _ = chrono::spawn(conn1()).expect("Could not spawn conn1");
    let _ = chrono::spawn(conn2()).expect("Could not spawn conn2");
    chrono::spawn(conn3()).expect("Could not spawn conn3").await;
}
