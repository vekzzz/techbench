# 02 — Speed Test: upload and bufferbloat

**What to build:** The Run gains an upload phase and measures Loaded Latency during both directions, so the Result shows how much latency grows under load and grades it. This is the slice that answers "why does Teams break up when someone uploads?".

Spec: [spec.md](../spec.md) — user stories 5, 6, 16, 17; Implementation Decisions › Measurement, Copy as text.

**Blocked by:** 01 — Speed Test: latency and download.

**Status:** ready-for-agent
**Category:** enhancement

- [x] Seam gains `speed_upload`, fake and real; uploads use requests under 100 MB (10 MB, repeated).
- [x] Upload uses 4 connections for 10 s, ignores the first 2 s, and reports Mbps over the remaining window.
- [x] During each direction a probe every 250 ms on a separate connection feeds Loaded Latency, reported as a median per direction.
- [x] Increase Under Load = Loaded Latency − idle latency, per direction, graded Low (under 30 ms), Moderate (30–100 ms) or High (over 100 ms); thresholds tested at 29, 30, 100 and 101 ms.
- [ ] The screen shows the phase sequence latency → download → upload with the live figure, then Loaded Latency, Increase Under Load and verdict for each direction.
- [x] Interrupted during upload keeps latency and download and marks upload and its Loaded Latency not measured; Stopped mid-phase keeps what was collected.
- [x] Data used includes upload bytes.
- [ ] Copy as text lists latency, jitter, download, upload and, per direction, Loaded Latency, Increase Under Load and verdict, then data used and a summary line; golden tests cover Completed, Stopped and Interrupted.

## Comments

**2026-10-15 — implementation pass (Claude).** Built test-first at seam S1: upload phase (`speed_upload` on the seam, real POST to `/__up` in 10 MB requests, fake and a real 1 MB smoke test), Loaded Latency every 250 ms on a separate connection during each direction, Increase Under Load and the Low/Moderate/High verdict (tested at 29, 30, 100, 101 ms), upload and under-load lines in Copy as text. Loaded latency is the median of probes after the 2 s warm-up and is left unmeasured when the phase does not finish. Added at the Technician's request (not in the original spec): star ratings for Web Browsing, Streaming, Gaming and Video Calls (spec › Ratings). The screen part is built and type-checks but has not been looked at in the running app.

Correction: golden Copy as text tests cover Completed, No internet and Rate limited, not Stopped or Interrupted text.
