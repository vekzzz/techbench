import { CopyResult } from "@/components/copy-result";
import { TargetInput } from "@/components/target-input";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { milliseconds } from "@/lib/format";
import type { HopInfo, ProbeInfo } from "@/lib/toolbox";
import { useTraceroute, type TracerouteRun } from "@/store";

export function Traceroute() {
  const { input, error, run, update, start, stop } = useTraceroute();
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
          <Label htmlFor="traceroute-target">Target</Label>
          <TargetInput
            id="traceroute-target"
            autoFocus
            value={input.target}
            onChange={(target) => update({ target })}
            family={input.family}
            onFamilyChange={(family) => update({ family })}
          />
        </div>
        {error && <p className="text-xs text-destructive">{error}</p>}
        <div className="flex items-center gap-2">
          <Button type="submit" variant={running ? "outline" : "default"}>
            {running ? "Stop" : "Start"}
          </Button>
          {run && <CopyResult text={run.result?.text} runId={run.id} />}
        </div>
      </form>

      {run && <RunView run={run} />}
    </div>
  );
}

function RunView({ run }: { run: TracerouteRun }) {
  if (run.result?.ending === "resolutionFailed") {
    return <p className="text-xs text-destructive">{run.result.summary}</p>;
  }
  return (
    <div className="flex flex-col gap-3">
      <p className="text-xs text-muted-foreground">
        Resolved address: <span className="font-mono text-foreground">{run.resolved ?? "resolving…"}</span>
      </p>
      {run.hops.length > 0 && <HopTable hops={run.hops} />}
      {!run.result && (
        <p className="text-xs text-muted-foreground">
          Following the path… {run.hops.length} hop{run.hops.length === 1 ? "" : "s"} found
        </p>
      )}
      {run.result && <p className="text-xs text-muted-foreground">{run.result.summary}</p>}
    </div>
  );
}

function HopTable({ hops }: { hops: HopInfo[] }) {
  return (
    <table className="w-full overflow-hidden rounded-md border text-xs">
      <thead className="bg-muted/40 text-left text-muted-foreground">
        <tr>
          <th className="px-3 py-1.5 font-normal">Hop</th>
          <th className="px-3 py-1.5 font-normal">Address</th>
          <th className="px-3 py-1.5 font-normal">Hostname</th>
          <th className="px-3 py-1.5 font-normal" colSpan={3}>
            Round trips
          </th>
        </tr>
      </thead>
      <tbody>
        {hops.map((hop) => (
          <tr key={hop.number} className="border-t">
            <td className="px-3 py-1.5 font-mono">{hop.number}</td>
            <td className="px-3 py-1.5 font-mono">{hop.address ?? "*"}</td>
            <td className="px-3 py-1.5 text-muted-foreground">{hop.hostname ?? "-"}</td>
            {hop.probes.map((probe, index) => (
              <td key={index} className="px-3 py-1.5 font-mono">
                {probeText(probe)}
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/** A probe's round trip (with the router, when it is not the Hop's own); `*` silent (a filtering router, not a slow one); `!` unreachable; `×` could not be sent from this PC. */
function probeText(probe: ProbeInfo) {
  if (probe.status === "silent") return "*";
  if (probe.status === "unreachable") return "!";
  if (probe.status === "couldNotSend") return "×";
  return probe.from ? `${milliseconds(probe.roundTripMs)} (${probe.from})` : milliseconds(probe.roundTripMs);
}
