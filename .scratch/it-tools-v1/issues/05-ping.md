# 05 — Ping

**What to build:** The Technician pings a Target and sees one row per echo with its status, then a summary. This ticket adds ICMP echo to the network adapter, using the Windows ICMP helper API for IPv4 and IPv6 so it works without Administrator rights.

Spec: user stories 26–31; Implementation Decisions › Network adapter, Per-Tool rules › Ping.

**Blocked by:** 03 — Port Check, single port.

**Status:** ready-for-agent
**Category:** enhancement

- [ ] Defaults are 4 echoes, 32-byte payload, 1 s interval and 2 s timeout; count, payload size and timeout are editable.
- [ ] A Don't Fragment toggle is available; an oversized echo with Don't Fragment set reports fragmentation needed.
- [ ] Each echo row shows sequence number, round-trip time, TTL and status; reply, timed out, destination unreachable and fragmentation needed are distinct statuses.
- [ ] The summary shows sent, received, loss percentage and minimum/average/maximum round-trip time.
- [ ] Ping works over IPv4 and IPv6 and honours the force toggle.
- [ ] Stopping mid-Run gives a stopped Result summarising the echoes so far, and it can be copied.
- [ ] Ping runs without Administrator rights.
- [ ] Core tests with the fake adapter cover each echo status, the summary maths, stopping, and golden Copy as text.
- [ ] A smoke test pings `127.0.0.1` with the real adapter and receives replies.
