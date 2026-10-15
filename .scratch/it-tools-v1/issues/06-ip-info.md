# 06 — IP Info

**What to build:** The Technician opens IP Info and sees every network adapter's details, active ones first, plus the PC's public IP, ISP and city. This ticket adds adapter enumeration and the public lookup (the app's only external call) to the network adapter; later Tools rely on it to find the adapter carrying the default route.

Spec: user stories 8, 69–73; Implementation Decisions › Network adapter, Per-Tool rules › IP Info.

**Blocked by:** 03 — Port Check, single port.

**Status:** ready-for-agent
**Category:** enhancement

- [x] Each adapter shows name, up/down state, IPv4 and IPv6 addresses with prefixes, gateway, DNS servers, MAC, DHCP enabled, and lease obtained/expires.
- [x] Active adapters are listed first; inactive ones are collapsed.
- [x] The adapter carrying the default route is identified, for use by Continuous Ping, DNS Lookup and LAN Scan.
- [x] Public IP, ISP and city come from a single HTTPS call; if it fails, an error is shown and local details still display.
- [x] Refresh re-reads all details.
- [x] Copy as text works.
- [x] Core tests with the fake adapter cover adapter ordering, default-route adapter identification, the public-lookup failure path, and golden Copy as text.
- [x] A smoke test confirms real adapter enumeration returns at least one adapter; any test calling the public lookup is opt-in only.

## Comments

**2026-10-13 — implementation pass (Claude).** Built test-first at seam S1 over the fake network (S2): 11 core tests, 1 smoke test, and 1 opt-in internet test (`cargo test --workspace -- --include-ignored`).

The core (`ip_info`):
- **Run order:** IP Info is a Run, so it can be stopped: adapters first, then the slower public lookup.
- **Loopback** is left out.
- **Unspecified gateways are dropped.** Teredo reports `::` as a gateway, which routes nothing.
- **The public `default_route_adapter()` rule** is for Continuous Ping, DNS Lookup and LAN Scan. Of the active adapters with an IPv4 gateway, the lowest IPv4 metric wins; failing that, the same for IPv6.
- **Ordering:** the default-route adapter, then other active adapters by name, then inactive ones.
- **Copy as text** shows only the fields each adapter actually has.

The real adapter:
- **`windows_adapters.rs`** uses `GetAdaptersAddresses`, plus `GetAdaptersInfo` for DHCP lease times, which only that older call reports (IPv4 only).
- **`public_ip.rs`** makes one HTTPS call to `https://ipinfo.io/json` through `ureq`, using Windows' own TLS (native-tls/schannel) and certificate store, with a 5 s timeout. The ISP name comes from ipinfo's `org` with the AS number removed.

The shell and UI: `start_ip_info`, `useIpInfo`, and the IP Info screen. It reads everything the first time it opens; Refresh re-reads; active adapters are expanded and inactive ones collapsed.

Verified on this PC:
- Wi-Fi was marked as the default route, with its DHCP lease times.
- Public: 203.0.113.7 (Example Telecom, Manila).
- Teredo no longer shows a `::` gateway.
- The copied text matched the format.

Note: corporate networks that need a web proxy aren't handled yet. `ureq` doesn't read the Windows proxy settings by default.

**2026-10-13 — review follow-up (Claude).** Fixed:
- **The default route comes from the routing table.** It was a guess from gateways and metrics, which misses a full-tunnel VPN that installs a default route with no gateway. Now the core asks the routing table which local address internet traffic leaves from (`source_address(8.8.8.8)`, then IPv6; nothing is sent) and picks the active adapter holding that address. The gateway-metric rule is only the fallback when there is no route. `ip_info::find_default_route(network)` gives this to tickets 07, 09 and 10, with notes left on each.
- **The Copy header names the PC's IP**: `PC: name (address)`.
- **New golden** for a stopped Run.
- **The smoke test goes through `ip_info::run`.** It stops when the adapters arrive, so it never reaches the internet.
- **`windows_adapters.rs`:**
  - `adapter_from` and `socket_ip` are now `unsafe fn` with documented contracts.
  - `linked` ties its references to the buffer it reads.
  - Every `unsafe` has its own SAFETY comment.
  - The fixed-size GUID is read with `CStr::from_bytes_until_nul`.
  - Both IP Helper calls share one sizing-and-retry helper.
- **The core owns the wording:** `describe_dhcp`, `ipv4_addresses` and `ipv6_addresses` are used by the shell, the pass-through `public_text` is gone, and times go through `format_time`.
- **Naming:** the TypeScript types `Adapter` and `PublicIp` now match the core.
- **Glossary:** added "Adapter" and "Default Route Adapter", and a Run is now "usually against a Target", because IP Info has none.

Accepted:
- IP Info has no summary line.
- The default route is identified by adapter name, which is unique on Windows.
- The unspecified-gateway check stays in two places, because both the display filter and the public rule need it.
- Loopback is filtered out, and descriptions, kinds and the DHCP server are shown.
