import { useEffect, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { calculateSubnet, subnetContains, type SubnetReport } from "@/lib/toolbox";
import { useAppStore } from "@/store";

type CheckOutcome = { inside: boolean } | { error: string };

export function SubnetCalculator() {
  const { input, check } = useAppStore((state) => state.subnetCalculator);
  const update = useAppStore((state) => state.updateSubnetCalculator);

  const [report, setReport] = useState<SubnetReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [checkOutcome, setCheckOutcome] = useState<CheckOutcome | null>(null);

  // Recalculate as the Technician types. Only the latest answer is shown.
  useEffect(() => {
    if (!input.trim()) {
      setReport(null);
      setError(null);
      return;
    }
    let latest = true;
    calculateSubnet(input).then(
      (next) => latest && (setReport(next), setError(null)),
      (reason) => latest && (setReport(null), setError(String(reason))),
    );
    return () => {
      latest = false;
    };
  }, [input]);

  useEffect(() => {
    if (!report || !check.trim()) {
      setCheckOutcome(null);
      return;
    }
    let latest = true;
    subnetContains(input, check).then(
      (inside) => latest && setCheckOutcome({ inside }),
      (reason) => latest && setCheckOutcome({ error: String(reason) }),
    );
    return () => {
      latest = false;
    };
  }, [input, check, report]);

  return (
    <div className="flex max-w-2xl flex-col gap-6">
      <div className="flex flex-col gap-2">
        <Label htmlFor="subnet-input">Address and prefix</Label>
        <Input
          id="subnet-input"
          autoFocus
          spellCheck={false}
          className="font-mono"
          placeholder="192.168.1.10/24 · 192.168.1.10 255.255.255.0 · 2001:db8::1/64"
          value={input}
          aria-invalid={error !== null}
          onChange={(event) => update({ input: event.target.value })}
        />
        {error && <p className="text-xs text-destructive">{error}</p>}
      </div>

      {report && <ReportTable report={report} />}

      {report && (
        <div className="flex flex-col gap-2">
          <Label htmlFor="subnet-check">Is this address in the subnet?</Label>
          <div className="flex items-center gap-3">
            <Input
              id="subnet-check"
              spellCheck={false}
              className="font-mono"
              placeholder={report.network}
              value={check}
              onChange={(event) => update({ check: event.target.value })}
            />
            {checkOutcome && "inside" in checkOutcome && (
              <Badge variant={checkOutcome.inside ? "default" : "secondary"}>
                {checkOutcome.inside ? "Inside" : "Outside"}
              </Badge>
            )}
          </div>
          {checkOutcome && "error" in checkOutcome && (
            <p className="text-xs text-destructive">{checkOutcome.error}</p>
          )}
        </div>
      )}
    </div>
  );
}

function ReportTable({ report }: { report: SubnetReport }) {
  const isIpv6 = report.mask === null;
  const rows: [string, string][] = isIpv6
    ? [
        ["Network", `${report.network}/${report.prefix}`],
        ["First address", report.firstUsable],
        ["Last address", report.lastUsable],
        ["Address count", report.usableCount],
      ]
    : [
        ["Network", `${report.network}/${report.prefix}`],
        ["Broadcast", report.broadcast ?? "none"],
        ["First usable", report.firstUsable],
        ["Last usable", report.lastUsable],
        ["Usable hosts", report.usableCount],
        ["Mask", report.mask ?? ""],
        ["Wildcard", report.wildcard ?? ""],
      ];

  return (
    <dl className="grid grid-cols-[max-content_1fr] overflow-hidden rounded-md border text-xs">
      {rows.map(([label, value]) => (
        <div key={label} className="contents [&>*]:border-b [&:last-child>*]:border-b-0">
          <dt className="bg-muted/40 px-3 py-1.5 text-muted-foreground">{label}</dt>
          <dd className="px-3 py-1.5 font-mono select-all">{value}</dd>
        </div>
      ))}
    </dl>
  );
}
