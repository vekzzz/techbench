import { useEffect, useState } from "react";

import { ActionMenu } from "@/components/action-menu";
import { CopyResult } from "@/components/copy-result";
import { FamilyToggle } from "@/components/family-toggle";
import { SpeedGraph } from "@/components/speed-graph";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { dataSize } from "@/lib/format";
import { ooklaInstalled, speedTestCostWarning, type DirectionResult, type SpeedDirection, type SpeedTestResult } from "@/lib/toolbox";
import { useSpeedTest, type SpeedTestRun } from "@/store";

/** A Run that ended without finishing for a reason other than the Technician stopping it. */
function isFailure(result: SpeedTestResult) {
  return (
    result.ending === "noInternet" ||
    result.ending === "rateLimited" ||
    result.ending === "interrupted" ||
    result.ending === "unavailable"
  );
}

const PHASES = { latency: "Measuring latency…", download: "Downloading…", upload: "Uploading…" } as const;

export function SpeedTest() {
  const { input, error, run, update, start, stop } = useSpeedTest();
  const running = run !== null && run.result === null;
  /** Why a Run needs confirming (a metered connection), while the Technician decides. */
  const [warning, setWarning] = useState<string | null>(null);
  /** Whether Ookla's own speedtest.exe is installed; null while still looking. */
  const [ookla, setOokla] = useState<boolean | null>(null);
  useEffect(() => {
    void ooklaInstalled().then(setOokla, () => setOokla(false));
  }, []);

  const begin = async () => {
    const reason = await speedTestCostWarning().catch(() => null);
    if (reason) setWarning(reason);
    else void start();
  };

  return (
    <div className="flex max-w-3xl flex-col gap-6">
      <form
        className="flex flex-col gap-4"
        onSubmit={(event) => {
          event.preventDefault();
          if (running) stop();
          else void begin();
        }}
      >
        <div className="flex items-center gap-2">
          <Button type="submit" variant={running ? "outline" : "default"}>
            {running ? "Stop" : "Start"}
          </Button>
          {run && <CopyResult text={run.result?.text} runId={run.id} />}
          <div className="ml-auto flex items-center gap-2">
            <ProviderToggle provider={input.provider} onChange={(provider) => update({ provider })} />
            {input.provider === "cloudflare" && <FamilyToggle family={input.family} onChange={(family) => update({ family })} />}
          </div>
        </div>

        <div className="flex items-end gap-3 text-xs">
          <span className="pb-2 text-muted-foreground">ISP plan (optional)</span>
          <PlanField
            id="plan-download"
            label="Download Mbps"
            value={input.planDownload}
            onChange={(planDownload) => update({ planDownload })}
          />
          <PlanField
            id="plan-upload"
            label="Upload Mbps"
            value={input.planUpload}
            onChange={(planUpload) => update({ planUpload })}
          />
        </div>
        {error && <p className="text-xs text-destructive">{error}</p>}
        {input.provider === "ookla" && (
          <p className="text-xs text-muted-foreground">
            Runs Ookla&apos;s own speedtest.exe, which you installed and whose licence you accepted. Ookla allows it for
            personal, non-commercial use only, and uploads each result to Ookla.
          </p>
        )}
        {input.provider === "ookla" && ookla === false && (
          <p role="status" className="rounded-md bg-warning-soft px-3 py-2 text-xs text-warning">
            Ookla&apos;s speedtest.exe was not found. Install it with{" "}
            <span className="font-mono">winget install Ookla.Speedtest.CLI</span>, run{" "}
            <span className="font-mono">speedtest</span> once in a terminal to accept its licence, then reopen this app.
          </p>
        )}
      </form>

      {warning && (
        <div role="alertdialog" className="flex flex-col gap-3 rounded-md bg-warning-soft px-4 py-3 text-sm text-warning">
          <p>{warning}</p>
          <div className="flex gap-2">
            <Button
              size="sm"
              onClick={() => {
                setWarning(null);
                void start();
              }}
            >
              Run anyway
            </Button>
            <Button size="sm" variant="outline" onClick={() => setWarning(null)}>
              Cancel
            </Button>
          </div>
        </div>
      )}

      {run ? <RunView run={run} /> : <EmptyState />}
    </div>
  );
}

