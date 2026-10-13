# 09 — DNS Lookup

**What to build:** The Technician looks up a name's records (or an address's name) against their PC's DNS server or one they choose, and sees each record with its TTL, which server answered and how long it took. Failure outcomes are distinct so the Technician knows whether the record, the zone or the server is at fault. This ticket adds direct DNS queries (UDP first, TCP fallback on truncation) to the network adapter.

Spec: user stories 45–51; Implementation Decisions › Network adapter, Per-Tool rules › DNS Lookup.

**Blocked by:** 06 — IP Info.

**Status:** ready-for-agent
**Category:** enhancement

- [ ] Record types A, AAAA, CNAME, MX, TXT, NS, PTR and SOA are supported.
- [ ] The default server is the first DNS server on the adapter carrying the default route; 8.8.8.8 and 1.1.1.1 are quick picks; any server can be typed.
- [ ] Entering an IP address as the query switches the record type to PTR automatically.
- [ ] Each record shows type, value and TTL; the Result names the answering server and the response time.
- [ ] NXDOMAIN, SERVFAIL, an empty answer and no response are reported as distinct, plain-language outcomes.
- [ ] Queries go to the chosen server directly; the hosts file is not consulted.
- [ ] Copy as text works.
- [ ] Core tests with the fake adapter cover record shaping per type, automatic PTR, the four outcomes, and golden Copy as text; tests against public DNS servers are opt-in only.
