use core::marker::PhantomData;

use smoltcp::phy::{self, Device, DeviceCapabilities, Medium};

use crate::hal::gpio::{Alternate, Gpioa, Output, Pin, PushPull, U};
use crate::hal::pac::SPI1;
use crate::hal::spi;

// Concrete types for the Enc28j60 device connected to a stm32f3 discovery board
type Spi = spi::Spi<
    SPI1,
    (
        Pin<Gpioa, U<5>, Alternate<PushPull, 5>>,
        Pin<Gpioa, U<6>, Alternate<PushPull, 5>>,
        Pin<Gpioa, U<7>, Alternate<PushPull, 5>>,
    ),
>;
type Ncs = Pin<Gpioa, U<4>, Output<PushPull>>;
type Int = enc28j60::Unconnected;
type Reset = Pin<Gpioa, U<3>, Output<PushPull>>;

pub(crate) struct Enc28j60(enc28j60::Enc28j60<Spi, Ncs, Int, Reset>);

const MTU: usize = 1516;

pub struct RxToken {
    buffer: [u8; MTU],
    size: u16
}

pub struct TxToken<'a, T: Device<'a>> {
    device: &'a mut T,
    phantom: PhantomData<&'a T>,
}

impl<'a> Device<'a> for Enc28j60 {
    type RxToken = RxToken;

    type TxToken = TxToken<'a, Enc28j60>;

    fn capabilities(&self) -> DeviceCapabilities {
        let mut dc = DeviceCapabilities::default();
        dc.medium = Medium::Ethernet;
        dc.max_transmission_unit = MTU;
        dc.max_burst_size = Some(0);

        dc
    }

    fn receive(&'a mut self) -> Option<(Self::RxToken, Self::TxToken)> {
        let mut buffer = [0; MTU];
        match self.0.receive(&mut buffer) {
            Ok(size) => {
                let rx = RxToken { buffer, size };
                let tx = TxToken {
                    device: self,
                    phantom: PhantomData,
                };

                Some((rx, tx))
            }
            Err(_) => panic!("recv error!"),
        }
    }

    fn transmit(&'a mut self) -> Option<Self::TxToken> {
        let tx = TxToken {
            device: self,
            phantom: PhantomData,
        };

        Some(tx)
    }
}

// ===== impl RxToken

impl phy::RxToken for RxToken {
    fn consume<R, F>(mut self, _: smoltcp::time::Instant, f: F) -> smoltcp::Result<R>
    where
        F: FnOnce(&mut [u8]) -> smoltcp::Result<R>,
    {
        f(&mut self.buffer[..self.size as usize])
    }
}

// ===== impl TxToken

impl<'a> phy::TxToken for TxToken<'a, Enc28j60> {
    fn consume<R, F>(self, _: smoltcp::time::Instant, len: usize, f: F) -> smoltcp::Result<R>
    where
        F: FnOnce(&mut [u8]) -> smoltcp::Result<R>,
    {
        let mut buffer = [0; MTU];
        let packet = &mut buffer[0..len];
        let result = f(packet);
        self.device.0.transmit(packet).expect("Failed to transmit");
        result
    }
}
