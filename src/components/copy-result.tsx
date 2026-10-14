import { useState } from "react";

import { Button } from "@/components/ui/button";
import { runText, type RunId } from "@/lib/toolbox";

/**
 * Copies a Result's plain text to the clipboard, ready to paste into a ticket. Pass `text` once
 * the Run has finished; while it is still going, pass its `runId` to copy the Result so far.
 */
export function CopyResult({ text, runId }: { text?: string | null; runId?: RunId | null }) {
  const [note, setNote] = useState<string | null>(null);
  const flash = (message: string) => {
    setNote(message);
    setTimeout(() => setNote(null), 1500);
  };
  const copy = async () => {
    const current = text ?? (runId != null ? await runText(runId) : null);
    if (current === null) return flash("Nothing to copy yet");
    await navigator.clipboard.writeText(current);
    flash("Copied");
  };

  return (
    <Button type="button" variant="outline" disabled={!text && runId == null} onClick={() => void copy()}>
      {note ?? "Copy as text"}
    </Button>
  );
}
