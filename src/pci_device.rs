use std::{collections::HashMap, sync::OnceLock};

use regex::Regex;

use crate::{LnkCap, LnkSta, PciAddr};

#[derive(Debug)]
pub struct PciDevice {
    addr: PciAddr,
    vendor_id: u16,
    device_id: u16,
    // TODO: cache derived values
    desc: String,
}

impl PciDevice {
    pub fn parse(desc: &str) -> Option<PciDevice> {
        static PCI_DEVICE_RE: OnceLock<Regex> = OnceLock::new();

        PCI_DEVICE_RE
            .get_or_init(|| {
                Regex::new(concat!(
                    r"^(?:([0-9a-f]{4}):)?([0-9a-f]{2}):([0-9a-f]{2})\.([0-7]).*",
                    r"\[([0-9a-f]{4}):([0-9a-f]{4})\]"
                ))
                .unwrap()
            })
            .captures(desc)
            .map(|caps| {
                let domain = caps
                    .get(1)
                    .map_or(0, |m| u16::from_str_radix(m.as_str(), 16).unwrap());
                let bus = u8::from_str_radix(&caps[2], 16).unwrap();
                let device = u8::from_str_radix(&caps[3], 16).unwrap();
                let function = u8::from_str_radix(&caps[4], 16).unwrap();
                let vendor_id = u16::from_str_radix(&caps[5], 16).unwrap();
                let device_id = u16::from_str_radix(&caps[6], 16).unwrap();

                PciDevice {
                    addr: PciAddr::new(domain, bus, device, function),
                    vendor_id,
                    device_id,
                    desc: desc.to_string(),
                }
            })
    }

    pub fn addr(&self) -> PciAddr {
        self.addr
    }

    pub fn vendor_id(&self) -> u16 {
        self.vendor_id
    }

    pub fn device_id(&self) -> u16 {
        self.device_id
    }

    pub fn short_name(&self) -> Option<&'static str> {
        static SHORT_NAMES: [((u16, u16), &str); 46] = [
            ((0x1000, 0x005d), "MegaRAID 3108"),
            ((0x1000, 0x00b2), "switch mgmt"),
            ((0x1000, 0x02b2), "placeholder"),
            ((0x1000, 0xc010), "placeholder"),
            ((0x1022, 0x1485), "AMD SPP"),
            ((0x1022, 0x1486), "AMD PSPCPP"),
            ((0x1022, 0x1487), "AMD HD Audio"),
            ((0x1022, 0x148a), "dummy function"),
            ((0x1022, 0x148c), "AMD XHCI"),
            ((0x1022, 0x1498), "AMD PTDMA"),
            ((0x1022, 0x149c), "AMD XHCI"),
            ((0x1022, 0x7901), "AMD SATA"),
            ((0x102b, 0x0522), "Matrox VGA"),
            ((0x102b, 0x0534), "Matrox VGA"),
            ((0x102b, 0x0536), "Matrox VGA"),
            ((0x10de, 0x0e0f), "NVIDIA GK208 HDMP/DP Audio"),
            ((0x10de, 0x128b), "NVIDIA GT 710"),
            ((0x10de, 0x1af1), "A100 NVSwitch"),
            ((0x10de, 0x20b0), "A100 SXM4 40GB"),
            ((0x10de, 0x22a3), "H100 NVSwitch"),
            ((0x10de, 0x2330), "H100 SXM5 80GB"),
            ((0x10de, 0x2335), "H200 SXM5 141GB"),
            ((0x10de, 0x2901), "B200 SXM6 192GB"),
            ((0x10ec, 0x8125), "Realtek RTL8125 2.5GbE"),
            ((0x1344, 0x51c3), "Micron NVMe"),
            ((0x144d, 0xa808), "Samsung NVMe"),
            ((0x144d, 0xa80a), "Samsung NVMe"),
            ((0x144d, 0xa80c), "Samsung NVMe"),
            ((0x144d, 0xa824), "Samsung NVMe"),
            ((0x144d, 0xa825), "Samsung NVMe"),
            ((0x14e4, 0x165f), "Broadcom BCM5720"),
            ((0x15b3, 0x1019), "MT28800 ConnectX-5 Ex ETH"),
            ((0x15b3, 0x101b), "MT28908 ConnectX-6 IB"),
            ((0x15b3, 0x101d), "MT2892 ConnectX-6 Dx ETH"),
            ((0x15b3, 0x101e), "ConnectX-7 IB VF"),
            ((0x15b3, 0x1021), "MT2910 ConnectX-7 IB"),
            ((0x15b3, 0xa2dc), "MT43244 BlueField-3"),
            ((0x15b3, 0xc2d5), "MT43244 BlueField-3 mgmt"),
            ((0x1912, 0x0014), "Renesas USB3"),
            ((0x1a03, 0x2000), "ASPEED VGA"),
            ((0x1a03, 0x2402), "ASPEED IPMI"),
            ((0x1b4b, 0x2241), "Marvell NVMe"),
            ((0x1b4b, 0x9485), "Marvell SAS/SATA"),
            ((0x8086, 0x1563), "Intel X550"),
            ((0x8086, 0x15f3), "Intel I225-V"),
            ((0x8086, 0x2723), "Intel Wi-Fi 6 AX200"),
        ];

        static SHORT_NAMES_LOOKUP: OnceLock<HashMap<(u16, u16), &'static str>> = OnceLock::new();

