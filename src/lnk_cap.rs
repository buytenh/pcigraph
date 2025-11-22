use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub struct LnkCap {
    gt: f32,
    width: u8,
}

impl LnkCap {
    pub fn new(gt: f32, width: u8) -> LnkCap {
        LnkCap { gt, width }
    }
}

impl Display for LnkCap {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{}GT/s x{}", self.gt, self.width)
    }
}
