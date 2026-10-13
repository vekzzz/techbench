# 11 — LAN Scan: hostnames, vendors, sort and filter

**What to build:** LAN Scan Devices gain a hostname (from reverse lookup) and a vendor (from a bundled copy of the IEEE OUI registry, so it works offline), and the Technician can sort and filter the Device list to find one Device in a busy subnet.

Spec: user stories 62 (hostname, vendor), 63, 64, 68; Implementation Decisions › Per-Tool rules › LAN Scan.

**Blocked by:** 08 — Traceroute (reverse lookup); 10 — LAN Scan: discovery.

**Status:** ready-for-agent
**Category:** enhancement

- [ ] Each Device's hostname comes from reverse lookup and arrives after the Device first appears, without slowing the scan.
- [ ] Each Device's vendor comes from the bundled OUI registry matched on the MAC prefix, with no network access.
- [ ] Devices on other subnets show a blank MAC and vendor with an explanation that this is an ARP limitation.
- [ ] Devices can be sorted by IP (numerically), hostname or vendor, and filtered by text.
- [ ] Hostname and vendor are included in Copy as text.
- [ ] The installer from ticket 02 includes the OUI data.
- [ ] Core tests cover vendor lookup for known and unknown MAC prefixes, late hostname events, and golden Copy as text.
