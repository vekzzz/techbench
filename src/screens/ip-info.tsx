import { useEffect } from "react";

import { CopyResult } from "@/components/copy-result";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { AdapterInfo, AdapterKind } from "@/lib/toolbox";
import { useIpInfo } from "@/store";

export function IpInfo() {
  const { run, start, stop } = useIpInfo();
  const running = run !== null && run.result === null;

  // Read everything the first time IP Info opens; afterwards only Refresh re-reads.
  useEffect(() => {
    if (useIpInfo.getState().run === null) void start();
  }, [start]);

  return (
    <div className="flex max-w-3xl flex-col gap-6">
      <div className="flex items-center gap-2">
        <Button variant={running ? "outline" : "default"} onClick={() => (running ? stop() : void start())}>
          {running ? "Stop" : "Refresh"}
        </Button>
        {run?.result && <CopyResult text={run.result.text} />}
      </div>

      {run && (
        <>
          <p className="text-xs text-muted-foreground">
            Public address:{" "}
            {run.public ? (
              <span className={run.public.failed ? "text-destructive" : "font-mono text-foreground"}>
                {run.public.text}
              </span>
            ) : (
              "looking up…"
            )}
          </p>
          {run.adapters === null ? (
            <p className="text-xs text-muted-foreground">Reading adapters…</p>
          ) : (
            <div className="flex flex-col gap-3">
              {run.adapters.map((adapter) => (
                <AdapterCard
                  key={adapter.name}
                  adapter={adapter}
                  carriesDefaultRoute={adapter.name === run.defaultRoute}
                />
              ))}
            </div>
          )}
        </>
      )}
    </div>
  );
}

const KINDS: Record<AdapterKind, string> = {
  ethernet: "Ethernet",
  wireless: "Wireless",
  loopback: "Loopback",
  tunnel: "Tunnel",
  other: "Other",
};

/** One adapter; inactive adapters start collapsed. */
function AdapterCard({ adapter, carriesDefaultRoute }: { adapter: AdapterInfo; carriesDefaultRoute: boolean }) {
  return (
    <details open={adapter.up} className="group rounded-md border">
      <summary className="flex cursor-pointer list-none items-center gap-2 px-3 py-2 text-sm">
        <span className="font-medium">{adapter.name}</span>
        <Badge variant={adapter.up ? "default" : "outline"}>{adapter.up ? "Up" : "Down"}</Badge>
        {carriesDefaultRoute && <Badge variant="secondary">Default route</Badge>}
        <span className="ml-auto text-xs text-muted-foreground">{KINDS[adapter.kind]}</span>
      </summary>
      <dl className="grid grid-cols-[max-content_1fr] gap-x-4 gap-y-1 border-t px-3 py-2 text-xs">
        <Field label="Description" value={adapter.description} />
        <Field label="MAC" value={adapter.mac} mono />
        <Field label="IPv4" value={adapter.ipv4.join(", ")} mono />
        <Field label="IPv6" value={adapter.ipv6.join(", ")} mono />
        <Field label="Gateway" value={adapter.gateways.join(", ")} mono />
        <Field label="DNS servers" value={adapter.dnsServers.join(", ")} mono />
        <Field
          label="DHCP"
          value={adapter.dhcp ? (adapter.dhcp.server ? `On, server ${adapter.dhcp.server}` : "On") : "Off"}
        />
        {adapter.dhcp?.obtained && <Field label="Lease obtained" value={adapter.dhcp.obtained} mono />}
        {adapter.dhcp?.expires && <Field label="Lease expires" value={adapter.dhcp.expires} mono />}
      </dl>
    </details>
  );
}

/** A labelled detail; left out when the adapter doesn't have it. */
function Field({ label, value, mono = false }: { label: string; value: string | null; mono?: boolean }) {
  if (!value) return null;
  return (
    <>
      <dt className="text-muted-foreground">{label}</dt>
      <dd className={mono ? "font-mono break-all select-all" : ""}>{value}</dd>
    </>
  );
}
