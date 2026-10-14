import { open, save } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";

import { ActionMenu } from "@/components/action-menu";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { exportSavedTargets, importSavedTargets, type SavedTarget, type SavedTargetInput } from "@/lib/toolbox";
import { useSavedTargets } from "@/store";

const EMPTY: SavedTargetInput = { label: "", address: "", note: "" };
const JSON_FILES = [{ name: "JSON", extensions: ["json"] }];

export function SavedTargets() {
  const { targets, error, loaded, load, create, update, remove, draft, setDraft } = useSavedTargets();
  const [form, setForm] = useState<SavedTargetInput>(EMPTY);
  /** The Saved Target being edited; null when the form adds a new one. */
  const [editing, setEditing] = useState<number | null>(null);
  const [formError, setFormError] = useState<string | null>(null);
  /** The outcome of the last export or import, or why it failed. */
  const [transfer, setTransfer] = useState<{ text: string; failed: boolean } | null>(null);

  useEffect(() => {
    if (!loaded) void load();
  }, [loaded, load]);

  // A Device or Hop sent here with Save as Target arrives as a draft to confirm.
  useEffect(() => {
    if (draft === null) return;
    setForm(draft);
    setEditing(null);
    setFormError(null);
    setDraft(null);
  }, [draft, setDraft]);

  const reset = () => {
    setForm(EMPTY);
    setEditing(null);
    setFormError(null);
  };

  const submit = async () => {
    try {
      if (editing === null) await create(form);
      else await update(editing, form);
      reset();
    } catch (reason) {
      setFormError(String(reason));
    }
  };

  const edit = (saved: SavedTarget) => {
    setForm({ label: saved.label, address: saved.address, note: saved.note ?? "" });
    setEditing(saved.id);
    setFormError(null);
  };

  const exportAll = async () => {
    try {
      const path = await save({ defaultPath: "it-tools-saved-targets.json", filters: JSON_FILES });
      if (path === null) return;
      await exportSavedTargets(path);
      setTransfer({ text: `Exported ${targets.length} Saved Target${targets.length === 1 ? "" : "s"} to ${path}`, failed: false });
    } catch (reason) {
      setTransfer({ text: String(reason), failed: true });
    }
  };

  const importFile = async () => {
    try {
      const path = await open({ multiple: false, filters: JSON_FILES });
      if (typeof path !== "string") return;
      const { added, skipped } = await importSavedTargets(path);
      await load();
      setTransfer({
        text: `Imported ${added} Saved Target${added === 1 ? "" : "s"}${skipped > 0 ? `; skipped ${skipped} already stored` : ""}`,
        failed: false,
      });
    } catch (reason) {
      setTransfer({ text: String(reason), failed: true });
    }
  };

  return (
    <div className="flex max-w-3xl flex-col gap-6">
      <form
        className="flex flex-col gap-3"
        onSubmit={(event) => {
          event.preventDefault();
          void submit();
        }}
      >
        <div className="grid grid-cols-2 gap-3">
          <div className="flex flex-col gap-2">
            <Label htmlFor="saved-label">Label</Label>
            <Input
              id="saved-label"
              autoComplete="off"
              placeholder="Core switch"
              value={form.label}
              onChange={(event) => setForm({ ...form, label: event.target.value })}
            />
          </div>
          <div className="flex flex-col gap-2">
            <Label htmlFor="saved-address">Address</Label>
            <Input
              id="saved-address"
              spellCheck={false}
              autoComplete="off"
              className="font-mono"
              placeholder="Hostname or IP address"
              value={form.address}
              onChange={(event) => setForm({ ...form, address: event.target.value })}
            />
          </div>
        </div>
        <div className="flex flex-col gap-2">
          <Label htmlFor="saved-note">Note (optional)</Label>
          <Input
            id="saved-note"
            autoComplete="off"
            placeholder="VLAN 20, rack B"
            value={form.note}
            onChange={(event) => setForm({ ...form, note: event.target.value })}
          />
        </div>
        {formError && <p className="text-xs text-destructive">{formError}</p>}
        <div className="flex items-center gap-2">
          <Button type="submit">{editing === null ? "Add Saved Target" : "Save changes"}</Button>
          {(editing !== null || form !== EMPTY) && (
            <Button type="button" variant="outline" onClick={reset}>
              {editing === null ? "Clear" : "Cancel"}
            </Button>
          )}
        </div>
      </form>

      {error && <p className="text-xs text-destructive">{error}</p>}
      {loaded && !error && targets.length === 0 && (
        <p className="text-xs text-muted-foreground">No Saved Targets yet. Add one above, or import a backup.</p>
      )}
      {targets.length > 0 && (
        <table className="w-full overflow-hidden rounded-md border text-xs">
          <thead className="bg-muted/40 text-left text-muted-foreground">
            <tr>
              <th className="px-3 py-1.5 font-normal">Label</th>
              <th className="px-3 py-1.5 font-normal">Address</th>
              <th className="px-3 py-1.5 font-normal">Note</th>
              <th className="px-3 py-1.5 font-normal" />
            </tr>
          </thead>
          <tbody>
            {targets.map((saved) => (
              <tr key={saved.id} className="border-t">
                <td className="px-3 py-1.5">{saved.label}</td>
                <td className="px-3 py-1.5 font-mono">{saved.address}</td>
                <td className="px-3 py-1.5 text-muted-foreground">{saved.note ?? "-"}</td>
                <td className="px-3 py-1.5">
                  <div className="flex justify-end gap-1">
                    <ActionMenu address={saved.address} />
                    <Button type="button" size="xs" variant="ghost" onClick={() => edit(saved)}>
                      Edit
                    </Button>
                    <Button
                      type="button"
                      size="xs"
                      variant="ghost"
                      onClick={() => {
                        if (editing === saved.id) reset();
                        void remove(saved.id).catch((reason) => setFormError(String(reason)));
                      }}
                    >
                      Delete
                    </Button>
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      <div className="flex flex-col gap-2">
        <div className="flex items-center gap-2">
          <Button type="button" variant="outline" disabled={targets.length === 0} onClick={() => void exportAll()}>
            Export…
          </Button>
          <Button type="button" variant="outline" onClick={() => void importFile()}>
            Import…
          </Button>
        </div>
        {transfer && (
          <p className={transfer.failed ? "text-xs text-destructive" : "text-xs text-muted-foreground"}>{transfer.text}</p>
        )}
      </div>
    </div>
  );
}
