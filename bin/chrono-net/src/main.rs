#![no_std]
#![no_main]
#![feature(type_alias_impl_trait, generic_associated_types)]

use defmt_rtt as _;
use panic_probe as _;
use smoltcp::phy::{Device, Medium};
use smoltcp::time::Duration;
use stm32f3 as _;

use enc28j60;
use smoltcp::iface::{InterfaceBuilder, NeighborCache, Routes, SocketStorage};
use smoltcp::socket::{
    IcmpEndpoint, IcmpPacketMetadata, IcmpSocket, IcmpSocketBuffer, TcpSocket, TcpSocketBuffer,
};
use smoltcp::wire::{
    EthernetAddress, Icmpv4Packet, Icmpv4Repr, IpAddress, IpCidr, IpEndpoint, Ipv4Address,
};

use chrono::hal::delay::Delay;
use chrono::hal::pac::{CorePeripherals, Peripherals};
use chrono::hal::prelude::*;
use chrono::hal::spi::Spi;
use chrono::net::devices::Enc28j60;
use chrono::time::Instant;

/* Constants */
const KB: u16 = 1024; // bytes
const RX_BUF_SIZE: u16 = 7 * KB;
const MAC_ADDR: [u8; 6] = [0x2, 0x3, 0x4, 0x5, 0x6, 0x7];

#[cortex_m_rt::entry]
fn main() -> ! {
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
    let enc28j60 = match enc28j60::Enc28j60::new(
        spi,
        ncs,
        enc28j60::Unconnected,
        // enc28j60::Unconnected,
        reset,
        &mut delay,
        RX_BUF_SIZE,
        MAC_ADDR,
    ) {
        Ok(d) => d,
        Err(_) => panic!("Could not initialise driver"),
    };

    delay.delay_ms(100_u8);

    // ==================================
    // INIT DEVICE DONE
    // ==================================

    let device = Enc28j60::new(enc28j60);

    // let mut cache = [None; 4];
    // let neighbor_cache = NeighborCache::new(&mut cache[..]);

    // let mut rx_buffer = [0; 256];
    // let mut rx_metadata_buffer = [IcmpPacketMetadata::EMPTY];
    // let mut tx_buffer = [0; 256];
    // let mut tx_metadata_buffer = [IcmpPacketMetadata::EMPTY];
    // let tcp1_rx_buffer = IcmpSocketBuffer::new(&mut rx_metadata_buffer[..], &mut rx_buffer[..]);
    // let tcp1_tx_buffer = IcmpSocketBuffer::new(&mut tx_metadata_buffer[..], &mut tx_buffer[..]);
    // let icmp_socket = IcmpSocket::new(tcp1_rx_buffer, tcp1_tx_buffer);

    // let ethernet_addr = EthernetAddress(MAC_ADDR);
    // let mut ip_addrs = [
    //     IpCidr::new(IpAddress::v4(192, 168, 69, 1), 24),
    // ];

    // let checksum = device.capabilities().checksum;
    // let medium = device.capabilities().medium;
    // let mut storage = [SocketStorage::EMPTY; 2];
    // let mut builder = InterfaceBuilder::new(device, &mut storage[..]).ip_addrs(&mut ip_addrs[..]);
    // if medium == Medium::Ethernet {
    //     builder = builder
    //         .hardware_addr(ethernet_addr.into())
    //         .neighbor_cache(neighbor_cache);
    // }
    // let mut iface = builder.finalize();

    // let icmp_handle = iface.add_socket(icmp_socket);

    // defmt::debug!("Starting");
    // loop {
    //     let timestamp = Instant::now();
    //     match iface.poll(timestamp.into()) {
    //         Ok(_) => {}
    //         Err(e) => {
    //             defmt::debug!("poll error: {}", e);
    //         }
    //     }

    //     let socket = iface.get_socket::<IcmpSocket>(icmp_handle);
    //     if !socket.is_open() {
    //         defmt::debug!("Binding to icmp ident {}", 8);
    //         socket.bind(IcmpEndpoint::Ident(80)).unwrap();
    //     }

    //     if socket.can_recv() {
    //         let (payload, _) = socket.recv().unwrap();

    //         let icmp_packet = Icmpv4Packet::new_checked(&payload).unwrap();
    //         let icmp_repr = Icmpv4Repr::parse(&icmp_packet, &checksum).expect("Could not parse icmp packet");
    //         defmt::info!("ICMP REPR: {}", icmp_repr);
    //     }
    // }

    let mut cache = [None; 4];
    let neighbor_cache = NeighborCache::new(&mut cache[..]);

    let mut tx_buffer = [0; 2048];
    let mut rx_buffer = [0; 2048];
    let tcp_rx_buffer = TcpSocketBuffer::new(&mut rx_buffer[..]);
    let tcp_tx_buffer = TcpSocketBuffer::new(&mut tx_buffer[..]);
    let tcp_socket = TcpSocket::new(tcp_rx_buffer, tcp_tx_buffer);

    let ethernet_addr = EthernetAddress(MAC_ADDR);
    // IP Address: fdaa::1/64
    let ip_addr = IpAddress::v4(192, 168, 69, 1);
    let mut ip_addrs = [IpCidr::new(ip_addr, 24)];
    let default_v4_gw = Ipv4Address::new(192, 168, 69, 100);
    let mut routes_storage = [None; 1];
    let mut routes = Routes::new(&mut routes_storage[..]);
    routes.add_default_ipv4_route(default_v4_gw).unwrap();

    let mut storage = [SocketStorage::EMPTY; 2];
    let mut iface = InterfaceBuilder::new(device, &mut storage[..])
        .ip_addrs(&mut ip_addrs[..])
        .hardware_addr(ethernet_addr.into())
        .neighbor_cache(neighbor_cache)
        .finalize();

    let tcp_handle = iface.add_socket(tcp_socket);
    let (socket, ctx) = iface.get_socket_and_context::<TcpSocket>(tcp_handle);
    socket
        .connect(ctx, (IpAddress::v4(192, 168, 69, 100), 7777), 49500)
        .unwrap();

    let mut tcp_active = false;
    loop {
        match iface.poll(Instant::now().into()) {
            Ok(_) => {}
            Err(e) => {
                defmt::debug!("poll error: {}", e);
            }
        }

        let socket = iface.get_socket::<TcpSocket>(tcp_handle);
        if socket.is_active() && !tcp_active {
            defmt::debug!("connected");
        } else if !socket.is_active() && tcp_active {
            panic!("disconnected");
        }
        tcp_active = socket.is_active();
        // if !socket.is_open() {
        //     defmt::info!("Binding socket");
        //     if let Err(_) = socket.listen(IpEndpoint::new(IpAddress::v4(192, 168, 69, 1), 7777)) {
        //         panic!("Could not bind socket");
        //     }
        // }
        let msg = "hello";
        if socket.can_send() {
            socket.send_slice(msg.as_bytes()).unwrap();
            defmt::debug!("SENT ALL THE DATA")
        }

        // if socket.can_recv() {
        //     defmt::info!("Can receive!");
        //     if let Err(_) = socket.recv(|buffer| {
        //         let recvd_len = buffer.len();
        //         if !buffer.is_empty() {
        //             defmt::info!("tcp:7777 recv data: {:?}", buffer.as_ref());
        //         }
        //         (recvd_len, buffer)
        //     }) {
        //         panic!("Failed to receive from socket")
        //     }
        // }
    }
}
