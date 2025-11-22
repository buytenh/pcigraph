use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub struct LnkSta {
    gt: f32,
    width: u8,
    downgraded: bool,
}

impl LnkSta {
    pub fn new(gt: f32, width: u8, downgraded: bool) -> LnkSta {
        LnkSta {
            gt,
            width,
            downgraded,
        }
    }
}

impl Display for LnkSta {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "{}GT/s x{}{}",
            self.gt,
            self.width,
            if self.downgraded {
                "\\n(downgraded)"
            } else {
                ""
            }
        )
    }
}
