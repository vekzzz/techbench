# 05 — Ping

**What to build:** The Technician pings a Target and sees one row per echo with its status, then a summary. This ticket adds ICMP echo to the network adapter, using the Windows ICMP helper API for IPv4 and IPv6 so it works without Administrator rights.

Spec: user stories 26–31; Implementation Decisions › Network adapter, Per-Tool rules › Ping.

**Blocked by:** 03 — Port Check, single port.

**Status:** ready-for-agent
**Category:** enhancement

- [x] Defaults are 4 echoes, 32-byte payload, 1 s interval and 2 s timeout; count, payload size and timeout are editable.
- [x] A Don't Fragment toggle is available; an oversized echo with Don't Fragment set reports fragmentation needed.
- [x] Each echo row shows sequence number, round-trip time, TTL and status; reply, timed out, destination unreachable and fragmentation needed are distinct statuses.
- [x] The summary shows sent, received, loss percentage and minimum/average/maximum round-trip time.
- [x] Ping works over IPv4 and IPv6 and honours the force toggle.
- [x] Stopping mid-Run gives a stopped Result summarising the echoes so far, and it can be copied.
- [x] Ping runs without Administrator rights.
- [x] Core tests with the fake adapter cover each echo status, the summary maths, stopping, and golden Copy as text.
- [x] A smoke test pings `127.0.0.1` with the real adapter and receives replies.

## Comments

**2026-10-13 — implementation pass (Claude).** Built test-first at seam S1 over the fake network (S2) and tokio's paused clock: 12 core tests and 2 smoke tests.

Prefactors deferred from the ticket 03 review, now that a second Tool needs them:
- **`ChosenTarget` in the core** bundles a Target with its IP version (`parse`, `resolve`).
- **`createRunStore` in the UI** owns a Run's lifecycle: the stale-update guard, the Run id, rejected input, and stopping the previous Run. Port Check and Ping both use it.
- **The shell** shares `AddressFamilyInput` and `EndingView`.
- **The UI** shares `CopyResult` and `milliseconds()`.

Ping itself:
- **The S2 trait gains `icmp_echo`**, built on `EchoRequest` and `EchoOutcome` (Reply, TimedOut, Unreachable, FragmentationNeeded).
- **The real adapter** (`windows_icmp.rs`) uses `IcmpSendEcho2` and `Icmp6SendEcho2` on tokio's blocking pool, with no admin rights needed. IPv6 replies carry no TTL.
- **Echoes go 1 s after the previous one was sent**, or at once if waiting for its reply already took longer.
- **Copy as text** adds an `Echoes:` line (count x size, timeout, Don't Fragment) so the evidence of an MTU test is complete.
- **My limits:** count 1–1000, size 0–65500 bytes (the same maximum as Windows `ping`), timeout 100–60000 ms. Each error names its field.
- **The UI takes its defaults from the core** through a new `ping_defaults` command.

Verified in the running app against 8.8.8.8:
- 4 replies with TTL 118.
- 1473 bytes with Don't Fragment gave "Fragmentation needed", and 1472 bytes with Don't Fragment gave replies (path MTU 1500).
- Stopping a 10-echo Run partway copied as "Stopped after 3 of 10 echoes".
- Ping kept its state across a Tool switch, and Port Check still works on the shared store.

Housekeeping: F: ran out of space because `target\debug` had reached 19.5 GB. I ran `cargo clean --profile dev`, and the dev profile now keeps line tables only for our crates and no debug info for dependencies.
