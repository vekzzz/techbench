import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { LEAD_ACTIONS, followLead, saveAsTarget, type LeadAction } from "@/lib/follow-lead";

/**
 * The action menu on a Device, Hop or Saved Target: takes its address to another Tool with the
 * Run started. Pass `saveable` to also offer Save as Target (not for a Saved Target, which is one).
 */
export function ActionMenu({
  address,
  hostname,
  saveable = false,
  only,
}: {
  address: string;
  /** Pre-fills the label of Save as Target when known. */
  hostname?: string | null;
  saveable?: boolean;
  /** Offer just these actions, e.g. the Speed Test Server only makes sense for the path tools. */
  only?: LeadAction[];
}) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button type="button" size="xs" variant="ghost" aria-label={`Actions for ${address}`}>
          Actions
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        {LEAD_ACTIONS.filter((action) => !only || only.includes(action.id)).map((action) => (
          <DropdownMenuItem key={action.id} onSelect={() => void followLead(action.id, address)}>
            {action.label}
          </DropdownMenuItem>
        ))}
        {saveable && (
          <>
            <DropdownMenuSeparator />
            <DropdownMenuItem onSelect={() => saveAsTarget(address, hostname)}>Save as Target</DropdownMenuItem>
          </>
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
