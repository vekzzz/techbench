# 04 — Port Check: lists, ranges and Port Presets

**What to build:** The Technician checks many TCP ports on one Target in a single Run, by typing a list with ranges or by choosing a Port Preset. Each port shows its outcome and a service name hint, outcomes appear as each port finishes, and a whole Preset completes in about two seconds even when every port is Filtered.

Spec: user stories 52, 53, 56, 57; Implementation Decisions › Per-Tool rules › Port Check.

**Blocked by:** 03 — Port Check, single port.

**Status:** ready-for-agent
**Category:** enhancement

- [ ] The port field accepts comma-separated ports and inclusive ranges (e.g. `22, 80, 8000-8010`), deduplicated and sorted; invalid entries are rejected with a clear message.
- [ ] A list of more than 1,024 ports is rejected with a clear message.
- [ ] Up to 32 connects run in parallel with a 2 s timeout each, so an all-Filtered Preset finishes in roughly one timeout, not one per port.
- [ ] The seven built-in Port Presets (Windows, Directory, Web, Remote, Mail, Database, Printing) contain exactly the ports listed in the spec; choosing one fills the port list; Presets are not editable.
- [ ] Well-known ports show a service name hint (e.g. 3389 RDP, 5985 WinRM).
- [ ] Outcomes stream in as each port finishes; the summary counts Open, Closed and Filtered.
- [ ] Core tests cover list and range parsing, deduplication, the cap, Preset contents, concurrent classification with the fake adapter, and golden Copy as text for a multi-port Result.
