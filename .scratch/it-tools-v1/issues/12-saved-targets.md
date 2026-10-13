# 12 — Saved Targets

**What to build:** The Technician stores Targets under a label with an optional note, manages them from the Saved Targets screen, picks them in any Tool that takes a Target, and exports or imports them as JSON. The picker lives in the shared Target input, so Tools built after this ticket get it with no extra work.

Spec: user stories 17–23; Implementation Decisions › Toolbox core (Saved Target store), Tauri shell, UI.

**Blocked by:** 03 — Port Check, single port.

**Status:** ready-for-agent
**Category:** enhancement

- [ ] From the Saved Targets screen the Technician can create a Saved Target (label, address, optional note), edit it and delete it.
- [ ] Saved Targets persist between launches in the app's data folder.
- [ ] The shared Target input offers Saved Targets in every Tool that takes a Target.
- [ ] A typed Target can be saved as a Saved Target from the Target input.
- [ ] Export writes a JSON file with a format version; import adds entries and skips any whose label and address both match an existing Saved Target.
- [ ] A malformed import file is rejected with a clear message and leaves the existing list untouched.
- [ ] Core tests using a temporary folder cover create/read/update/delete, persistence across store instances, the export/import round trip, the duplicate-skip rule, and malformed import.
