# 04 — Port Check: lists, ranges and Port Presets

**What to build:** The Technician checks many TCP ports on one Target in a single Run, by typing a list with ranges or by choosing a Port Preset. Each port shows its outcome and a service name hint, outcomes appear as each port finishes, and a whole Preset completes in about two seconds even when every port is Filtered.

Spec: user stories 52, 53, 56, 57; Implementation Decisions › Per-Tool rules › Port Check.

**Blocked by:** 03 — Port Check, single port.

**Status:** ready-for-agent
**Category:** enhancement

- [x] The port field accepts comma-separated ports and inclusive ranges (e.g. `22, 80, 8000-8010`), deduplicated and sorted; invalid entries are rejected with a clear message.
- [x] A list of more than 1,024 ports is rejected with a clear message.
- [x] Up to 32 connects run in parallel with a 2 s timeout each, so an all-Filtered Preset finishes in roughly one timeout, not one per port.
- [x] The seven built-in Port Presets (Windows, Directory, Web, Remote, Mail, Database, Printing) contain exactly the ports listed in the spec; choosing one fills the port list; Presets are not editable.
- [x] Well-known ports show a service name hint (e.g. 3389 RDP, 5985 WinRM).
- [x] Outcomes stream in as each port finishes; the summary counts Open, Closed and Filtered.
- [x] Core tests cover list and range parsing, deduplication, the cap, Preset contents, concurrent classification with the fake adapter, and golden Copy as text for a multi-port Result.

## Comments

**2026-10-13 — implementation pass (Claude).** Built test-first at seam S1 over the fake network (S2). `tests/port_check.rs` now has 33 tests.
- **The paused tokio clock pins the timing exactly.** A silent Preset of 5 ports takes one 2 s timeout; 40 silent ports take 4 s (32, then 8).
- **The request field `port` is now `ports`** in the core, the shell and the UI.
- **The core** gains `PortCheck::ports()`, `PORT_PRESETS`, `service_name()` and `PortCheckResult::ports_requested`. Connects run through `buffer_unordered(32)`. Outcomes are reported as each port finishes, and the Result is sorted by port.
- **A stopped Run** keeps the ports that already answered and reports "Stopped after N of M ports".
- **The shell** adds `port_presets()`, and `start_port_check` now returns `{ id, portCount }` so the UI can show progress.
- **Small choices of mine:**
  - An empty entry (`22, , 80`) is rejected rather than ignored.
  - A backwards range (`90-80`) is rejected and the message suggests `80-90`.
  - Every error names the offending entry.

Verified in the running app:
- The Windows Preset on localhost: 135 RPC and 445 SMB Open; 3389, 5985 and 5986 Closed. The copied text matched the golden format.
- The Web Preset on a silent address showed "0 of 4 ports answered" at about 1 s, then 4 Filtered at about 2.7 s. One at a time would have taken 8 s.
- `1-64` stopped after the first wave copied as "Stopped after 32 of 64 ports".
