# Spec: Speed Test — an internet speed Tool

Status: ready-for-agent
Category: enhancement

Vocabulary follows [CONTEXT.md](../../CONTEXT.md) (Speed Test, Speed Test Server, Loaded Latency, Increase Under Load). Respects [ADR 0001](../../docs/adr/0001-tools-run-only-on-technician-pc.md) and [ADR 0002](../../docs/adr/0002-tauri-with-native-windows-network-apis.md); the provider choice is [ADR 0003](../../docs/adr/0003-speed-test-uses-cloudflare-public-endpoints.md). Extends the [v1 spec](../it-tools-v1/spec.md), which listed speedtest as out of scope.

## Problem Statement

When a user says "the internet is slow", the Technician has no quick answer on the PC in front of them: they open a browser speed-test site, get a number with no record of which server, which adapter or which public IP it came from, and cannot paste anything useful into a ticket or an ISP complaint. Those sites also hide the thing that most often breaks Teams and VoIP calls: latency that balloons while someone uploads (bufferbloat). Comparing Wi-Fi against Ethernet or a VPN means repeating the whole routine and remembering what each run said.

## Solution

An eighth sidebar Tool, **Speed Test**, that runs without a Target and measures, against Cloudflare's public speed endpoints: idle latency and jitter, download speed, upload speed, and Loaded Latency during each direction with a Low/Moderate/High verdict on the Increase Under Load. The Result copies as plain text with a header recording the Speed Test Server, time, the PC and the Adapter it used, and the public IP/ISP, so it can go straight into a ticket or an ISP complaint. Wi-Fi-vs-Ethernet and VPN comparisons are done by running twice and comparing two copied Results.

## User Stories

1. As a Technician, I want one button that measures latency, download and upload, so that I get "is the internet slow?" answered in under a minute.
2. As a Technician, I want the live speed and the current phase (latency, download, upload) shown while it runs, so that I can see it is working.
3. As a Technician, I want a live graph of speed and Loaded Latency, so that I can see the line saturate and latency spike.
4. As a Technician, I want idle latency (median) and jitter, so that I know the line's baseline quality.
5. As a Technician, I want Loaded Latency for download and for upload separately, with the Increase Under Load graded Low, Moderate or High, so that I can tell whether uploads or downloads wreck call quality.
6. As a Technician, I want download and upload speeds that reflect what the line can carry, measured with several connections, so that a fast line is not under-reported.
7. As a Technician, I want an optional plan speed (download and upload), so that the Result says "38 Mbps (19% of 200 Mbps plan)" for an ISP complaint.
8. As a Technician, I want the Result to show which Speed Test Server answered (location code and IP), so that a slow result can be tied to a path.
9. As a Technician, I want an action menu on the Speed Test Server's address offering Ping, Continuous Ping and Traceroute, so that I can chase where the path slows down.
10. As a Technician, I want the Result to show the data used, so that I know what the Run cost.
11. As a Technician, I want to be asked before a Run starts on a metered, roaming or over-limit connection, so that a hotspot or capped LTE backup line is not drained by accident.
12. As a Technician, I want the same IPv4/IPv6 toggle as other Tools, so that I can compare address families on a dual-stack fault.
13. As a Technician, I want a Run to fail fast with "no internet" when the server is unreachable, so that I do not wait on a timeout.
14. As a Technician, I want a rate limit from Cloudflare shown with its wait time, and nothing retried silently, so that I know why a Run did not happen.
15. As a Technician, I want an interrupted Run to keep the phases that finished and mark the rest not measured, so that partial evidence is not thrown away.
16. As a Technician, I want to Stop a Run at any time and still copy what was measured.
17. As a Technician, I want to copy the Result as plain text with a header naming the Speed Test Server, time, my PC with its Adapter, and my public IP/ISP, so that a ticket reader knows when, from where and over which line it ran.
18. As a Technician, I want Speed Test to keep running while I use other Tools, and other Tools to run alongside it, so that I can start a Continuous Ping to the gateway during a Speed Test and watch bufferbloat on its graph.
19. As a Technician, I want star ratings for Web Browsing, Streaming, Gaming and Video Calls, so that I can tell a user in plain words what their connection is good for.

## Implementation Decisions

### Shape

- A new Tool `speed-test` with no Target. One Speed Test Run at a time; Start is disabled while one runs. It follows the existing three layers: core module `speed_test.rs` with all logic, a thin Tauri shell command, a React screen. State and the live Run live in the app-level store, so switching Tools never stops it.
- The plan speeds (download, upload) are optional screen inputs kept in the store until the app closes; they are never written to disk.

### Rules of the internet-facing Tools

