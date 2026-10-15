# 11 — LAN Scan: hostnames, vendors, sort and filter

**What to build:** LAN Scan Devices gain a hostname (from reverse lookup) and a vendor (from a bundled copy of the IEEE OUI registry, so it works offline), and the Technician can sort and filter the Device list to find one Device in a busy subnet.

Spec: user stories 62 (hostname, vendor), 63, 64, 68; Implementation Decisions › Per-Tool rules › LAN Scan.

**Blocked by:** 08 — Traceroute (reverse lookup); 10 — LAN Scan: discovery.

**Status:** ready-for-agent
**Category:** enhancement

- [x] Each Device's hostname comes from reverse lookup and arrives after the Device first appears, without slowing the scan.
- [x] Each Device's vendor comes from the bundled OUI registry matched on the MAC prefix, with no network access.
- [x] Devices on other subnets show a blank MAC and vendor with an explanation that this is an ARP limitation.
- [x] Devices can be sorted by IP (numerically), hostname or vendor, and filtered by text.
- [x] Hostname and vendor are included in Copy as text.
- [x] The installer from ticket 02 includes the OUI data.
- [x] Core tests cover vendor lookup for known and unknown MAC prefixes, late hostname events, and golden Copy as text.

## Comments

**2026-10-14 — implementation pass (Claude).** Built test-first with tickets 12 and 13. 11 new S1 tests for LAN Scan (49 in all) over the fake network.

- **Vendor:** `oui::vendor(mac)` looks the first three bytes up in `crates/toolbox-core/data/oui.tsv`, compiled into the app with `include_str!` (40,217 MA-L assignments, 1.2 MB, "Private" entries left out). `python scripts/update-oui.py` rebuilds it from the IEEE registry. Because it is compiled in, the installer carries it with no extra bundling.
- **Hostname:** `LanScanEvent::Hostname { address, hostname }` arrives after the Device event. Reverse lookups run beside the probing, 16 at a time, each given up on after 2 s. A test shows a 1.5 s lookup does not delay a scan that needs 1 s of probing. Stopping once every address is probed only abandons names still coming in; the Result stays Completed.
- **Copy as text** gains HOSTNAME and VENDOR columns, and the footnote now reads "MAC and vendor are blank for devices beyond this PC's own subnet: ARP does not cross routers."
- **Screen:** filter box (matches IP, hostname, MAC or vendor), sort by IP (numeric), hostname or vendor (Devices without the detail last), "N of M shown", hostname filling in as it arrives. Sort and filter are kept in the app store, so they survive switching Tools. They change only the screen; Copy as text always lists every Device in IP order.

My choices:
- **Only MA-L (24-bit) assignments are bundled.** The newer MA-M and MA-S blocks (28/36-bit) are not, so some recent small vendors show no vendor. Randomised (locally administered) MACs show none either.
- A reverse lookup that finds nothing leaves the hostname as "-".

**Not verified in the running app:** the sort and filter controls and the late hostnames on screen (hence the unticked criterion). The app does start and open its window.

**Installer:** `npm run package` built `IT Tools_0.1.0_x64-setup.exe` (2.44 MiB, up from 1.71 MiB); the release `it-tools.exe` contains the vendor table. I did not run the new installer.

**2026-10-14 — review follow-up (Claude).** The sort type is now one `DeviceSort` type shared by the store and the screen.

Deferred: a timed-out reverse lookup frees its slot, but the Windows name lookup behind it keeps its thread until it finishes, so with a dead DNS server more than 16 can be waiting at once (same shape as the `SendARP` note on ticket 10). The filter also matches MAC, which the ticket did not ask for.

**2026-10-14 — GUI check (Claude).** Checked in the running app on 2026-10-14: the dev build (UI served by vite) driven through WebView2's DevTools protocol, so clicks and typing went into the app's own page, never another window; results read from the page, screenshots from the page, Copy checked on the Windows clipboard.
- On a real scan of 19 Devices: IP order is numeric (`.9` before `.11`); Vendor sort runs CLOUD NETWORK…, Fiberhome…, Google, Intel, NETGEAR, TP-Link, then Devices without a vendor; Hostname sort puts WORKSTATION-A, MAC-A1B2C3, LAPTOP-B first.
- Filter `tp-link` → 5 TP-Link Devices and "5 of 19 shown"; `macbook` → 192.168.1.19 by hostname; `5C-A6-E6-9A` → 4 by MAC. Sort and filter were still set after visiting Saved Targets and coming back.
