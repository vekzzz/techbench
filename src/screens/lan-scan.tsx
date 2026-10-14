import { useEffect, useState } from "react";

import { ActionMenu } from "@/components/action-menu";
import { CopyResult } from "@/components/copy-result";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { milliseconds } from "@/lib/format";
import { defaultLanRange, lanRangeInfo, type DeviceInfo } from "@/lib/toolbox";
import { useAppStore, useLanScan, type DeviceSort, type LanScanRun } from "@/store";

/** Whether the default range has been offered yet: it is offered once, so clearing the field sticks. */
let defaultOffered = false;

export function LanScan() {
  const { input, error, run, update, start, stop } = useLanScan();
  const running = run !== null && run.result === null;
  const [rangeError, setRangeError] = useState<string | null>(null);
  /** A big range waiting for the Technician's confirmation: how many addresses it has. */
  const [confirming, setConfirming] = useState<number | null>(null);
  const [noDefault, setNoDefault] = useState(false);

  // Offer the network of the adapter carrying the default route the first time the screen opens.
  useEffect(() => {
    if (defaultOffered) return;
    defaultOffered = true;
    void defaultLanRange().then((range) => {
      if (range === null) setNoDefault(true);
      else if (useLanScan.getState().input.range === "") update({ range });
    });
  }, [update]);

  const submit = async () => {
    if (running) return stop();
    setRangeError(null);
    try {
      const info = await lanRangeInfo(input.range);
      if (info.needsConfirmation && !input.confirmed) return setConfirming(info.addresses);
    } catch (reason) {
      return setRangeError(String(reason));
    }
    void start();
  };

  return (
    <div className="flex max-w-3xl flex-col gap-6">
      <form
        className="flex flex-col gap-4"
        onSubmit={(event) => {
          event.preventDefault();
          void submit();
        }}
      >
        <div className="flex flex-col gap-2">
          <Label htmlFor="lan-scan-range">Range</Label>
          <Input
            id="lan-scan-range"
            autoFocus
            spellCheck={false}
            autoComplete="off"
            className="font-mono"
            placeholder="192.168.1.0/24   or   192.168.1.10-192.168.1.50"
            value={input.range}
            onChange={(event) => {
              update({ range: event.target.value, confirmed: false });
              setConfirming(null);
              setRangeError(null);
            }}
          />
          {noDefault && input.range === "" && (
            <p className="text-xs text-muted-foreground">
              The adapter carrying the default route has no IPv4 address, so there is no range to offer: enter one.
            </p>
          )}
        </div>

        {confirming !== null && (
          <div className="flex flex-col gap-2 rounded-md border p-3">
            <p className="text-xs">
              This range has {confirming.toLocaleString("en-US")} addresses and will take a while to sweep. Scan it anyway?
            </p>
            <div className="flex gap-2">
              <Button
                type="button"
                size="sm"
                onClick={() => {
                  update({ confirmed: true });
                  setConfirming(null);
                  void start();
                }}
              >
                Scan {confirming.toLocaleString("en-US")} addresses
              </Button>
              <Button type="button" size="sm" variant="outline" onClick={() => setConfirming(null)}>
                Cancel
              </Button>
            </div>
          </div>
        )}

        {(rangeError ?? error) && <p className="text-xs text-destructive">{rangeError ?? error}</p>}
        <div className="flex items-center gap-2">
          <Button type="submit" variant={running ? "outline" : "default"}>
            {running ? "Stop" : "Scan"}
          </Button>
          {run && <CopyResult text={run.result?.text} runId={run.id} />}
        </div>
      </form>

      {run && <RunView run={run} />}
    </div>
  );
}

/** An IPv4 address as a number, for sorting Devices the way the Result lists them. */
function addressKey(address: string) {
  return address.split(".").reduce((key, part) => key * 256 + Number(part), 0);
}

const SORTS = [
  { id: "ip", label: "IP" },
  { id: "hostname", label: "Hostname" },
  { id: "vendor", label: "Vendor" },
] as const;

/** Devices matching the filter text (in any of IP, hostname, MAC or vendor), in the chosen order. Devices without the sorted-by detail come last. */
function visibleDevices(devices: DeviceInfo[], sort: DeviceSort, filter: string) {
  const needle = filter.trim().toLowerCase();
  const shown = devices.filter(
    (device) =>
      needle === "" ||
      [device.address, device.hostname, device.mac, device.vendor].some((field) => field?.toLowerCase().includes(needle)),
  );
  const byAddress = (a: DeviceInfo, b: DeviceInfo) => addressKey(a.address) - addressKey(b.address);
  const byText = (field: "hostname" | "vendor") => (a: DeviceInfo, b: DeviceInfo) => {
    const [left, right] = [a[field], b[field]];
    if (left === null || right === null) return left === right ? byAddress(a, b) : left === null ? 1 : -1;
    return left.localeCompare(right, undefined, { sensitivity: "base" }) || byAddress(a, b);
  };
  return shown.sort(sort === "ip" ? byAddress : byText(sort));
}

