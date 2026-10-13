# 10 — LAN Scan: discovery

**What to build:** The Technician sweeps an address range, defaulting to their own subnet, and watches Devices appear with IP, MAC and response time. On the Technician's own subnet ARP runs alongside ping, so Windows PCs that drop ping are still found and flagged "no ping reply". This ticket adds ARP resolution to the network adapter.

Spec: user stories 58–61, 62 (IP, MAC, response time), 65–67; Implementation Decisions › Network adapter, Per-Tool rules › LAN Scan.

**Blocked by:** 05 — Ping; 06 — IP Info.

**Status:** ready-for-agent
**Category:** enhancement

- [ ] The default range is the IPv4 network of the adapter carrying the default route; the range is editable as CIDR or start–end.
- [ ] Up to 1,024 addresses runs freely; 1,025 to 65,536 requires confirmation; anything larger is refused with a clear message. Network and broadcast addresses are skipped.
- [ ] 64 addresses are probed concurrently; a /24 completes in a few seconds on a typical LAN.
- [ ] On the Technician's own subnet, ARP and ICMP run together and a Device exists if either answers; elsewhere ICMP only.
- [ ] Each Device shows IP, MAC (own subnet only) and response time; a Device that answered ARP but not ping is flagged "no ping reply".
- [ ] Devices appear as they are found, with a progress indicator; the scan can be stopped and copied at any point.
- [ ] IPv6 ranges are rejected with an explanation that LAN Scan is IPv4 only.
- [ ] Core tests with the fake adapter cover range parsing, the size policy, own-subnet versus other-subnet probing, the "no ping reply" flag, stopping, and golden Copy as text.

## Comments

**2026-10-13 — note from ticket 06 (Claude).** Call `ip_info::find_default_route(network)` to get the Default Route Adapter. Caveat: if no adapter has an IPv4 route, the rule falls back to IPv6, and that adapter may have no IPv4 address at all. In that case LAN Scan must leave the range empty and ask for one rather than guess.
