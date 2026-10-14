# 09 — DNS Lookup

**What to build:** The Technician looks up a name's records (or an address's name) against their PC's DNS server or one they choose, and sees each record with its TTL, which server answered and how long it took. Failure outcomes are distinct so the Technician knows whether the record, the zone or the server is at fault. This ticket adds direct DNS queries (UDP first, TCP fallback on truncation) to the network adapter.

Spec: user stories 45–51; Implementation Decisions › Network adapter, Per-Tool rules › DNS Lookup.

**Blocked by:** 06 — IP Info.

**Status:** ready-for-agent
**Category:** enhancement

- [x] Record types A, AAAA, CNAME, MX, TXT, NS, PTR and SOA are supported.
- [x] The default server is the first DNS server on the adapter carrying the default route; 8.8.8.8 and 1.1.1.1 are quick picks; any server can be typed.
- [x] Entering an IP address as the query switches the record type to PTR automatically.
- [ ] Each record shows type, value and TTL; the Result names the answering server and the response time.
- [x] NXDOMAIN, SERVFAIL, an empty answer and no response are reported as distinct, plain-language outcomes.
- [x] Queries go to the chosen server directly; the hosts file is not consulted.
- [ ] Copy as text works.
- [x] Core tests with the fake adapter cover record shaping per type, automatic PTR, the four outcomes, and golden Copy as text; tests against public DNS servers are opt-in only.

## Comments

**2026-10-13 — note from ticket 06 (Claude).** Call `ip_info::find_default_route(network)` to get the Default Route Adapter. The routing table decides it, so a VPN's DNS servers win when the VPN carries the default route. Its `dns_servers` gives the default server: use the first one.

**2026-10-14 — implementation pass (Claude).** Built test-first. 17 S1 tests over the fake network; 14 tests of the real DNS client against a small DNS server the test runs on this PC (UDP and TCP, built byte by byte from RFC 1035); 1 opt-in test against 8.8.8.8, which passed (`cargo test -- --ignored`).

- **S2 `dns_query(server, name, type)`** returns `DnsReply { answer, round_trip }`. The real client (`dns_client.rs`) is hand-written rather than a DNS crate: encoder, UDP with one retry (two tries of 2 s), TCP when the answer is truncated, and a decoder for A, AAAA, CNAME, MX, TXT, NS, PTR and SOA with bounded compression-pointer following. It talks to the chosen server directly, so the hosts file and the system resolver never get a say.
- **Outcomes** are distinct: records; NXDOMAIN; "exists, but has no AAAA records"; SERVFAIL, REFUSED or "error code N"; an answer that could not be read; no response.
- **An IP address is looked up as PTR automatically**: `8.8.4.8` becomes `8.4.8.8.in-addr.arpa`, and IPv6 goes under `ip6.arpa` a nibble at a time. The Result says "PTR (chosen automatically)" and shows the name actually asked.
- **Default server:** the first DNS server of the Default Route Adapter (`ip_info::find_default_route`), skipping a link-local IPv6 one (unreachable without its zone). With none configured, the Result says so. The screen shows which server an empty field means; 8.8.8.8 and 1.1.1.1 are quick picks.
- **Shell and UI:** `start_dns_lookup`, `default_dns_server`, a DNS Lookup screen (record type buttons, server field with quick picks, record table, summary line, Copy as text).

My choices: names with underscores are accepted (`_dmarc.example.com`); the TXT value shows each string in quotes; the query id comes from the clock and a counter, with the random source port doing the work of making answers hard to spoof.

**Not verified in the running app:** the DNS Lookup screen. I did not attempt the GUI check again after the problems on tickets 07 and 08, so the criterion list is ticked for the core and the real client only. Please try the screen once.