/** What a Run measures, shown before the first one so the empty screen explains itself. */
function EmptyState() {
  const rows = [
    ["Latency and jitter", "How quickly the server answers while the line is idle, and how much that varies."],
    ["Download and upload", "Four connections for 10 s each way; the first 2 s are not counted."],
    ["Latency under load", "The same probe while the line is full. A big increase is what breaks calls and games."],
    ["Ratings", "Web browsing, streaming, gaming and video calls, from the figures above."],
  ];
  return (
    <section className="flex max-w-xl flex-col gap-4 pt-4">
      <h2 className="text-balance text-lg font-semibold tracking-tight">Measure this PC&apos;s internet line</h2>
      <dl className="flex flex-col divide-y rounded-md border bg-card text-sm">
        {rows.map(([term, detail]) => (
          <div key={term} className="grid grid-cols-[10rem_1fr] gap-4 px-4 py-3">
            <dt className="font-medium">{term}</dt>
            <dd className="text-muted-foreground">{detail}</dd>
          </div>
        ))}
      </dl>
      <p className="text-xs text-muted-foreground">A Run moves roughly 0.25 GB on a 100 Mbps line.</p>
    </section>
  );
}

function ProviderToggle({
  provider,
  onChange,
}: {
  provider: "cloudflare" | "ookla";
  onChange: (provider: "cloudflare" | "ookla") => void;
}) {
  const options = [
    { value: "cloudflare", label: "Cloudflare" },
    { value: "ookla", label: "Ookla" },
  ] as const;
  return (
    <div role="radiogroup" aria-label="Speed test provider" className="flex shrink-0 rounded-md border p-0.5">
      {options.map((option) => (
        <Button
          key={option.value}
          type="button"
          size="xs"
          role="radio"
          aria-checked={provider === option.value}
          variant={provider === option.value ? "secondary" : "ghost"}
          onClick={() => onChange(option.value)}
        >
          {option.label}
        </Button>
      ))}
    </div>
  );
}

function PlanField({
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
    <div className="flex flex-col gap-1">
      <Label htmlFor={id} className="text-xs text-muted-foreground">
        {label}
      </Label>
      <Input
        id={id}
        inputMode="decimal"
        autoComplete="off"
        placeholder="e.g. 200"
        className="h-8 w-28 font-mono"
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
    </div>
  );
}

function RunView({ run }: { run: SpeedTestRun }) {
  const result = run.result;
  const live = run.samples.filter((sample) => sample.phase === run.phase);
  const liveMbps = live.length > 0 ? live[live.length - 1].mbps : null;

  return (
    <div className="flex flex-col gap-4">
      {!result && (
        <div className="flex flex-col gap-1">
          <span className="font-mono text-4xl font-medium tracking-tight">{liveMbps === null ? "—" : liveMbps.toFixed(1)}</span>
          <span className="text-xs text-muted-foreground">
            Mbps {run.phase === "upload" ? "upload" : "download"}
            {run.phase && <> · {PHASES[run.phase]}</>}
            {(run.phase === "download" || run.phase === "upload") && <> (live, warm-up included)</>}
          </span>
        </div>
      )}

      {result && isFailure(result) && (
        <p role="alert" className="rounded-md bg-danger-soft px-4 py-3 text-sm text-danger">
          {result.summary}
        </p>
      )}

      {result && (
        <div className="grid grid-cols-2 gap-4">
          <BigSpeed label="download" direction={result.download} />
          <BigSpeed label="upload" direction={result.upload} />
        </div>
      )}

      <ServerLine run={run} />

      {(run.samples.length > 0 || run.loadedProbes.length > 0) && (
        <SpeedGraph samples={run.samples} loadedProbes={run.loadedProbes} idleLatencyMs={run.latency?.medianMs ?? null} />
      )}

      {result && <ResultView result={result} />}
    </div>
  );
}

