use core::future::{poll_fn, Future};
use core::task::{Poll, Waker};

use smoltcp::iface::{Interface, NeighborCache, Routes, SocketStorage, InterfaceBuilder};
use smoltcp::wire::{EthernetAddress, IpAddress, Ipv4Address, IpCidr};

use super::devices::Enc28j60;
use crate::time::{sleep, Instant};


const MAC_ADDR: [u8; 6] = [0x2, 0x3, 0x4, 0x5, 0x6, 0x7];

pub struct Stack<'a> {
    interface: Interface<'a, Enc28j60>,
    waker: Option<Waker>,
}

impl<'a> Stack<'a> {
    pub fn new(device: Enc28j60) -> Stack<'a> {
        let mut cache = [None; 16];
        let neighbor_cache = NeighborCache::new(&mut cache[..]);

        let ethernet_addr = EthernetAddress(MAC_ADDR);
        let ip_addr = IpAddress::v4(192, 168, 69, 1);
        let mut ip_addrs = [IpCidr::new(ip_addr, 24)];

        let default_v4_gw = Ipv4Address::new(192, 168, 69, 100);
        let mut routes_storage = [None; 1];
        let mut routes = Routes::new(&mut routes_storage[..]);
        routes.add_default_ipv4_route(default_v4_gw).unwrap();

        let mut storage = [SocketStorage::EMPTY; 16];
        let mut interface = InterfaceBuilder::new(device, &mut storage[..])
            .ip_addrs(&mut ip_addrs[..])
            .hardware_addr(ethernet_addr.into())
            .neighbor_cache(neighbor_cache)
            .finalize();

        Stack {
            interface,
            waker: None,
        }
    }

    async fn start(&mut self) {
        poll_fn(|cx| {
            // Register waker. If there is a waker, drop it
            if let Some(waker) = self.waker.take() {
                drop(waker)
            };
            self.waker = Some(cx.waker().clone());

            let timestamp = Instant::now();
            match self.interface.poll(timestamp.into()) {
                Ok(_) => {}
                Err(e) => defmt::debug!("Interface poll error: {}", e),
            };

            // If we are ready to poll the interface again, poll for more packets. Otherwise sleep
            // until the next deadline
            if let Some(deadline) = self.interface.poll_delay(timestamp.into()) {
                let delay = sleep(deadline.into());
                crate::pin!(delay);
                if delay.poll(cx).is_ready() {
                    cx.waker().wake_by_ref()
                }
            }

            Poll::Pending
        })
        .await
    }
}
