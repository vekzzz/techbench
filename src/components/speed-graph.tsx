import type { LoadedProbe, SpeedSample } from "@/lib/toolbox";

/** Seconds each direction transfers, and how many of its first seconds the speed ignores. */
const PHASE_SECONDS = 10;
const WARM_UP_SECONDS = 2;
const WIDTH = 600;
const HEIGHT = 120;

/**
 * The Run so far on one timeline, download then upload: speed in Mbps as the solid line, latency
 * under load as the red line (each on its own scale), idle latency as the dashed line. The grey
 * bands are the warm-up seconds, which the final speed does not count.
 */
export function SpeedGraph({
  samples,
  loadedProbes,
  idleLatencyMs,
}: {
  samples: SpeedSample[];
  loadedProbes: LoadedProbe[];
  idleLatencyMs: number | null;
}) {
  const offset = (phase: string) => (phase === "upload" ? PHASE_SECONDS : 0);
  const x = (seconds: number) => (seconds / (PHASE_SECONDS * 2)) * WIDTH;
  const fastest = Math.max(1, ...samples.map((sample) => sample.mbps)) * 1.15;
  const slowest = Math.max(10, idleLatencyMs ?? 0, ...loadedProbes.map((probe) => probe.latencyMs)) * 1.15;
  const ySpeed = (mbps: number) => HEIGHT - (mbps / fastest) * HEIGHT;
  const yLatency = (ms: number) => HEIGHT - (ms / slowest) * HEIGHT;

  const speedLine = samples
    .map((sample) => `${x(offset(sample.phase) + sample.atSeconds)},${ySpeed(sample.mbps)}`)
    .join(" ");
  const latencyLine = loadedProbes
    .map((probe) => `${x(offset(probe.direction) + probe.atSeconds)},${yLatency(probe.latencyMs)}`)
    .join(" ");

  return (
    <figure className="flex flex-col gap-1">
      <svg
        viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
        preserveAspectRatio="none"
        className="h-36 w-full rounded-md border bg-card"
        role="img"
        aria-label="Speed and latency under load, download then upload"
      >
        {[0, PHASE_SECONDS].map((start) => (
          <rect key={start} x={x(start)} y={0} width={x(WARM_UP_SECONDS)} height={HEIGHT} className="fill-muted" />
        ))}
        <line x1={x(PHASE_SECONDS)} x2={x(PHASE_SECONDS)} y1={0} y2={HEIGHT} className="stroke-border" strokeWidth={1} />
        {idleLatencyMs !== null && (
          <line
            x1={0}
            x2={WIDTH}
            y1={yLatency(idleLatencyMs)}
            y2={yLatency(idleLatencyMs)}
            className="stroke-muted-foreground"
            strokeDasharray="4 4"
            strokeWidth={1}
            vectorEffect="non-scaling-stroke"
          />
        )}
        {speedLine && (
          <polyline points={speedLine} fill="none" className="stroke-chart-1" strokeWidth={1.5} vectorEffect="non-scaling-stroke" />
        )}
        {latencyLine && (
          <polyline points={latencyLine} fill="none" className="stroke-chart-2" strokeWidth={1.5} vectorEffect="non-scaling-stroke" />
        )}
      </svg>
      <figcaption className="flex justify-between gap-2 text-[0.625rem] text-muted-foreground">
        <span>Download</span>
        <span className="text-center">
          Solid: speed, up to {Math.round(fastest)} Mbps · Red: latency under load, up to {Math.round(slowest)} ms · Dashed: idle latency
        </span>
        <span>Upload</span>
      </figcaption>
    </figure>
  );
}
