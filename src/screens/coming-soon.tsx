import { HugeiconsIcon } from "@hugeicons/react";

import type { Screen } from "@/tools";

export function ComingSoon({ screen }: { screen: Screen }) {
  return (
    <div className="flex flex-col items-start gap-2 rounded-md border border-dashed p-6 text-muted-foreground">
      <HugeiconsIcon icon={screen.icon} strokeWidth={1.5} className="size-6" />
      <p className="text-sm text-foreground">{screen.name} is coming soon.</p>
      <p className="text-xs">{screen.summary}</p>
    </div>
  );
}
