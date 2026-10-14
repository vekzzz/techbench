import { useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import type { AddressFamily } from "@/lib/toolbox";
import { useSavedTargets } from "@/store";

const FAMILIES: { value: AddressFamily; label: string }[] = [
  { value: "auto", label: "Auto" },
  { value: "ipv4", label: "IPv4" },
  { value: "ipv6", label: "IPv6" },
];

/** The Target field every Tool that takes a Target uses, with its Saved Targets picker and IP version toggle. */
export function TargetInput({
  id,
  value,
  onChange,
  family,
  onFamilyChange,
  placeholder = "Hostname or IP address",
  autoFocus = false,
}: {
  id: string;
  value: string;
  onChange: (value: string) => void;
  /** Leave out both for a Tool with no IP version to force, such as DNS Lookup. */
  family?: AddressFamily;
  onFamilyChange?: (family: AddressFamily) => void;
  placeholder?: string;
  autoFocus?: boolean;
}) {
  const { targets, error, loaded, load, create } = useSavedTargets();
  const [saving, setSaving] = useState(false);
  const [label, setLabel] = useState("");
  const [saveError, setSaveError] = useState<string | null>(null);

  useEffect(() => {
    if (!loaded) void load();
  }, [loaded, load]);

  const save = async () => {
    try {
      await create({ label, address: value, note: "" });
      setSaving(false);
      setLabel("");
      setSaveError(null);
    } catch (reason) {
      setSaveError(String(reason));
    }
  };

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center gap-2">
        <Input
          id={id}
          autoFocus={autoFocus}
          spellCheck={false}
          autoComplete="off"
          className="font-mono"
          placeholder={placeholder}
          value={value}
          onChange={(event) => onChange(event.target.value)}
        />
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button type="button" size="sm" variant="outline" className="shrink-0">
              Saved
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="max-h-72 overflow-auto">
            {error ? (
              <DropdownMenuLabel className="max-w-80 font-normal whitespace-normal text-destructive">{error}</DropdownMenuLabel>
            ) : targets.length === 0 ? (
              <DropdownMenuLabel className="font-normal text-muted-foreground">No Saved Targets yet</DropdownMenuLabel>
            ) : (
              targets.map((saved) => (
                <DropdownMenuItem key={saved.id} onSelect={() => onChange(saved.address)}>
                  <span>{saved.label}</span>
                  <span className="ml-3 font-mono text-muted-foreground">{saved.address}</span>
                </DropdownMenuItem>
              ))
            )}
          </DropdownMenuContent>
        </DropdownMenu>
        <Button
          type="button"
          size="sm"
          variant="outline"
          className="shrink-0"
          disabled={value.trim() === ""}
          onClick={() => {
            setSaving(!saving);
            setSaveError(null);
          }}
        >
          Save
        </Button>
        {family !== undefined && onFamilyChange && (
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
        )}
      </div>
      {saving && (
        <div className="flex flex-col gap-1">
          <div className="flex items-center gap-2">
            <Input
              autoFocus
              spellCheck={false}
              autoComplete="off"
              placeholder={`Label for ${value.trim()}`}
              value={label}
              onChange={(event) => setLabel(event.target.value)}
              onKeyDown={(event) => {
                // Enter here saves the Target; it must not start the Run the surrounding form belongs to.
                if (event.key === "Enter") {
                  event.preventDefault();
                  void save();
                }
              }}
            />
            <Button type="button" size="sm" onClick={() => void save()}>
              Save Target
            </Button>
            <Button type="button" size="sm" variant="ghost" onClick={() => setSaving(false)}>
              Cancel
            </Button>
          </div>
          {saveError && <p className="text-xs text-destructive">{saveError}</p>}
        </div>
      )}
    </div>
  );
}
