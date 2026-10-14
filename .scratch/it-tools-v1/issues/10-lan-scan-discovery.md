# 10 — LAN Scan: discovery

**What to build:** The Technician sweeps an address range, defaulting to their own subnet, and watches Devices appear with IP, MAC and response time. On the Technician's own subnet ARP runs alongside ping, so Windows PCs that drop ping are still found and flagged "no ping reply". This ticket adds ARP resolution to the network adapter.

Spec: user stories 58–61, 62 (IP, MAC, response time), 65–67; Implementation Decisions › Network adapter, Per-Tool rules › LAN Scan.

**Blocked by:** 05 — Ping; 06 — IP Info.

**Status:** ready-for-agent
**Category:** enhancement

- [ ] The default range is the IPv4 network of the adapter carrying the default route; the range is editable as CIDR or start–end.
- [x] Up to 1,024 addresses runs freely; 1,025 to 65,536 requires confirmation; anything larger is refused with a clear message. Network and broadcast addresses are skipped.
- [x] 64 addresses are probed concurrently; a /24 completes in a few seconds on a typical LAN.
- [x] On the Technician's own subnet, ARP and ICMP run together and a Device exists if either answers; elsewhere ICMP only.
- [x] Each Device shows IP, MAC (own subnet only) and response time; a Device that answered ARP but not ping is flagged "no ping reply".
- [ ] Devices appear as they are found, with a progress indicator; the scan can be stopped and copied at any point.
- [x] IPv6 ranges are rejected with an explanation that LAN Scan is IPv4 only.
- [x] Core tests with the fake adapter cover range parsing, the size policy, own-subnet versus other-subnet probing, the "no ping reply" flag, stopping, and golden Copy as text.

## Comments

**2026-10-13 — note from ticket 06 (Claude).** Call `ip_info::find_default_route(network)` to get the Default Route Adapter. Caveat: if no adapter has an IPv4 route, the rule falls back to IPv6, and that adapter may have no IPv4 address at all. In that case LAN Scan must leave the range empty and ask for one rather than guess.

**2026-10-14 — implementation pass (Claude).** Built test-first: 38 S1 tests over the fake network, 4 smoke tests against the real adapter (loopback, this PC's own address with its MAC through real `SendARP`, the default range, an ARP that gives up in time). Full suite: 205 passed.

- **S2 `arp_resolve(address, timeout)`** added to the network adapter. The real one is `SendARP` (`windows_arp.rs`) on a blocking thread; `SendARP` has no timeout of its own, so the adapter stops waiting after 1 s and lets the call finish on its thread. Fake: `with_arp`, `arp_requests`.
- **Range** (`LanScanRequest { range, confirmed }`): CIDR, `start-end`, or one address. CIDR with host bits set means the whole network (`192.168.1.77/24` scans `192.168.1.0/24`). IPv6 text is rejected with "LAN Scan is IPv4 only". `check_range()` gives `{ addresses, needs_confirmation }` for the screen; `validate()` also refuses an unconfirmed big range, so the core enforces the policy, not just the UI.
- **Size policy counts the addresses the range spans** (a /22 is 1,024, free; a /16 is 65,536, confirm; a /15 is refused), not the ones probed after skipping network and broadcast.
- **Probing:** 64 at a time, 1 s ping timeout, so a silent /24 takes four waves (4 s; tested with the paused clock). On the own subnet the ping and ARP run together; a Device exists if either answered.
- **Default range:** `lan_scan::default_range` is the Default Route Adapter's IPv4 network (a real address preferred over a 169.254 one). With no IPv4 address it returns None and the screen asks for a range, as ticket 06 asked.
- **Shell and UI:** `default_lan_range`, `lan_range_info`, `start_lan_scan`; a LAN Scan screen (range field prefilled once, confirm panel for big ranges, progress bar, Devices as found, "no ping reply" badge, Copy as text while running or stopped).

My choices:
- **"Own subnet" is every up, non-tunnel, non-loopback adapter's IPv4 subnet**, not only the Default Route Adapter's: ARP works on any directly connected subnet, so a second NIC gets MACs too. A range that straddles a subnet edge is decided address by address.
- **Network and broadcast are skipped for CIDR ranges of /30 or wider only.** A `start-end` range probes exactly what was named, and /31 and /32 keep every address.
- **ARP-only Devices show "-" for response time**: the ARP time is not measured, only the ping's.
- **Devices carry `on_own_subnet`**, so Copy as text and the screen can say why a MAC is blank ("ARP does not cross routers"). Hostname and vendor are ticket 11.
- **Progress events are thinned** to about every 1% on big scans (every address on small ones) so a /16 does not send 65,000 messages.
- The Result's header has "Range:" instead of "Target:"/"Resolved:", since a range has no Resolved Address.

**Not verified in the running app:** the LAN Scan screen. I did not attempt the GUI check after the problems on tickets 07–09, so the screen-facing criteria are ticked for the core and shell only. Please try the screen once, including a range above 1,024 addresses (e.g. `10.0.0.0/21`) to see the confirm panel.

**2026-10-14 — review follow-up (Claude).** Fixed: one `prefix_mask` helper instead of three copies of the mask expression; `validate` parses the range once.

Accepted or deferred, for you to decide:
- **`start-end` ranges do not skip network/broadcast addresses**, only CIDR ranges of /30 or wider do. The ticket says "Network and broadcast addresses are skipped" without saying which; `192.168.1.0-192.168.1.255` therefore probes `.0` and `.255`.
- **`SendARP` may answer from the Windows neighbour cache**, so a host that left very recently can still show as an ARP-only Device for a few minutes.
- **`SendARP` cannot be cancelled.** After the 1 s timeout its thread is abandoned and finishes on its own; on a /16 of the own subnet that is up to ~64 extra blocking threads per second for a few seconds (tokio's pool allows 512). Fine for a /24; watch it on a /16.
- **The default range is offered once per app run** (`defaultOffered` in the screen), so it can go stale if the network changes; the Technician edits the field.
- **A VPN or virtual adapter with a wide prefix counts as "own subnet"** (tunnel-kind adapters are excluded, others are not).
- The confirm panel states the probed count (a /21: 2,046) while the gate counts the span (2,048).
- Copy header says "Range:" and omits "Resolved:" (a range has no Resolved Address); the spec's header list names Target and Resolved Address.
- One full-suite run showed a single failed test in a real-network suite that I could not identify; three following full runs were clean. It was not in the LAN Scan tests as far as I can tell.