function BigSpeed({ label, direction }: { label: string; direction: DirectionResult }) {
  return (
    <div className="flex flex-col gap-1">
      {direction.mbps === null ? (
        <span className="py-2 text-sm text-muted-foreground">not measured</span>
      ) : (
        <span className="font-mono text-4xl font-medium tracking-tight">{direction.mbps.toFixed(1)}</span>
      )}
      <span className="text-xs text-muted-foreground">
        Mbps {label}
        {direction.planPercent !== null && <> · {direction.planPercent}% of plan</>}
      </span>
    </div>
  );
}

function ServerLine({ run }: { run: SpeedTestRun }) {
  const server = run.result?.server ?? run.server;
  if (!server) return null;
  return (
    <p className="flex items-center gap-2 text-xs text-muted-foreground">
      Server:{" "}
      <span className="font-mono text-foreground">
        {server.provider} {server.location}
        {server.address && ` (${server.address})`}
      </span>
      {server.address && <ActionMenu address={server.address} only={["ping", "continuous-ping", "traceroute"]} />}
    </p>
  );
}

const VERDICT_CLASS = { Low: "text-success", Moderate: "text-warning", High: "text-danger" } as const;

function UnderLoad({ direction, name }: { direction: DirectionResult; name: SpeedDirection }) {
  if (direction.loadedLatencyMs === null || direction.increaseMs === null || direction.verdict === null) {
    return <span className="text-muted-foreground">not measured</span>;
  }
  return (
    <span className="font-mono" aria-label={`Latency while ${name === "download" ? "downloading" : "uploading"}`}>
      {direction.loadedLatencyMs.toFixed(1)} ms (+{direction.increaseMs.toFixed(1)} ms,{" "}
      <span className={VERDICT_CLASS[direction.verdict]}>{direction.verdict}</span>)
    </span>
  );
}

function Stars({ count }: { count: number | null }) {
  if (count === null) return <span className="text-muted-foreground">not rated</span>;
  return (
    <span className="font-mono text-sm tracking-widest" aria-label={`${count} of 5 stars`}>
      <span className="text-warning">{"★".repeat(count)}</span>
      <span className="text-muted-foreground/30">{"★".repeat(5 - count)}</span>
    </span>
  );
}

function ResultView({ result }: { result: SpeedTestResult }) {
  const rated = result.ratings.some((rating) => rating.stars !== null);
  return (
    <div className="flex flex-col gap-4">
      <dl className="grid grid-cols-[max-content_1fr] gap-x-4 gap-y-1 rounded-md border px-3 py-2 text-xs">
        <dt className="text-muted-foreground">PC</dt>
        <dd className="font-mono">{result.pc}</dd>
        <dt className="text-muted-foreground">Public</dt>
        <dd className={result.publicFailed ? "text-destructive" : "font-mono"}>{result.public}</dd>
        <dt className="text-muted-foreground">Idle latency</dt>
        <dd className="font-mono">{result.latencyMedianMs === null ? "not measured" : `${result.latencyMedianMs.toFixed(1)} ms`}</dd>
        <dt className="text-muted-foreground">Jitter</dt>
        <dd className="font-mono">{result.jitterMs === null ? "not measured" : `${result.jitterMs.toFixed(1)} ms`}</dd>
        <dt className="text-muted-foreground">Latency while downloading</dt>
        <dd>
          <UnderLoad direction={result.download} name="download" />
        </dd>
        <dt className="text-muted-foreground">Latency while uploading</dt>
        <dd>
          <UnderLoad direction={result.upload} name="upload" />
        </dd>
        <dt className="text-muted-foreground">Data used</dt>
        <dd className="font-mono">{dataSize(result.dataUsedBytes)}</dd>
      </dl>

      {rated && (
        <div className="flex flex-col gap-1 rounded-md border px-3 py-2 text-xs">
          <span className="text-muted-foreground">How well this connection handles</span>
          <dl className="grid grid-cols-[max-content_1fr] items-center gap-x-4 gap-y-1">
            {result.ratings.map((rating) => (
              <div key={rating.useCase} className="contents">
                <dt>{rating.useCase}</dt>
                <dd>
                  <Stars count={rating.stars} />
                </dd>
              </div>
            ))}
          </dl>
        </div>
      )}

      {!isFailure(result) && <p className="text-xs text-muted-foreground">{result.summary}</p>}
    </div>
  );
}