- The v1 rule "IP Info's public lookup is the app's only external call" becomes: Tools whose subject is the internet (IP Info's public lookup, Speed Test) may call out; every other Tool stays offline. The `public_ip()` doc comment in `network.rs` is updated in ticket 01.

### Network adapter seam (S2)

New narrow operations; no measuring logic behind them:

- `speed_probe(family)` → one empty HTTP request (`__down?bytes=0`) on an open connection: round trip, the server's own time from `server-timing`, the Speed Test Server's location code and address.
- `speed_download(stop, family)` / `speed_upload(stop, family)` → one stream of chunks: bytes and timing as they arrive or are sent. Each request stays under 100 MB (25 MB downloads, 10 MB uploads, repeated). The blocking read loop checks the cancellation token between chunks.
- `connection_cost()` → metered / unmetered / roaming / over-limit / unknown, from Windows' connection cost. Needs no elevation. Makes the `windows` crate (already in Cargo.lock through Tauri) a direct dependency. Added in ticket 03.
- The real implementation uses the existing `ureq` (native-tls) inside `spawn_blocking`. A fake implementation scripts transfer rate and latency per call on tokio's paused clock.
- Request rules (ADR 0003): honest User-Agent, no Origin/Referer pretence, no result posting, requests under 100 MB.

### Measurement (core)

- **Idle latency**: 20 probes; median of (round trip − server time). **Jitter**: mean absolute difference between consecutive samples, the definition Continuous Ping uses.
- **Download / upload**: 4 connections at once, 10 s each direction. The first 2 s are ignored; speed = bytes in the remaining window ÷ the window's length. Reported in Mbps.
- **Loaded Latency**: a probe every 250 ms on a separate connection during each direction; the median per direction. **Increase Under Load** = Loaded Latency − idle latency. Verdict per direction: Low under 30 ms, Moderate 30–100 ms, High over 100 ms.
- **Ratings**: one to five stars per use case, each the lowest of the stars its figures earn, so one weak figure cannot hide behind strong ones. A use case is not rated when a figure it needs was not measured. Thresholds (the least for 5, 4, 3, 2 stars; below the last is 1):
  - Web Browsing: download 25 / 10 / 5 / 2 Mbps; latency at most 50 / 100 / 200 / 400 ms.
  - Streaming: download 50 / 25 / 10 / 5 Mbps; jitter at most 30 / 60 / 100 / 200 ms.
  - Gaming: latency at most 30 / 60 / 100 / 150 ms; jitter at most 10 / 20 / 40 / 80 ms.
  - Video Calls: upload 10 / 5 / 3 / 1 Mbps; latency at most 100 / 150 / 250 / 400 ms; jitter at most 20 / 30 / 50 / 100 ms.
  - Packet loss is not measured (ADR 0003), so ratings say nothing about it.
- **Data used**: total bytes transferred, shown in the Result.
- **Plan comparison**: with a plan speed set, the Result adds "% of plan" for that direction.
- **Address family**: IPv4 by default, forced IPv6 on request, like other Tools; the Server line shows the family used.
- **Public IP/ISP**: the existing `public_ip()` lookup, run in parallel with the idle latency phase; its failure is shown in the header and does not end the Run.
- **Default Route Adapter**: the header names the Adapter the Run went out through, found with `ip_info::find_default_route`.

### Endings

Completed, Stopped, **No internet** (server unreachable at the start, fails within about 5 s), **Rate limited** (HTTP 429, shows `Retry-After`, no automatic retry), **Interrupted** (connection dropped part-way; finished phases kept, the rest marked not measured). Stopping always leaves a Result containing everything collected.

### Copy as text

Header follows IP Info's shape plus the Server:

```
Speed Test
Server:    Cloudflare HKG (104.16.124.96)
Time:      2026-10-14 14:05:09 +08:00
PC:        BPSI-VENUEL (192.168.1.23) via Wi-Fi
Public:    203.177.x.x, PLDT, Makati
```

Then aligned plain-text rows for latency, jitter, download and upload (with Loaded Latency, Increase Under Load and verdict per direction), the data used, and one summary line. No Markdown. Core-owned and golden-tested like every other Tool.

### UI

- Sidebar entry, big live Mbps figure with the current phase, then the Result table.
- A live graph (SVG, in the style of Continuous Ping's): speed samples plus a Loaded Latency line (ticket 04).
- Start / Stop; IPv4/IPv6 toggle; optional plan inputs.
- Confirmation dialog on a metered, roaming or over-limit connection, like LAN Scan's range confirmation (ticket 03).
- The Server address uses the existing action menu with Ping, Continuous Ping and Traceroute only.

## Testing Decisions

- Same philosophy as v1: tests call the core's public entry point with a request, assert on events, the final Result and the Copy as text, never on internals. Written test-first; the `tdd` skill guides the loop.
- **Default**: S1 with a fake S2 and the paused clock covers idle median and jitter, the 2 s warm-up window, 4-connection aggregation, Loaded Latency medians, verdict thresholds at 29/30/100/101 ms, data used, plan percentages, family forcing, each ending (No internet, Rate limited, Interrupted keeping finished phases, Stopped mid-phase), public-lookup failure in the header, one-Run-at-a-time, and golden Copy as text.
- **Smoke, opt-in** (`speed_test_on_this_pc.rs`, not run by default): real Cloudflare with small transfers — a 1 MB download, a 1 MB upload, 3 probes — proving the calls, header parsing and location code without spending gigabytes.
- **UI**: no automated tests; verified manually in the running app, as in v1.

## Out of Scope

- LAN throughput (iperf-style) — needs a far-end agent (ADR 0001); backlog next to remote agents.
- Run history and trends over time (the v1 spec excludes a persistent Run log).
- Packet loss, and Ookla, M-Lab, LibreSpeed or fast.com as providers (ADR 0003).
- Persisting plan speeds per site; there is no site concept.
- Choosing a different Adapter or server, an Ookla-style dial, and automatic retries.
- Blocking or warning when other Runs are active.

## Further Notes

- Data use per Run is roughly speed × 10 s per direction: about 0.25 GB on a 100 Mbps line and up to about 2.5 GB at 1 Gbps. That is why ticket 03 (metered confirmation) follows straight after the core Tool.
- Cloudflare's use is a grey area (ADR 0003). If it starts refusing the endpoints, the provider is swappable at the seam; Ookla via a Technician-installed `speedtest.exe` is the noted fallback.
- Build order: 01 latency and download → 02 upload and bufferbloat → 03 metered confirmation → 04 live graph → 05 plan comparison. The three goals (sanity check, ISP evidence, bufferbloat) are met after 02.
