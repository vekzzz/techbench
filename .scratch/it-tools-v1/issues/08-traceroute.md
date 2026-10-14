# 08 — Traceroute

**What to build:** The Technician traces the path to a Target and watches each Hop appear as it is discovered, with hostnames filling in afterwards. This ticket adds reverse lookup to the network adapter.

Spec: user stories 39–44; Implementation Decisions › Network adapter, Per-Tool rules › Traceroute.

**Blocked by:** 05 — Ping.

**Status:** ready-for-agent
**Category:** enhancement

- [x] Traceroute sends ICMP echo with TTL increasing from 1, 3 probes per Hop, 1 s timeout per probe, up to 30 Hops.
- [x] It stops as soon as the Resolved Address replies.
- [ ] Each Hop row appears as discovered with its number, address and three round-trip times; silent probes show `*`.
- [x] Hop hostnames arrive as later events and never delay discovery of the next Hop.
- [x] Traceroute works over IPv4 and IPv6.
- [ ] A trace can be stopped mid-way, and a stopped or running trace can be copied.
- [x] Core tests with the fake adapter cover Hop assembly, `*` for silent probes, stopping at the Target, the 30-Hop limit, late hostnames, and golden Copy as text.
- [x] A smoke test traces `127.0.0.1` with the real adapter and gets one Hop.

## Comments

**2026-10-13 — note from ticket 05 (Claude).** The network adapter's `icmp_echo` doesn't yet do what Traceroute needs:
- `EchoRequest` has no TTL; the Windows adapter always sends 128.
- `EchoOutcome::Reply` doesn't carry the responding address.
- There is no time-exceeded outcome; `windows_icmp.rs` currently maps `IP_TTL_EXPIRED_TRANSIT` to Unreachable.

Add all three here, following the spec's S2 description ("ICMP echo … with TTL … returning reply (round-trip time, TTL, responding address), time exceeded (responding address, used by Traceroute)"). Ping keeps sending TTL 128.

**2026-10-14 — implementation pass (Claude).** Built test-first at seam S1 over the fake network (S2) and tokio's paused clock: 14 core tests, 3 smoke tests (loopback IPv4 and IPv6; one opt-in test to 8.8.8.8, which passed).

Adapter (the three things the note from ticket 05 asked for, plus names):
- `EchoRequest.ttl`, and `EchoOutcome::TimeExceeded { from, round_trip }` (Windows reports a round trip for TTL-expired replies too). Ping keeps sending TTL 128 and shows a "Time exceeded" row if one ever comes back.
- `Network::reverse_lookup`, through `getnameinfo` (Windows' own resolver). The responding address of a *Reply* is not added: a Reply is always from the Target itself, which Traceroute already knows.

Traceroute (`traceroute` module):
- Up to 30 Hops. The three probes of a Hop go out **together** (1 s timeout), so a silent Hop costs one second, not three.
- Stops when the Target answers, or when a router reports the destination unreachable.
- Reverse lookups run **beside** the probing and arrive as `Hostname` events; each gives up after 2 s.
- Copy as text works mid-Run (same `Progress` mechanism as ticket 07): hop table with `*` for silent probes, then "Reached 8.8.8.8 in 9 hops" / "Did not reach … within 30 hops" / "Destination unreachable: … reported it at hop N" / "Stopped after N hops" / "Still running: N hops found so far".
- Shell `start_traceroute`, store, and a Traceroute screen with a table that fills in as Hops arrive.

**Not verified in the running app:** the Traceroute screen (rows appearing, hostnames filling in, Copy while running). My GUI check failed: the app window was not actually visible where I captured, and I captured an unrelated browser page instead (deleted; nothing was typed or clicked). Boxes "Each Hop row appears as discovered…" and "A trace can be stopped mid-way, and a stopped or running trace can be copied" stay unticked for a manual look.
