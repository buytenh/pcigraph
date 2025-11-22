# pcigraph

This tool turns `lspci` output into a graphviz graph representing a machine's PCI(e) switches and devices.

It was originally written to help debug performance problems on large GPU servers.

#### Usage

Make sure you have `graphviz` installed, and then run:

```bash
(dmidecode; lspci -nnvv) | cargo run > pci.dot
dot -Tpng pci.dot > pci.png
```

Including `dmidecode` output is optional.  If it is included, `pcigraph` will annotate the produced graph with any PCI slot names found in System Slot Information (DMI type 9) records in the `dmidecode` output.

#### Sample output

- [Dell PowerEdge XE9680](samples/dell-poweredge-xe9680.png)
- [ORACLE SERVER E4-2c](samples/oracle-server-e4-2c.png)
- [Supermicro SYS-821GE-TNHR](samples/supermicro-sys-821ge-tnhr.png)
