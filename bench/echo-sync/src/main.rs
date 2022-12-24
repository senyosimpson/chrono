#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;
use heapless::Vec;
use panic_probe as _;
use stm32f3 as _;

use smoltcp::iface::{InterfaceBuilder, Neighbor, NeighborCache, Route, Routes, SocketStorage};
use smoltcp::socket::{AnySocket, TcpSocket, TcpSocketBuffer};
use smoltcp::wire::{EthernetAddress, IpAddress, IpCidr, Ipv4Address};

use stm32f3xx_hal::delay::Delay;
use stm32f3xx_hal::pac;
use stm32f3xx_hal::prelude::*;
use stm32f3xx_hal::spi::Spi;

use chrono::net::devices::Enc28j60;
use chrono::time::Instant;

const MAC_ADDR: [u8; 6] = [0x2, 0x3, 0x4, 0x5, 0x6, 0x7];
const KB: u16 = 1024; // bytes
const RX_BUF_SIZE: u16 = 7 * KB;

const NUM_SOCKETS: usize = 50;
static mut BUFFERS: [([u8; 64], [u8; 64]); NUM_SOCKETS] = [([0; 64], [0; 64]); NUM_SOCKETS];

#[cortex_m_rt::entry]
fn main() -> ! {
    defmt::debug!("Initialising system");

    let peripherals = unsafe { pac::Peripherals::steal() };

    // This is a workaround, so that the debugger will not disconnect immediately on asm::wfe();
    // https://github.com/probe-rs/probe-rs/issues/350#issuecomment-740550519
    peripherals.DBGMCU.cr.modify(|_, w| {
        w.dbg_sleep().set_bit();
        w.dbg_standby().set_bit();
        w.dbg_stop().set_bit()
    });

    let mut rcc = peripherals.RCC.constrain();
    let cfg = rcc.cfgr.hclk(1.MHz());
    let mut flash = peripherals.FLASH.constrain();
    let clocks = cfg.freeze(&mut flash.acr);

    let core_peripherals = unsafe { pac::CorePeripherals::steal() };
    // core_peripherals.DCB.enable_trace();
    // core_peripherals.DWT.enable_cycle_counter();

    let mut gpioa = peripherals.GPIOA.split(&mut rcc.ahb);

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
        reset,
        &mut delay,
        RX_BUF_SIZE,
        MAC_ADDR,
    ) {
        Ok(d) => d,
        Err(_) => panic!("Could not initialise driver"),
    };

    let device = Enc28j60::new(enc28j60);

    delay.delay_ms(100_u8);

    defmt::debug!("Initialised ethernet device");
    let mut neighbor_cache_storage: [Option<(IpAddress, Neighbor)>; 2] = [None; 2];
    let mut routes_storage: [Option<(IpCidr, Route)>; 1] = [None; 1];
    let mut sockets_storage = [SocketStorage::EMPTY; NUM_SOCKETS];
    let mut ip_addrs_storage: [IpCidr; 1] = [IpCidr::new(IpAddress::v4(192, 168, 69, 1), 24)];

    let neighbor_cache = NeighborCache::new(&mut neighbor_cache_storage[..]);

    let ethernet_addr = EthernetAddress(MAC_ADDR);

    let default_v4_gw = Ipv4Address::new(192, 168, 69, 100);
    let mut routes = Routes::new(&mut routes_storage[..]);
    routes
        .add_default_ipv4_route(default_v4_gw)
        .expect("Failed to add default ipv4 route");

    let mut interface = InterfaceBuilder::new(device, &mut sockets_storage[..])
        .ip_addrs(&mut ip_addrs_storage[..])
        .hardware_addr(ethernet_addr.into())
        .neighbor_cache(neighbor_cache)
        .finalize();

    defmt::debug!("Done!");

    unsafe {
        for (rx_buffer, tx_buffer) in BUFFERS.iter_mut() {
            let rx_buffer = TcpSocketBuffer::new(&mut rx_buffer[..]);
            let tx_buffer = TcpSocketBuffer::new(&mut tx_buffer[..]);
            let tcp_socket = TcpSocket::new(rx_buffer, tx_buffer);
            interface.add_socket(tcp_socket);
        }
    }

    loop {
        let timestamp = Instant::now();
        match interface.poll(timestamp.into()) {
            Ok(_) => {}
            Err(e) => {
                defmt::info!("poll error: {}", e);
            }
        }

        for (i, (_, socket)) in interface.sockets_mut().enumerate() {
            // let socket = interface.get_socket::<TcpSocket>(tcp_handle);
            // Note: safe to unwrap since we only have TCP sockets
            let socket = TcpSocket::downcast(socket).unwrap();
            if !socket.is_open() {
                socket.listen(7777).expect("Could not listen on port 7777");
                defmt::info!("Socket {}: Listening on port 7777", i + 1);
            }

            if socket.may_recv() {
                let (bytes, data) = match socket.recv(|buffer| {
                    let data: Vec<u8, 64> =
                        Vec::from_slice(buffer).expect("Could not create vector from slice");

                    if !data.is_empty() {
                        defmt::debug!("Recv data: {:?}", &data[..data.len()]);
                    }

                    (buffer.len(), (buffer.len(), data))
                }) {
                    Ok(res) => res,
                    Err(e) => {
                        defmt::debug!("Recv error: {}", e);
                        continue;
                    }
                };

                if socket.can_send() && !data.is_empty() {
                    defmt::debug!("Send data: {:?}", &data[..bytes]);
                    match socket.send_slice(&data[..bytes]) {
                        Ok(_) => {}
                        Err(e) => defmt::debug!("Send error: {}", e),
                    }
                }
            } else if socket.may_send() {
                defmt::debug!("tcp close");
                socket.close();
            }
        }
    }
}
