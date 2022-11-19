use core::cell::RefCell;
use core::future::poll_fn;
use core::mem::MaybeUninit;
use core::task::{Context, Poll};

use smoltcp::iface::{
    Interface, InterfaceBuilder, Neighbor, NeighborCache, Route, Routes, SocketStorage,
};
use smoltcp::wire::{EthernetAddress, IpAddress, IpCidr, Ipv4Address};

use super::devices::Enc28j60;
use super::MAC_ADDR;
use crate::time::Instant;

static mut STORAGE: MaybeUninit<Storage> = MaybeUninit::uninit();

pub static mut STACK: Stack = Stack::new();

struct Storage {
    neighbor_cache: [Option<(IpAddress, Neighbor)>; 16],
    routes: [Option<(IpCidr, Route)>; 1],
    sockets: [SocketStorage<'static>; 16],
    ip_addrs: [IpCidr; 1],
}

pub struct Stack {
    pub(crate) inner: Option<RefCell<Inner>>,
    initialised: bool,
}

pub struct Inner {
    pub interface: Interface<'static, Enc28j60>,
}

pub fn stack() -> &'static mut Stack {
    unsafe { &mut STACK }
}

unsafe impl Sync for Stack {}

impl Stack {
    pub const fn new() -> Stack {
        Stack {
            inner: None,
            initialised: false,
        }
    }

    pub fn init(&mut self, device: Enc28j60) {
        let storage = {
            let s = Storage {
                neighbor_cache: [None; 16],
                routes: [None; 1],
                sockets: [SocketStorage::EMPTY; 16],
                ip_addrs: [IpCidr::new(IpAddress::v4(192, 168, 69, 1), 24)],
            };
            unsafe { STORAGE.write(s) }
        };

        let neighbor_cache = NeighborCache::new(&mut storage.neighbor_cache[..]);

        let ethernet_addr = EthernetAddress(MAC_ADDR);

        let default_v4_gw = Ipv4Address::new(192, 168, 69, 100);
        let mut routes = Routes::new(&mut storage.routes[..]);
        routes.add_default_ipv4_route(default_v4_gw).unwrap();

        let interface = InterfaceBuilder::new(device, &mut storage.sockets[..])
            .ip_addrs(&mut storage.ip_addrs[..])
            .hardware_addr(ethernet_addr.into())
            .neighbor_cache(neighbor_cache)
            .finalize();

        let inner = Inner { interface };

        self.inner = Some(RefCell::new(inner));
        self.initialised = true;
    }

    pub async fn start(&mut self) {
        assert!(
            self.initialised,
            "initialise net stack before usage via call to .init()"
        );

        defmt::debug!("Starting net stack");
        poll_fn(|cx| self.poll_start(cx)).await
    }

    pub fn poll_start(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        let mut inner = self.inner.as_ref().unwrap().borrow_mut();

        let timestamp = Instant::now();
        match inner.interface.poll(timestamp.into()) {
            Ok(_) => {}
            Err(e) => defmt::warn!("Interface poll error: {}", e),
        };

        cx.waker().wake_by_ref();
        Poll::Pending
    }
}
