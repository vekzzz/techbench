import { useState } from "react";

import { Button } from "@/components/ui/button";

/** Copies a Result's plain text to the clipboard, ready to paste into a ticket. */
export function CopyResult({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <Button
      type="button"
      variant="outline"
      onClick={() =>
        void navigator.clipboard.writeText(text).then(() => {
          setCopied(true);
          setTimeout(() => setCopied(false), 1500);
        })
      }
    >
      {copied ? "Copied" : "Copy as text"}
    </Button>
  );
}
