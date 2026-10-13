import { useEffect } from "react";

import { CopyResult } from "@/components/copy-result";
import { TargetInput } from "@/components/target-input";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { milliseconds } from "@/lib/format";
import { pingDefaults, type EchoReport, type EchoStatus } from "@/lib/toolbox";
import { usePing, type PingRun } from "@/store";

export function Ping() {
  const { input, error, run, update, start, stop } = usePing();
  const running = run !== null && run.result === null;
  usePingDefaults();

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
          <Label htmlFor="ping-target">Target</Label>
          <TargetInput
            id="ping-target"
            autoFocus
            value={input.target}
            onChange={(value) => update({ target: value })}
            family={input.family}
            onFamilyChange={(value) => update({ family: value })}
          />
        </div>
        <div className="flex flex-wrap items-end gap-3">
          <NumberField id="ping-count" label="Echoes" value={input.count} onChange={(count) => update({ count })} />
          <NumberField
            id="ping-size"
            label="Size (bytes)"
            value={input.payloadSize}
            onChange={(payloadSize) => update({ payloadSize })}
          />
          <NumberField
            id="ping-timeout"
            label="Timeout (ms)"
            value={input.timeoutMs}
            onChange={(timeoutMs) => update({ timeoutMs })}
          />
          <div className="flex h-7 items-center gap-2">
            <Checkbox
              id="ping-dont-fragment"
              checked={input.dontFragment}
              onCheckedChange={(checked) => update({ dontFragment: checked === true })}
            />
            <Label htmlFor="ping-dont-fragment" title="Forbid fragmenting, to probe the path MTU. IPv4 only: Windows cannot forbid it for IPv6.">
              Don't Fragment
            </Label>
          </div>
        </div>
        {error && <p className="text-xs text-destructive">{error}</p>}
        <div className="flex items-center gap-2">
          <Button type="submit" variant={running ? "outline" : "default"}>
            {running ? "Stop" : "Ping"}
          </Button>
          {run?.result && <CopyResult text={run.result.text} />}
        </div>
      </form>

      {run && <RunView run={run} />}
    </div>
  );
}

/** Fills the count, size and timeout with the toolbox core's defaults the first time Ping opens. */
function usePingDefaults() {
  const { input, update } = usePing();
  const unset = input.count === "" && input.payloadSize === "" && input.timeoutMs === "";
  useEffect(() => {
    if (unset) void pingDefaults().then(update);
  }, [unset, update]);
}

function NumberField({
  id,
  label,
  value,
  onChange,
}: {
  id: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <div className="flex flex-col gap-2">
      <Label htmlFor={id}>{label}</Label>
      <Input
        id={id}
        inputMode="numeric"
        spellCheck={false}
        autoComplete="off"
        className="w-28 font-mono"
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
    </div>
  );
}

function RunView({ run }: { run: PingRun }) {
  const result = run.result;

  if (result?.ending === "resolutionFailed") {
    return <p className="text-xs text-destructive">{result.summary}</p>;
  }

  return (
    <div className="flex flex-col gap-3">
      <p className="text-xs text-muted-foreground">
        Resolved address: <span className="font-mono text-foreground">{run.resolved ?? "resolving…"}</span>
      </p>
      {!result && (
        <p className="text-xs text-muted-foreground">
          Pinging… {run.echoesRequested !== null && `${run.echoes.length} of ${run.echoesRequested} echoes finished`}
        </p>
      )}
      {run.echoes.length > 0 && <EchoTable echoes={run.echoes} />}
      {result && <p className="text-xs text-muted-foreground">{result.summary}</p>}
    </div>
  );
}

function EchoTable({ echoes }: { echoes: EchoReport[] }) {
  return (
    <table className="w-full overflow-hidden rounded-md border text-xs">
      <thead className="bg-muted/40 text-left text-muted-foreground">
        <tr>
          <th className="px-3 py-1.5 font-normal">Seq</th>
          <th className="px-3 py-1.5 font-normal">Status</th>
          <th className="px-3 py-1.5 font-normal">Time</th>
          <th className="px-3 py-1.5 font-normal">TTL</th>
        </tr>
      </thead>
      <tbody>
        {echoes.map((echo) => (
          <tr key={echo.sequence} className="border-t">
            <td className="px-3 py-1.5 font-mono">{echo.sequence}</td>
            <td className="px-3 py-1.5">
              <StatusBadge status={echo.status} />
              {echo.from && <span className="ml-2 font-mono text-muted-foreground">from {echo.from}</span>}
            </td>
            <td className="px-3 py-1.5 font-mono">{milliseconds(echo.roundTripMs)}</td>
            <td className="px-3 py-1.5 font-mono">{echo.ttl ?? "-"}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

const STATUSES: Record<EchoStatus, { label: string; variant: "default" | "secondary" | "outline" | "destructive" }> = {
  reply: { label: "Reply", variant: "default" },
  timedOut: { label: "Timed out", variant: "outline" },
  unreachable: { label: "Unreachable", variant: "destructive" },
  fragmentationNeeded: { label: "Fragmentation needed", variant: "secondary" },
};

function StatusBadge({ status }: { status: EchoStatus }) {
  const { label, variant } = STATUSES[status];
  return <Badge variant={variant}>{label}</Badge>;
}
