import { Button } from "@/components/ui/button";
import type { AddressFamily } from "@/lib/toolbox";

const FAMILIES: { value: AddressFamily; label: string }[] = [
  { value: "auto", label: "Auto" },
  { value: "ipv4", label: "IPv4" },
  { value: "ipv6", label: "IPv6" },
];

/** Which IP version a Run may use: Auto prefers IPv4. */
export function FamilyToggle({
  family,
  onChange,
}: {
  family: AddressFamily;
  onChange: (family: AddressFamily) => void;
}) {
  return (
    <div role="radiogroup" aria-label="IP version" className="flex shrink-0 rounded-md border p-0.5">
      {FAMILIES.map((option) => (
        <Button
          key={option.value}
          type="button"
          size="xs"
          role="radio"
          aria-checked={family === option.value}
          variant={family === option.value ? "secondary" : "ghost"}
          onClick={() => onChange(option.value)}
        >
          {option.label}
        </Button>
      ))}
    </div>
  );
}
