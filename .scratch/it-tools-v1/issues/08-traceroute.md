# 08 — Traceroute

**What to build:** The Technician traces the path to a Target and watches each Hop appear as it is discovered, with hostnames filling in afterwards. This ticket adds reverse lookup to the network adapter.

Spec: user stories 39–44; Implementation Decisions › Network adapter, Per-Tool rules › Traceroute.

**Blocked by:** 05 — Ping.

**Status:** ready-for-agent
**Category:** enhancement

- [x] Traceroute sends ICMP echo with TTL increasing from 1, 3 probes per Hop, 1 s timeout per probe, up to 30 Hops.
- [x] It stops as soon as the Resolved Address replies.
- [x] Each Hop row appears as discovered with its number, address and three round-trip times; silent probes show `*`.
- [x] Hop hostnames arrive as later events and never delay discovery of the next Hop.
- [x] Traceroute works over IPv4 and IPv6.
- [x] A trace can be stopped mid-way, and a stopped or running trace can be copied.
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

**2026-10-14 — review follow-up (Claude).** Fixed:
- **A failure to send from this PC looked like 30 silent Hops.** When no route exists or the adapter is down, every probe fails at once, so the trace flashed through 30 rows of `*` and ended "Did not reach". It now ends at the first Hop: "Could not send probes from this PC to 8.8.8.8: there is no route, or the adapter is down" (`×` in the table).
- **Load-balanced Hops hid the other routers.** A probe answered by a different router than the Hop's own address now shows it: `2 ms (10.0.0.3)`, in the text and in the UI.
- **Stopping after the Target was reached turned a complete trace into a stopped one.** Stopping while names are still arriving now just abandons the names; the Result stays Completed.
- **Glossary:** "trace" and "router" (as a name for a Hop) removed from the code, tests and UI ("Start" button; "Following the path… 1 hop found"); a copy-paste plural bug fixed.
- **`windows_names.rs`:** a dead "answer equals the address" guard removed (NI_NAMEREQD already fails in that case), and the Winsock comment now says it is started once for the process.

Accepted or deferred:
- A Reply carries no responding address. A reply always comes from the Target, which Traceroute already knows; the spec's S2 sentence is not literally met.
- Probes of a Hop go out together, so a router that rate-limits ICMP may show a few more `*` than sequential probing would.
- Ending on a router's "destination unreachable", the 2 s name timeout, and waiting up to 2 s at the end for the last names are additions beyond the ticket.
- IPv6 routers (time exceeded over ICMPv6) rest on the Windows API's documented behaviour; only IPv6 loopback was exercised for real.

**2026-10-14 — GUI check (Claude).** Checked in the running app on 2026-10-14: the dev build (UI served by vite) driven through WebView2's DevTools protocol, so clicks and typing went into the app's own page, never another window; results read from the page, screenshots from the page, Copy checked on the Windows clipboard.
- To 192.0.2.1 (never reached): 3 rows after 1.5 s, 6 after 4 s, 9 after 7 s; hop 1 the gateway, hop 2 with its hostname filled in, later Hops `*` in every probe column.
- Copy while running ended "Still running: 9 hops found so far". Stop ended it at "Stopped after 17 hops", no rows arrived afterwards, and Copy gave the stopped Result.
