# 12 — Saved Targets

**What to build:** The Technician stores Targets under a label with an optional note, manages them from the Saved Targets screen, picks them in any Tool that takes a Target, and exports or imports them as JSON. The picker lives in the shared Target input, so Tools built after this ticket get it with no extra work.

Spec: user stories 17–23; Implementation Decisions › Toolbox core (Saved Target store), Tauri shell, UI.

**Blocked by:** 03 — Port Check, single port.

**Status:** ready-for-agent
**Category:** enhancement

- [ ] From the Saved Targets screen the Technician can create a Saved Target (label, address, optional note), edit it and delete it.
- [x] Saved Targets persist between launches in the app's data folder.
- [ ] The shared Target input offers Saved Targets in every Tool that takes a Target.
- [ ] A typed Target can be saved as a Saved Target from the Target input.
- [x] Export writes a JSON file with a format version; import adds entries and skips any whose label and address both match an existing Saved Target.
- [x] A malformed import file is rejected with a clear message and leaves the existing list untouched.
- [x] Core tests using a temporary folder cover create/read/update/delete, persistence across store instances, the export/import round trip, the duplicate-skip rule, and malformed import.

## Comments

**2026-10-14 — implementation pass (Claude).** Built test-first. 25 S1 tests over a temporary folder (`tempfile`, new dev-dependency).

- **Core `saved_targets`:** `SavedTargetStore::open(folder)` then `list`, `create`, `update`, `delete`, `export_to(path)`, `import_from(path)`. A Saved Target has an id, label, address and optional note. The label must not be empty and the address must be a valid hostname or IP address (the Target rules every Tool uses); spaces are trimmed. Every change is written to `saved-targets.json` in the folder by writing a temporary file and renaming it over the old one, and only becomes the list once that worked.
- **One JSON shape for file and export:** `{ "version": 1, "targets": [ { "id", "label", "address", "note"? } ] }`. Import adds entries (with fresh ids) and skips any whose label and address both match one already stored. A file that is not JSON, has the wrong shape, comes from a newer version, or has any invalid entry is rejected whole, naming the entry, and leaves the list and the store file untouched. A damaged store file at start-up is reported on the Saved Targets screen and never overwritten.
- **Shell:** the store lives in the app data folder (`%APPDATA%\com.venuel.ittools`). Commands to list, create, update, delete, export and import. Export and import use Tauri's dialog plugin for the save and open dialogs (new dependency `tauri-plugin-dialog`, `dialog:default` capability), then hand the chosen path to the core.
- **UI:** a Saved Targets screen (add/edit form, table with edit, delete and the action menu from ticket 13, Export and Import buttons with their outcome). The shared Target input gained a **Saved** picker and a **Save** button that stores the typed Target under a label, so every Tool that uses it has both.

My choices:
- Creating a duplicate by hand is allowed; only import skips duplicates, as the spec says.
- Ids are the highest stored id plus one, so an id can be reused after the newest Saved Target is deleted and the app restarted. Nothing refers to ids outside the screen.

**Not verified in the running app:** the Saved Targets screen, the Target input's picker and save, and the file dialogs. The app starts with the dialog plugin and the store setup in place.

**2026-10-14 — review follow-up (Claude).** Fixed:
- **DNS Lookup's field now has the Saved picker and Save button.** It was a plain input, so "every Tool that takes a Target" was not met. `TargetInput` takes an optional placeholder and optional IP version toggle; DNS Lookup uses it without the toggle.
- **The Save form's label is cleared after a save** (the next Save opened with the old label).
- **A damaged store file now shows its message in the Saved picker**, not only on the Saved Targets screen; it used to read "No Saved Targets yet".

Deferred: a hand-edited store file with repeated ids is not checked, so Delete would remove every entry with that id; a malformed entry that fails JSON parsing is reported without naming the entry (only entries that parse but are invalid are named); Export writes straight to the chosen file rather than through a temporary file.
