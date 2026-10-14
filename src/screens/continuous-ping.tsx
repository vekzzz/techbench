import { useState } from "react";

import { CopyResult } from "@/components/copy-result";
import { TargetInput } from "@/components/target-input";
import { Button } from "@/components/ui/button";
import { milliseconds } from "@/lib/format";
import { defaultGateway, type PingStatistics } from "@/lib/toolbox";
import { GRAPH_POINTS, continuousPingSlots, useAppStore, type ContinuousPingRun } from "@/store";

/** Up to three Continuous Pings side by side, e.g. the gateway and 8.8.8.8 to tell a LAN problem from an ISP one. */
export function ContinuousPing() {
  const shown = useAppStore((state) => state.continuousTargets);
  const setShown = useAppStore((state) => state.setContinuousTargets);
  const slots = continuousPingSlots.slice(0, shown);
  const [gatewayNote, setGatewayNote] = useState<string | null>(null);

  const startAll = () => slots.forEach((slot) => slot.getState().input.target.trim() && void slot.getState().start());
  const stopAll = () => slots.forEach((slot) => slot.getState().stop());

  /** Puts the default gateway in the first empty Target slot, adding a slot if every shown one is in use. */
  const useGateway = async () => {
    const gateway = await defaultGateway();
    if (gateway === null) {
      setGatewayNote("No default gateway found.");
      return;
    }
    setGatewayNote(null);
    const empty = slots.findIndex((slot) => !slot.getState().input.target.trim());
    const index = empty >= 0 ? empty : shown < 3 ? shown : -1;
    if (index < 0) {
      setGatewayNote("All three Targets are in use.");
      return;
    }
    if (index >= shown) setShown(index + 1);
    continuousPingSlots[index].getState().update({ target: gateway, family: "auto" });
  };

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-center gap-2">
        <Button onClick={startAll}>Start all</Button>
        <Button variant="outline" onClick={stopAll}>
          Stop all
        </Button>
        <Button variant="outline" disabled={shown >= 3} onClick={() => setShown(shown + 1)}>
          Add Target
        </Button>
        <Button variant="ghost" onClick={() => void useGateway()}>
          Use default gateway
        </Button>
        {gatewayNote && <span className="text-xs text-muted-foreground">{gatewayNote}</span>}
      </div>
      {slots.map((_, index) => (
        <TargetSlot
          key={index}
          index={index}
          removable={shown > 1 && index === shown - 1}
          onRemove={() => {
            continuousPingSlots[index].getState().stop();
            setShown(shown - 1);
          }}
        />
      ))}
    </div>
  );
}

function TargetSlot({ index, removable, onRemove }: { index: number; removable: boolean; onRemove: () => void }) {
  const { input, error, run, update, start, stop } = continuousPingSlots[index]();
  const running = run !== null && run.result === null;

  return (
    <section className="flex flex-col gap-3 rounded-md border p-3">
      <form
        className="flex flex-col gap-2"
        onSubmit={(event) => {
          event.preventDefault();
          if (running) stop();
          else void start();
        }}
      >
        <TargetInput
          id={`continuous-target-${index}`}
          value={input.target}
          onChange={(target) => update({ target })}
          family={input.family}
          onFamilyChange={(family) => update({ family })}
        />
        <div className="flex items-center gap-2">
          <Button type="submit" size="sm" variant={running ? "outline" : "default"}>
            {running ? "Stop" : "Start"}
          </Button>
          {run && <CopyResult text={run.result?.text} runId={run.id} />}
          {removable && (
            <Button type="button" size="sm" variant="ghost" className="ml-auto" onClick={onRemove}>
              Remove
            </Button>
          )}
        </div>
        {error && <p className="text-xs text-destructive">{error}</p>}
      </form>
      {run && <SlotView run={run} />}
    </section>
  );
}

function SlotView({ run }: { run: ContinuousPingRun }) {
  if (run.result?.ending === "resolutionFailed") {
    return <p className="text-xs text-destructive">{run.result.summary}</p>;
  }
  return (
    <div className="flex flex-col gap-2">
      <p className="text-xs text-muted-foreground">
        Resolved address: <span className="font-mono text-foreground">{run.resolved ?? "resolving…"}</span>
      </p>
      {run.statistics && <Totals statistics={run.statistics} />}
      <LatencyGraph points={run.points} />
      {run.result && <p className="text-xs text-muted-foreground">{run.result.summary}</p>}
    </div>
  );
}

function Totals({ statistics }: { statistics: PingStatistics }) {
  const totals: [string, string][] = [
    ["Sent", String(statistics.sent)],
    ["Received", String(statistics.received)],
    ["Loss", `${statistics.lossPercent}%`],
    ["Min", milliseconds(statistics.minMs)],
    ["Avg", milliseconds(statistics.avgMs)],
    ["Max", milliseconds(statistics.maxMs)],
    ["Jitter", milliseconds(statistics.jitterMs)],
  ];
  return (
    <dl className="flex flex-wrap gap-x-5 gap-y-1 text-xs">
      {totals.map(([label, value]) => (
        <div key={label} className="flex gap-1.5">
          <dt className="text-muted-foreground">{label}</dt>
          <dd className={label === "Loss" && statistics.lossPercent > 0 ? "font-mono text-destructive" : "font-mono"}>
            {value}
          </dd>
        </div>
      ))}
    </dl>
  );
}

/** The last minute of round trips as a line; lost echoes are red bars, so drops stand out from slow replies. */
function LatencyGraph({ points }: { points: ContinuousPingRun["points"] }) {
  const width = 600;
  const height = 80;
  const step = width / (GRAPH_POINTS - 1);
  const highest = Math.max(5, ...points.map((point) => point.roundTripMs ?? 0)) * 1.2;
  const x = (index: number) => (GRAPH_POINTS - points.length + index) * step;
  const y = (ms: number) => height - (ms / highest) * height;

  const line = points
    .map((point, index) => (point.roundTripMs === null ? null : `${x(index)},${y(point.roundTripMs)}`))
    .filter(Boolean)
    .join(" ");

  return (
    <figure className="flex flex-col gap-1">
      <svg
        viewBox={`0 0 ${width} ${height}`}
        preserveAspectRatio="none"
        className="h-20 w-full rounded border bg-muted/20"
        role="img"
        aria-label="Round trip over the last minute"
      >
        {points.map((point, index) =>
          point.roundTripMs === null ? (
            <rect
              key={point.sequence}
              x={x(index) - step / 2}
              y={0}
              width={step}
              height={height}
              className="fill-destructive/40"
            />
          ) : null,
        )}
        <polyline points={line} fill="none" className="stroke-primary" strokeWidth={1.5} vectorEffect="non-scaling-stroke" />
      </svg>
      <figcaption className="text-[0.625rem] text-muted-foreground">
        Last {GRAPH_POINTS} echoes; up to {Math.round(highest)} ms. Red: lost.
      </figcaption>
    </figure>
  );
}