        SHORT_NAMES_LOOKUP
            .get_or_init(|| HashMap::from(SHORT_NAMES))
            .get(&(self.vendor_id, self.device_id))
            .copied()
    }

    pub fn is_root_port(&self) -> bool {
        static PCIE_ROOT_PORT_RE: OnceLock<Regex> = OnceLock::new();

        PCIE_ROOT_PORT_RE
            .get_or_init(|| Regex::new(r" Express \(v2\) Root Port ").unwrap())
            .is_match(&self.desc)
    }

    pub fn numa_node(&self) -> Option<usize> {
        static NUMA_NODE_RE: OnceLock<Regex> = OnceLock::new();

        NUMA_NODE_RE
            .get_or_init(|| Regex::new(r"NUMA node: ([0-9]*)\n").unwrap())
            .captures(&self.desc)
            .map(|caps| usize::from_str_radix(&caps[1], 16).unwrap())
    }

    pub fn device_group_name(&self) -> String {
        match self.numa_node() {
            None => {
                if self.addr().bus() == 0 {
                    "PCH".to_string()
                } else {
                    "CPU".to_string()
                }
            }
            Some(numa_node) => {
                if self.addr().bus() == 0 {
                    format!("PCH (on NUMA node #{})", numa_node)
                } else {
                    format!("NUMA node #{}", numa_node)
                }
            }
        }
    }

    pub fn lnk_cap(&self) -> Option<LnkCap> {
        static LNK_CAP_RE: OnceLock<Regex> = OnceLock::new();

        LNK_CAP_RE
            .get_or_init(|| {
                Regex::new(concat!(
                    r"LnkCap:\tPort #[0-9]*, ",
                    r"Speed ([0-9.]*)GT/s, ",
                    r"Width x([0-9]*)"
                ))
                .unwrap()
            })
            .captures(&self.desc)
            .map(|caps| {
                LnkCap::new(
                    caps[1].parse::<f32>().unwrap(),
                    caps[2].parse::<u8>().unwrap(),
                )
            })
    }

    pub fn secondary_bus(&self) -> Option<u8> {
        static SECONDARY_BUS_RE: OnceLock<Regex> = OnceLock::new();

        SECONDARY_BUS_RE
            .get_or_init(|| Regex::new(r", secondary=([0-9a-f]{2}), subordinate=").unwrap())
            .captures(&self.desc)
            .map(|caps| u8::from_str_radix(&caps[1], 16).unwrap())
    }

    pub fn is_upstream_port(&self) -> bool {
        static PCIE_UPSTREAM_PORT_RE: OnceLock<Regex> = OnceLock::new();

        PCIE_UPSTREAM_PORT_RE
            .get_or_init(|| Regex::new(r" Express \(v2\) Upstream Port, ").unwrap())
            .is_match(&self.desc)
    }

    pub fn is_endpoint(&self) -> bool {
        static PCIE_ENDPOINT_RE: OnceLock<Regex> = OnceLock::new();

        PCIE_ENDPOINT_RE
            .get_or_init(|| Regex::new(r" Express \(v2\) (?:Legacy )?Endpoint, ").unwrap())
            .is_match(&self.desc)
    }

    pub fn is_pci_bridge(&self) -> bool {
        static PCIE_PCI_BRIDGE_RE: OnceLock<Regex> = OnceLock::new();

        PCIE_PCI_BRIDGE_RE
            .get_or_init(|| {
                Regex::new(r" Express \(v2\) PCI-Express to PCI/PCI-X Bridge, ").unwrap()
            })
            .is_match(&self.desc)
    }

    pub fn lnk_sta(&self) -> Option<LnkSta> {
        static LNK_STA_RE: OnceLock<Regex> = OnceLock::new();

        LNK_STA_RE
            .get_or_init(|| {
                Regex::new(concat!(
                    r"LnkSta:\t",
                    r"Speed ([0-9.]*)GT/s",
                    r"((?: \(ok\))?)",
                    r"((?: \(downgraded\))?)",
                    r", ",
                    r"Width x([0-9]*)",
                    r"((?: \(ok\))?)",
                    r"((?: \(downgraded\))?)"
                ))
                .unwrap()
            })
            .captures(&self.desc)
            .map(|caps| {
                LnkSta::new(
                    caps[1].parse::<f32>().unwrap(),
                    caps[4].parse::<u8>().unwrap(),
                    !caps[3].is_empty() || !caps[6].is_empty(),
                )
            })
    }

    pub fn serial_number(&self) -> Option<u64> {
        static DEVICE_SERIAL_NUMBER_RE: OnceLock<Regex> = OnceLock::new();

        DEVICE_SERIAL_NUMBER_RE
            .get_or_init(|| {
                Regex::new(concat!(
                    r"\] Device Serial Number ",
                    r"([0-9a-f]{2})-",
                    r"([0-9a-f]{2})-",
                    r"([0-9a-f]{2})-",
                    r"([0-9a-f]{2})-",
                    r"([0-9a-f]{2})-",
                    r"([0-9a-f]{2})-",
                    r"([0-9a-f]{2})-",
                    r"([0-9a-f]{2})\n",
                ))
                .unwrap()
            })
            .captures(&self.desc)
            .map(|caps| {
                u64::from_be_bytes([
                    u8::from_str_radix(&caps[1], 16).unwrap(),
                    u8::from_str_radix(&caps[2], 16).unwrap(),
                    u8::from_str_radix(&caps[3], 16).unwrap(),
                    u8::from_str_radix(&caps[4], 16).unwrap(),
                    u8::from_str_radix(&caps[5], 16).unwrap(),
                    u8::from_str_radix(&caps[6], 16).unwrap(),
                    u8::from_str_radix(&caps[7], 16).unwrap(),
                    u8::from_str_radix(&caps[8], 16).unwrap(),
                ])
            })
    }
}
