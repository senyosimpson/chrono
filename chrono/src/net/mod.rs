mod enc28j60;
pub mod devices {
    pub use super::enc28j60::Enc28j60;
}

mod stack;
pub use stack::{stack, Stack};

mod tcp;
pub use tcp::TcpSocket;

pub fn buffer<const N: usize>() -> ([u8; N], [u8; N]) {
    ([0; N], [0; N])
}

const MAC_ADDR: [u8; 6] = [0x2, 0x3, 0x4, 0x5, 0x6, 0x7];
