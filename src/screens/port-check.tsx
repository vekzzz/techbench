import { useEffect, useState } from "react";

import { TargetInput } from "@/components/target-input";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { portPresets, type PortOutcome, type PortPreset, type PortReport } from "@/lib/toolbox";
import { useAppStore, type PortCheckRun } from "@/store";

export function PortCheck() {
  const { target, ports, family, error, run } = useAppStore((state) => state.portCheck);
  const update = useAppStore((state) => state.updatePortCheck);
  const start = useAppStore((state) => state.startPortCheck);
  const stop = useAppStore((state) => state.stopPortCheck);
  const presets = usePortPresets();
  const running = run !== null && run.result === null;

  return (
    <div className="flex max-w-3xl flex-col gap-6">
      <form
        className="flex flex-col gap-4"
        onSubmit={(event) => {
          event.preventDefault();
          if (running) stop();
          else void start();
        }}
      >
        <div className="flex flex-col gap-2">
          <Label htmlFor="port-check-target">Target</Label>
          <TargetInput
            id="port-check-target"
            autoFocus
            value={target}
            onChange={(value) => update({ target: value })}
            family={family}
            onFamilyChange={(value) => update({ family: value })}
          />
        </div>
        <div className="flex flex-col gap-2">
          <Label htmlFor="port-check-ports">Ports</Label>
          <Input
            id="port-check-ports"
            spellCheck={false}
            autoComplete="off"
            className="font-mono"
            placeholder="443   or   22, 80, 8000-8010"
            value={ports}
            onChange={(event) => update({ ports: event.target.value })}
          />
          <div className="flex flex-wrap items-center gap-1">
            <span className="mr-1 text-xs text-muted-foreground">Presets</span>
            {presets.map((preset) => (
              <Button
                key={preset.name}
                type="button"
                size="xs"
                variant="outline"
                title={preset.ports.join(", ")}
                onClick={() => update({ ports: preset.ports.join(", ") })}
              >
                {preset.name}
              </Button>
            ))}
          </div>
        </div>
        {error && <p className="text-xs text-destructive">{error}</p>}
        <div className="flex items-center gap-2">
          <Button type="submit" variant={running ? "outline" : "default"}>
            {running ? "Stop" : "Check"}
          </Button>
          {run?.result && <CopyResult text={run.result.text} />}
        </div>
      </form>

      {run && <RunView run={run} />}
    </div>
  );
}

/** The built-in Port Presets, fetched once from the toolbox core. */
function usePortPresets() {
  const [presets, setPresets] = useState<PortPreset[]>([]);
  useEffect(() => {
    void portPresets().then(setPresets);
  }, []);
  return presets;
}

function RunView({ run }: { run: PortCheckRun }) {
  const result = run.result;

  if (result?.ending === "resolutionFailed") {
    return <p className="text-xs text-destructive">Could not resolve the Target: {result.failureReason}</p>;
  }

  const ports = [...run.ports].sort((a, b) => a.port - b.port);
  return (
    <div className="flex flex-col gap-3">
      <p className="text-xs text-muted-foreground">
        Resolved address: <span className="font-mono text-foreground">{run.resolved ?? "resolving…"}</span>
      </p>
      {!result && (
        <p className="text-xs text-muted-foreground">
          Checking… {run.portsRequested !== null && `${run.ports.length} of ${run.portsRequested} ports checked`}
        </p>
      )}
      {ports.length > 0 && <PortTable ports={ports} />}
      {result && <p className="text-xs text-muted-foreground">{result.summary}</p>}
    </div>
  );
}

function PortTable({ ports }: { ports: PortReport[] }) {
  return (
    <table className="w-full overflow-hidden rounded-md border text-xs">
      <thead className="bg-muted/40 text-left text-muted-foreground">
        <tr>
          <th className="px-3 py-1.5 font-normal">Port</th>
          <th className="px-3 py-1.5 font-normal">Service</th>
          <th className="px-3 py-1.5 font-normal">Outcome</th>
          <th className="px-3 py-1.5 font-normal">Connect time</th>
        </tr>
      </thead>
      <tbody>
        {ports.map((report) => (
          <tr key={report.port} className="border-t">
            <td className="px-3 py-1.5 font-mono">{report.port}</td>
            <td className="px-3 py-1.5 text-muted-foreground">{report.service ?? "-"}</td>
            <td className="px-3 py-1.5">
              <OutcomeBadge outcome={report.outcome} />
            </td>
            <td className="px-3 py-1.5 font-mono">{connectTime(report.connectTimeMs)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

const OUTCOMES: Record<PortOutcome, { label: string; variant: "default" | "secondary" | "outline"; hint: string }> = {
  open: { label: "Open", variant: "default", hint: "The Target accepted the connection." },
  closed: { label: "Closed", variant: "secondary", hint: "The Target refused the connection: it is up, but nothing is listening." },
  filtered: { label: "Filtered", variant: "outline", hint: "No answer before the timeout, usually a firewall dropping it." },
};

function OutcomeBadge({ outcome }: { outcome: PortOutcome }) {
  const { label, variant, hint } = OUTCOMES[outcome];
  return (
    <Badge variant={variant} title={hint}>
      {label}
    </Badge>
  );
}

function connectTime(milliseconds: number | null) {
  if (milliseconds === null) return "-";
  return milliseconds < 1 ? "<1 ms" : `${Math.round(milliseconds)} ms`;
}

function CopyResult({ text }: { text: string }) {
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
