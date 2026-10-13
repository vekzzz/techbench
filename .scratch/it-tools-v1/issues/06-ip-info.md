# 06 — IP Info

**What to build:** The Technician opens IP Info and sees every network adapter's details, active ones first, plus the PC's public IP, ISP and city. This ticket adds adapter enumeration and the public lookup (the app's only external call) to the network adapter; later Tools rely on it to find the adapter carrying the default route.

Spec: user stories 8, 69–73; Implementation Decisions › Network adapter, Per-Tool rules › IP Info.

**Blocked by:** 03 — Port Check, single port.

**Status:** ready-for-agent
**Category:** enhancement

- [ ] Each adapter shows name, up/down state, IPv4 and IPv6 addresses with prefixes, gateway, DNS servers, MAC, DHCP enabled, and lease obtained/expires.
- [ ] Active adapters are listed first; inactive ones are collapsed.
- [ ] The adapter carrying the default route is identified, for use by Continuous Ping, DNS Lookup and LAN Scan.
- [ ] Public IP, ISP and city come from a single HTTPS call; if it fails, an error is shown and local details still display.
- [ ] Refresh re-reads all details.
- [ ] Copy as text works.
- [ ] Core tests with the fake adapter cover adapter ordering, default-route adapter identification, the public-lookup failure path, and golden Copy as text.
- [ ] A smoke test confirms real adapter enumeration returns at least one adapter; any test calling the public lookup is opt-in only.
