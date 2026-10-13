# 08 — Traceroute

**What to build:** The Technician traces the path to a Target and watches each Hop appear as it is discovered, with hostnames filling in afterwards. This ticket adds reverse lookup to the network adapter.

Spec: user stories 39–44; Implementation Decisions › Network adapter, Per-Tool rules › Traceroute.

**Blocked by:** 05 — Ping.

**Status:** ready-for-agent
**Category:** enhancement

- [ ] Traceroute sends ICMP echo with TTL increasing from 1, 3 probes per Hop, 1 s timeout per probe, up to 30 Hops.
- [ ] It stops as soon as the Resolved Address replies.
- [ ] Each Hop row appears as discovered with its number, address and three round-trip times; silent probes show `*`.
- [ ] Hop hostnames arrive as later events and never delay discovery of the next Hop.
- [ ] Traceroute works over IPv4 and IPv6.
- [ ] A trace can be stopped mid-way, and a stopped or running trace can be copied.
- [ ] Core tests with the fake adapter cover Hop assembly, `*` for silent probes, stopping at the Target, the 30-Hop limit, late hostnames, and golden Copy as text.
- [ ] A smoke test traces `127.0.0.1` with the real adapter and gets one Hop.

## Comments

**2026-10-13 — note from ticket 05 (Claude).** The network adapter's `icmp_echo` doesn't yet do what Traceroute needs:
- `EchoRequest` has no TTL; the Windows adapter always sends 128.
- `EchoOutcome::Reply` doesn't carry the responding address.
- There is no time-exceeded outcome; `windows_icmp.rs` currently maps `IP_TTL_EXPIRED_TRANSIT` to Unreachable.

Add all three here, following the spec's S2 description ("ICMP echo … with TTL … returning reply (round-trip time, TTL, responding address), time exceeded (responding address, used by Traceroute)"). Ping keeps sending TTL 128.
