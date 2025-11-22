mod lnk_cap;
mod lnk_sta;
mod pci_addr;
mod pci_device;

use std::{
    collections::BTreeMap,
    io::{Error, Read, Write, stdin, stdout},
    sync::OnceLock,
};

use lnk_cap::LnkCap;
use lnk_sta::LnkSta;
use pci_addr::PciAddr;
use pci_device::PciDevice;
use regex::Regex;

#[derive(Debug, Default)]
struct Machine {
    dmi_slots: BTreeMap<PciAddr, String>,
    pci_devices: BTreeMap<PciAddr, PciDevice>,
}

impl Machine {
    fn parse<T: Read>(&mut self, src: &mut T) {
        let mut data = String::new();

        src.read_to_string(&mut data).unwrap();

        let sections = data
            .split("\n\n")
            .filter(|str| !str.is_empty())
            .collect::<Vec<&str>>();

        static DMI_SLOT_RE: OnceLock<Regex> = OnceLock::new();

        let dmi_slot_re = DMI_SLOT_RE.get_or_init(|| {
            Regex::new(concat!(
                r"(?s), DMI type 9, .*",
                r"Designation: ([^\n]*)\n.*",
                r"Bus Address: ([0-9a-f]{4}):([0-9a-f]{2}):([0-9a-f]{2})\.([0-7])"
            ))
            .unwrap()
        });

        for section in sections {
            if let Some(caps) = dmi_slot_re.captures(section) {
                let name = &caps[1];
                let domain = u16::from_str_radix(&caps[2], 16).unwrap();
                let bus = u8::from_str_radix(&caps[3], 16).unwrap();
                let device = u8::from_str_radix(&caps[4], 16).unwrap();
                let function = u8::from_str_radix(&caps[5], 16).unwrap();

                self.dmi_slots.insert(
                    PciAddr::new(domain, bus, device, function),
                    name.to_string(),
                );
            }

            if let Some(pci_device) = PciDevice::parse(section) {
                self.pci_devices.insert(pci_device.addr(), pci_device);
            }
        }
    }

    fn bus_devices(&self, domain: u16, bus: u8) -> Vec<PciAddr> {
        self.pci_devices
            .keys()
            .filter(|dev_addr| dev_addr.domain() == domain && dev_addr.bus() == bus)
            .copied()
            .collect::<Vec<_>>()
    }
}

struct MachineWriteState {
    cluster_id: u16,
    clusters: BTreeMap<String, u16>,
}

impl MachineWriteState {
    fn new() -> MachineWriteState {
        MachineWriteState {
            cluster_id: 0,
            clusters: BTreeMap::new(),
        }
    }

    fn get_cluster_index(&mut self, identifier: &str) -> u16 {
        *self
            .clusters
            .entry(identifier.to_string())
            .or_insert_with(|| {
                self.cluster_id += 1;
                self.cluster_id
            })
    }
}

impl Machine {
    fn write_graph<T: Write>(&self, w: &mut T) -> Result<(), Error> {
        let mut write_state = MachineWriteState::new();

        writeln!(w, "graph pci {{")?;
        writeln!(w, "\trankdir=LR;")?;

        for (addr, dev) in &self.pci_devices {
            if dev.is_root_port() {
                //
                // Dell PowerEdge R730xd PCI device 00:00.0 (Host Bridge) claims to be a
                // PCI Express (v2) Root Port, but has a type 0 configuration space header.
                // Ignore Root Ports that don't have a type 1 configuration space header.
                //
                if let Some(secondary_bus) = dev.secondary_bus() {
                    writeln!(w)?;
                    writeln!(
                        w,
                        "\t######################################################################"
                    )?;
                    writeln!(w, "\t# root port {}", addr)?;

                    writeln!(w)?;
                    writeln!(
                        w,
                        "\t\"{}\" [ label=\"Root port\\n{}\" shape=rectangle ];",
                        addr, addr
                    )?;

                    let device_group_name = dev.device_group_name();
                    let cluster_id = write_state.get_cluster_index(&device_group_name);

                    writeln!(w)?;
                    writeln!(w, "\tsubgraph cluster{} {{", cluster_id)?;
                    writeln!(w, "\t\tlabel=\"{}\";", device_group_name)?;
                    writeln!(w, "\t\t\"{}\";", addr)?;
                    writeln!(w, "\t}}")?;

                    self.write_bus(w, &mut write_state, dev, addr.domain(), secondary_bus)?;
                }
            }
        }

        writeln!(w, "}}")?;

        Ok(())
    }

