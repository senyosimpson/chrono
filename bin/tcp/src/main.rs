#![no_std]
#![no_main]
#![feature(type_alias_impl_trait, generic_associated_types)]

use core::cell::UnsafeCell;

use defmt_rtt as _;
use panic_probe as _;
use smoltcp::socket::{TcpSocket, TcpSocketBuffer};
use stm32f3 as _;

use enc28j60;
use smoltcp::iface::{InterfaceBuilder, NeighborCache, Routes, SocketStorage};
use smoltcp::wire::{EthernetAddress, IpAddress, IpCidr, Ipv4Address};

use chrono::hal::delay::Delay;
use chrono::hal::pac::{CorePeripherals, Peripherals};
use chrono::hal::prelude::*;
use chrono::hal::spi::Spi;
use chrono::io::{AsyncRead, AsyncWrite};
use chrono::net::devices::Enc28j60;
use chrono::net::TcpStream;

/* Constants */
const KB: u16 = 1024; // bytes
const RX_BUF_SIZE: u16 = 7 * KB;
const MAC_ADDR: [u8; 6] = [0x2, 0x3, 0x4, 0x5, 0x6, 0x7];

#[chrono::main]
async fn main() -> ! {
    let peripherals = Peripherals::take().unwrap();
    let mut core_peripherals = CorePeripherals::take().unwrap();

    let mut rcc = peripherals.RCC.constrain();
    let cfg = rcc.cfgr.hclk(1.MHz());
    let mut flash = peripherals.FLASH.constrain();
    let clocks = cfg.freeze(&mut flash.acr);
    let mut gpioa = peripherals.GPIOA.split(&mut rcc.ahb);

    core_peripherals.DWT.enable_cycle_counter();

    // SPI
    let mut ncs = gpioa
        .pa4
        .into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper);
    if let Err(_) = ncs.set_high() {
        panic!("Failed to drive ncs pin high");
    }

    let sck = gpioa
        .pa5
        .into_af_push_pull(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrl);
    let mosi = gpioa
        .pa7
        .into_af_push_pull(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrl);
    let miso = gpioa
        .pa6
        .into_af_push_pull(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrl);

    let spi = Spi::new(
        peripherals.SPI1,
        (sck, miso, mosi),
        1.MHz(),
        clocks,
        &mut rcc.apb2,
    );

    // ENC28J60
    let mut reset = gpioa
        .pa3
        .into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper);
    if let Err(_) = reset.set_high() {
        panic!("Failed to drive reset pin high");
    }

    let mut delay = Delay::new(core_peripherals.SYST, clocks);
    // TODO: Init entire device inside chrono's enc28j60 device
    let enc28j60 = match enc28j60::Enc28j60::new(
        spi,
        ncs,
        enc28j60::Unconnected,
        reset,
        &mut delay,
        RX_BUF_SIZE,
        MAC_ADDR,
    ) {
        Ok(d) => d,
        Err(_) => panic!("Could not initialise driver"),
    };

    delay.delay_ms(100_u8);

    defmt::debug!("Initialised ethernet device");

    // ==================================
    // INIT DEVICE DONE
    // ==================================

    let device = Enc28j60::new(enc28j60);

    // Configure ethernet and devices
    let ethernet_addr = EthernetAddress(MAC_ADDR);
    let ip_addr = IpAddress::v4(192, 168, 69, 1);
    let mut ip_addrs = [IpCidr::new(ip_addr, 24)];
    let default_v4_gw = Ipv4Address::new(192, 168, 69, 100);
    let mut routes_storage = [None; 1];
    let mut routes = Routes::new(&mut routes_storage[..]);
    routes.add_default_ipv4_route(default_v4_gw).unwrap();

    let mut cache = [None; 4];
    let neighbor_cache = NeighborCache::new(&mut cache[..]);

    let mut storage = [SocketStorage::EMPTY; 2];
    let mut iface = InterfaceBuilder::new(device, &mut storage[..])
        .ip_addrs(&mut ip_addrs[..])
        .hardware_addr(ethernet_addr.into())
        .neighbor_cache(neighbor_cache)
        .finalize();

    let mut tx_buffer = [0; 2048];
    let mut rx_buffer = [0; 2048];
    let tcp_rx_buffer = TcpSocketBuffer::new(&mut rx_buffer[..]);
    let tcp_tx_buffer = TcpSocketBuffer::new(&mut tx_buffer[..]);
    let mut tcp_socket = TcpSocket::new(tcp_rx_buffer, tcp_tx_buffer);
    // tcp_socket.listen(IpEndpoint::new(IpAddress::v4(192, 168, 69, 1), 7777));
    tcp_socket.listen(7777).unwrap();

    let tcp_handle = iface.add_socket(tcp_socket);

    let iface = UnsafeCell::new(iface);
    let mut tcp_stream = TcpStream::new(&iface, tcp_handle);

    let mut buf = [0u8; 1024];
    loop {
        match tcp_stream.read(&mut buf).await {
            Ok(n) => defmt::debug!("Read {} bytes", n),
            Err(_) => defmt::debug!("Failed to read"),
        }
    }
}