function RunView({ run }: { run: LanScanRun }) {
  const { sort, filter } = useAppStore((state) => state.lanScanView);
  const updateView = useAppStore((state) => state.updateLanScanView);
  const devices = visibleDevices(run.devices, sort, filter);
  const total = run.total ?? 0;

  return (
    <div className="flex flex-col gap-3">
      {!run.result && (
        <div className="flex flex-col gap-1">
          <div className="h-1.5 w-full overflow-hidden rounded-full bg-muted" role="progressbar" aria-valuenow={run.probed} aria-valuemax={total}>
            <div className="h-full bg-primary transition-[width]" style={{ width: total ? `${(run.probed / total) * 100}%` : "0%" }} />
          </div>
          <p className="text-xs text-muted-foreground">
            Scanning… {run.probed.toLocaleString("en-US")} of {total.toLocaleString("en-US")} addresses probed,{" "}
            {run.devices.length} Device{run.devices.length === 1 ? "" : "s"} found
          </p>
        </div>
      )}
      {run.devices.length > 0 && (
        <div className="flex flex-wrap items-center gap-3">
          <Input
            aria-label="Filter Devices"
            spellCheck={false}
            autoComplete="off"
            className="max-w-64"
            placeholder="Filter by IP, hostname, MAC or vendor"
            value={filter}
            onChange={(event) => updateView({ filter: event.target.value })}
          />
          <div role="radiogroup" aria-label="Sort Devices by" className="flex items-center gap-1">
            <span className="mr-1 text-xs text-muted-foreground">Sort by</span>
            {SORTS.map((option) => (
              <Button
                key={option.id}
                type="button"
                size="xs"
                role="radio"
                aria-checked={sort === option.id}
                variant={sort === option.id ? "secondary" : "outline"}
                onClick={() => updateView({ sort: option.id })}
              >
                {option.label}
              </Button>
            ))}
          </div>
          {devices.length !== run.devices.length && (
            <span className="text-xs text-muted-foreground">
              {devices.length} of {run.devices.length} shown
            </span>
          )}
        </div>
      )}
      {devices.length > 0 && <DeviceTable devices={devices} />}
      {run.devices.some((device) => !device.onOwnSubnet) && (
        <p className="text-xs text-muted-foreground">
          MAC and vendor are blank for Devices beyond this PC's own subnet: ARP does not cross routers.
        </p>
      )}
      {run.result && <p className="text-xs text-muted-foreground">{run.result.summary}</p>}
    </div>
  );
}

function DeviceTable({ devices }: { devices: DeviceInfo[] }) {
  return (
    <table className="w-full overflow-hidden rounded-md border text-xs">
      <thead className="bg-muted/40 text-left text-muted-foreground">
        <tr>
          <th className="px-3 py-1.5 font-normal">IP</th>
          <th className="px-3 py-1.5 font-normal">Hostname</th>
          <th className="px-3 py-1.5 font-normal">MAC</th>
          <th className="px-3 py-1.5 font-normal">Vendor</th>
          <th className="px-3 py-1.5 font-normal">Response time</th>
          <th className="px-3 py-1.5 font-normal" />
          <th className="px-3 py-1.5 font-normal" />
        </tr>
      </thead>
      <tbody>
        {devices.map((device) => (
          <tr key={device.address} className="border-t">
            <td className="px-3 py-1.5 font-mono">{device.address}</td>
            <td className="px-3 py-1.5 text-muted-foreground">{device.hostname ?? "-"}</td>
            <td className="px-3 py-1.5 font-mono">{device.mac ?? "-"}</td>
            <td className="px-3 py-1.5 text-muted-foreground">{device.vendor ?? "-"}</td>
            <td className="px-3 py-1.5 font-mono">{milliseconds(device.responseMs)}</td>
            <td className="px-3 py-1.5">
              {device.noPingReply && (
                <Badge variant="outline" title="It answered ARP but not the ping, typically a PC whose firewall drops echoes.">
                  no ping reply
                </Badge>
              )}
            </td>
            <td className="px-3 py-1.5 text-right">
              <ActionMenu address={device.address} hostname={device.hostname} saveable />
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