    fn write_bus<T: Write>(
        &self,
        w: &mut T,
        write_state: &mut MachineWriteState,
        parent_dev: &PciDevice,
        domain: u16,
        bus: u8,
    ) -> Result<(), Error> {
        writeln!(w)?;
        writeln!(w, "\t# domain {:04x} bus {:02x}", domain, bus)?;

        let bus_devices = self.bus_devices(domain, bus);

        //
        // In the ORACLE SERVER E4-2c, a DMI System Slot handle refers to the PCI bus
        // address of the Root Complex's Root Port or the upstream bridge's Downstream
        // Port, and not to the PCI bus address of the downstream bridge's Upstream
        // Port or the downstream Endpoint.  For this reason, we re-query for the
        // parent's PCI bus address if we don't find a System Slot handle for the
        // downstream address.
        //
        let slot_name = self
            .dmi_slots
            .get(&PciAddr::new(domain, bus, 0, 0))
            .or_else(|| self.dmi_slots.get(&parent_dev.addr()));

        writeln!(w)?;

        let intermediate = if slot_name.is_some() {
            format!("{}_{:02x}", parent_dev.addr(), bus)
        } else {
            format!("{}", parent_dev.addr())
        };

        if let Some(slot_name) = slot_name {
            let parent_lnk_cap = parent_dev.lnk_cap().unwrap();

            writeln!(
                w,
                "\t\"{}\" -- \"{}\" [ label=\"{}\" ];",
                parent_dev.addr(),
                intermediate,
                parent_lnk_cap,
            )?;

            writeln!(
                w,
                "\t\"{}\" [ label=\"{}\" shape=rectangle ];",
                intermediate, slot_name
            )?;
        }

        if let Some(first_dev_addr) = bus_devices.first() {
            let first_dev = self.pci_devices.get(first_dev_addr).unwrap();

            let label =
                if self.pci_device_unique_id(parent_dev) != self.pci_device_unique_id(first_dev) {
                    first_dev
                        .lnk_sta()
                        .map(|lnk_sta| format!(" [ label=\"{}\" ]", lnk_sta))
                        .unwrap()
                } else {
                    "".to_string()
                };

            // TODO: lhead into the cluster in case of multi-function device
            writeln!(
                w,
                "\t\"{}\" -- \"{}\"{};",
                intermediate, first_dev_addr, label
            )?;
        } else {
            let parent_lnk_cap = parent_dev.lnk_cap().unwrap();

            writeln!(
                w,
                "\t\"{}\" -- \"bus {:04x}:{:02x}\"{};",
                intermediate,
                domain,
                bus,
                if slot_name.is_none() {
                    format!(" [ label=\"{}\" ]", parent_lnk_cap)
                } else {
                    "".to_string()
                }
            )?;

            writeln!(w)?;

            writeln!(
                w,
                "\t\"bus {:04x}:{:02x}\" [ shape=rectangle ];",
                domain, bus
            )?;
        }

        let upstream_ports = bus_devices
            .iter()
            .filter(|dev_addr| self.pci_devices.get(dev_addr).unwrap().is_upstream_port())
            .copied()
            .collect::<Vec<_>>();

        let pci_bridges = bus_devices
            .iter()
            .filter(|dev_addr| self.pci_devices.get(dev_addr).unwrap().is_pci_bridge())
            .copied()
            .collect::<Vec<_>>();

        let endpoints = bus_devices
            .iter()
            .filter(|dev_addr| self.pci_devices.get(dev_addr).unwrap().is_endpoint())
            .copied()
            .collect::<Vec<_>>();

        if !upstream_ports.is_empty() {
            for dev_addr in upstream_ports {
                let dev = self.pci_devices.get(&dev_addr).unwrap();

                let downstream_port_bus = dev.secondary_bus().unwrap();

                let downstream_ports = self.bus_devices(dev_addr.domain(), downstream_port_bus);

                let unique_id = self.pci_device_unique_id(dev);

                writeln!(w)?;

                writeln!(
                    w,
                    "\tsubgraph cluster{} {{",
                    write_state.get_cluster_index(&unique_id)
                )?;

                writeln!(w, "\t\tlabel=\"PCIe switch\";")?;

                writeln!(w, "\t\t\"{}\";", dev_addr)?;

                for downstream_port_addr in &downstream_ports {
                    writeln!(w, "\t\t\"{}\";", downstream_port_addr)?;
                }

                writeln!(w, "\t}}")?;

                writeln!(w)?;

                writeln!(
                    w,
                    "\t# domain {:04x} bus {:02x} is a switch internal bus",
                    domain, downstream_port_bus
                )?;

                for downstream_port_addr in &downstream_ports {
                    writeln!(w)?;
                    writeln!(w, "\t\"{}\" -- \"{}\";", dev_addr, downstream_port_addr)?;
                }

                for downstream_port_addr in &downstream_ports {
                    let downstream_port = self.pci_devices.get(downstream_port_addr).unwrap();

                    let secondary_bus = downstream_port.secondary_bus().unwrap();

                    self.write_bus(
                        w,
                        write_state,
                        downstream_port,
                        downstream_port_addr.domain(),
                        secondary_bus,
                    )?;
                }
            }
        } else if !pci_bridges.is_empty() {
            for dev_addr in pci_bridges {
                let dev = self.pci_devices.get(&dev_addr).unwrap();

                let unique_id = self.pci_device_unique_id(dev);

                writeln!(w)?;

                writeln!(
                    w,
                    "\tsubgraph cluster{} {{",
                    write_state.get_cluster_index(&unique_id)
                )?;

                writeln!(w, "\t\tlabel=\"PCI bridge\";")?;

                writeln!(w, "\t\t\"{}\";", dev_addr)?;

                writeln!(w, "\t}}")?;

                let secondary_bus = dev.secondary_bus().unwrap();

                writeln!(w)?;

                writeln!(w, "\t# domain {:04x} bus {:02x}", domain, secondary_bus)?;

                let secondary_devices = self.bus_devices(dev_addr.domain(), secondary_bus);

                for secondary_device in &secondary_devices {
                    writeln!(w)?;

                    writeln!(w, "\t\"{}\" -- \"{}\";", dev_addr, secondary_device)?;

                    writeln!(w)?;

                    let dev = self.pci_devices.get(secondary_device).unwrap();

                    writeln!(
                        w,
                        "\t\"{}\" [ label=\"{}\\n{}\" ];",
                        secondary_device,
                        dev.short_name().unwrap_or(&format!(
                            "unknown {:04x}:{:04x}",
                            dev.vendor_id(),
                            dev.device_id()
                        )),
                        secondary_device
                    )?;
                }
            }
        } else if let Some(first_dev_addr) = endpoints.first() {
            let first_dev = self.pci_devices.get(first_dev_addr).unwrap();

            writeln!(w)?;

            writeln!(
                w,
                "\t\"{}\" [ label=\"{}\\n{}\" ];",
                first_dev_addr,
                first_dev.short_name().unwrap_or(&format!(
                    "unknown {:04x}:{:04x}",
                    first_dev.vendor_id(),
                    first_dev.device_id()
                )),
                first_dev_addr
            )?;

            if endpoints.len() > 1 {
                let unique_id = self.pci_device_unique_id(first_dev);

                writeln!(w)?;

                writeln!(
                    w,
                    "\tsubgraph cluster{} {{",
                    write_state.get_cluster_index(&unique_id)
                )?;

                for dev_addr in &endpoints {
                    writeln!(w, "\t\t\"{}\";", dev_addr)?;
                }

                writeln!(w, "\t}}")?;

                for a_b in endpoints.windows(2) {
                    writeln!(w)?;

                    writeln!(w, "\t\"{}\" -- \"{}\";", a_b[0], a_b[1])?;

                    let dev = self.pci_devices.get(&a_b[1]).unwrap();

                    writeln!(
                        w,
                        "\t\"{}\" [ label=\"{}\\n{}\" ];",
                        a_b[1],
                        dev.short_name().unwrap_or(&format!(
                            "unknown {:04x}:{:04x}",
                            dev.vendor_id(),
                            dev.device_id()
                        )),
                        a_b[1]
                    )?;
                }
            }
        }

        Ok(())
    }

    fn pci_device_unique_id(&self, dev: &PciDevice) -> String {
        if let Some(serial_number) = dev.serial_number() {
            if dev.is_upstream_port() {
                let downstream_port_bus = dev.secondary_bus().unwrap();

                let downstream_ports = self.bus_devices(dev.addr().domain(), downstream_port_bus);

                for downstream_port_addr in &downstream_ports {
                    let downstream_port = self.pci_devices.get(downstream_port_addr).unwrap();

                    if let Some(downstream_serial_number) = downstream_port.serial_number() {
                        //
                        // If a downstream port for this upstream port has a different
                        // Device Serial Number than the upstream port does, then don't
                        // trust the Device Serial Number for the upstream port.
                        //
                        if serial_number != downstream_serial_number {
                            return format!("{}", dev.addr());
                        }
                    }
                }
            }

            return format!("{:016x}", serial_number);
        }

        format!("{}", dev.addr())
    }
}

fn main() {
    let mut machine = Machine::default();

    machine.parse(&mut stdin());

    machine.write_graph(&mut stdout()).unwrap();
}
