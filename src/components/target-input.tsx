import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { AddressFamily } from "@/lib/toolbox";

const FAMILIES: { value: AddressFamily; label: string }[] = [
  { value: "auto", label: "Auto" },
  { value: "ipv4", label: "IPv4" },
  { value: "ipv6", label: "IPv6" },
];

/** The Target field every Tool that takes a Target uses, with its IP version toggle. */
export function TargetInput({
  id,
  value,
  onChange,
  family,
  onFamilyChange,
  invalid = false,
  autoFocus = false,
}: {
  id: string;
  value: string;
  onChange: (value: string) => void;
  family: AddressFamily;
  onFamilyChange: (family: AddressFamily) => void;
  invalid?: boolean;
  autoFocus?: boolean;
}) {
  return (
    <div className="flex items-center gap-2">
      <Input
        id={id}
        autoFocus={autoFocus}
        spellCheck={false}
        autoComplete="off"
        className="font-mono"
        placeholder="Hostname or IP address"
        value={value}
        aria-invalid={invalid}
        onChange={(event) => onChange(event.target.value)}
      />
      <div role="radiogroup" aria-label="IP version" className="flex shrink-0 rounded-md border p-0.5">
        {FAMILIES.map((option) => (
          <Button
            key={option.value}
            type="button"
            size="xs"
            role="radio"
            aria-checked={family === option.value}
            variant={family === option.value ? "secondary" : "ghost"}
            onClick={() => onFamilyChange(option.value)}
          >
            {option.label}
          </Button>
        ))}
      </div>
    </div>
  );
}
