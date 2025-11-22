use std::fmt::{Display, Formatter};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PciAddr {
    pub domain: u16,
    pub bus: u8,
    pub devfn: u8,
}

impl PciAddr {
    pub fn new(domain: u16, bus: u8, device: u8, function: u8) -> PciAddr {
        PciAddr {
            domain,
            bus,
            devfn: (device << 3) | function,
        }
    }

    pub fn domain(&self) -> u16 {
        self.domain
    }

    pub fn bus(&self) -> u8 {
        self.bus
    }

    pub fn device(&self) -> u8 {
        self.devfn >> 3
    }

    pub fn function(&self) -> u8 {
        self.devfn & 0x7
    }
}

impl Display for PciAddr {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "{:04x}:{:02x}:{:02x}.{:x}",
            self.domain(),
            self.bus(),
            self.device(),
            self.function()
        )
    }
}
