import { useEffect, useState } from "react";

import { CopyResult } from "@/components/copy-result";
import { TargetInput } from "@/components/target-input";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { milliseconds } from "@/lib/format";
import { DNS_RECORD_TYPES, defaultDnsServer } from "@/lib/toolbox";
import { useDnsLookup } from "@/store";

/** Whether the text is an IPv4 address (four numbers) or an IPv6 one (hex groups with colons), the way the toolbox core decides. */
function isIpAddress(text: string) {
  const unbracketed = text.replace(/^\[(.*)\]$/, "$1");
  const ipv4 = /^(\d{1,3})(\.\d{1,3}){3}$/.test(unbracketed);
  const ipv6 = unbracketed.includes(":") && /^[\da-f:.]+$/i.test(unbracketed);
  return ipv4 || ipv6;
}

/** Servers worth comparing against this PC's own: well-known public resolvers. */
const QUICK_SERVERS = ["8.8.8.8", "1.1.1.1"];

export function DnsLookup() {
  const { input, error, run, update, start, stop } = useDnsLookup();
  const running = run !== null && run.result === null;
  const [pcServer, setPcServer] = useState<string | null>(null);

  // Show which server an empty Server field means.
  useEffect(() => {
    void defaultDnsServer().then(setPcServer);
  }, []);

  const result = run?.result;
  // An IP address is always looked up as PTR, whatever type is chosen.
  const looksLikeAnAddress = isIpAddress(input.query.trim());

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
          <Label htmlFor="dns-query">Name or IP address</Label>
          <TargetInput
            id="dns-query"
            autoFocus
            placeholder="example.com   or   8.8.8.8 for its name"
            value={input.query}
            onChange={(query) => update({ query })}
          />
        </div>

        <div className="flex flex-col gap-2">
          <Label>Record type</Label>
          <div role="radiogroup" aria-label="Record type" className="flex w-fit flex-wrap gap-1">
            {DNS_RECORD_TYPES.map((type) => {
              const chosen = looksLikeAnAddress ? type === "PTR" : input.recordType === type;
              return (
                <Button
                  key={type}
                  type="button"
                  size="xs"
                  role="radio"
                  aria-checked={chosen}
                  disabled={looksLikeAnAddress}
                  variant={chosen ? "secondary" : "outline"}
                  onClick={() => update({ recordType: type })}
                >
                  {type}
                </Button>
              );
            })}
          </div>
          {looksLikeAnAddress && <p className="text-xs text-muted-foreground">An IP address is looked up as PTR.</p>}
        </div>

        <div className="flex flex-col gap-2">
          <Label htmlFor="dns-server">DNS server (address or name)</Label>
          <Input
            id="dns-server"
            spellCheck={false}
            autoComplete="off"
            className="font-mono"
            placeholder={pcServer ? `This PC's: ${pcServer}` : "This PC's own DNS server"}
            value={input.server}
            onChange={(event) => update({ server: event.target.value })}
          />
          <div className="flex flex-wrap items-center gap-1">
            <span className="mr-1 text-xs text-muted-foreground">Ask</span>
            <Button type="button" size="xs" variant="outline" onClick={() => update({ server: "" })}>
              This PC's server
            </Button>
            {QUICK_SERVERS.map((server) => (
              <Button key={server} type="button" size="xs" variant="outline" onClick={() => update({ server })}>
                {server}
              </Button>
            ))}
          </div>
        </div>

        {error && <p className="text-xs text-destructive">{error}</p>}
        <div className="flex items-center gap-2">
          <Button type="submit" variant={running ? "outline" : "default"}>
            {running ? "Stop" : "Look up"}
          </Button>
          {result && <CopyResult text={result.text} />}
        </div>
      </form>

      {running && <p className="text-xs text-muted-foreground">Asking…</p>}
      {result && (
        <div className="flex flex-col gap-3">
          <p className="text-xs text-muted-foreground">
            {result.server && (
              <>
                Server <span className="font-mono text-foreground">{result.server}</span>
                {result.answeredMs !== null && <> answered in {milliseconds(result.answeredMs)}</>}
              </>
            )}
            {result.showAsked && (
              <>
                {" "}
                · asked <span className="font-mono text-foreground">{result.asked}</span>
              </>
            )}
          </p>
          {result.records.length > 0 && (
            <table className="w-full overflow-hidden rounded-md border text-xs">
              <thead className="bg-muted/40 text-left text-muted-foreground">
                <tr>
                  <th className="px-3 py-1.5 font-normal">Name</th>
                  <th className="px-3 py-1.5 font-normal">Type</th>
                  <th className="px-3 py-1.5 font-normal">TTL</th>
                  <th className="px-3 py-1.5 font-normal">Value</th>
                </tr>
              </thead>
              <tbody>
                {result.records.map((record, index) => (
                  <tr key={index} className="border-t">
                    <td className="px-3 py-1.5 font-mono">{record.name}</td>
                    <td className="px-3 py-1.5 font-mono">{record.recordType}</td>
                    <td className="px-3 py-1.5 font-mono">{record.ttl}</td>
                    <td className="px-3 py-1.5 font-mono break-all select-all">{record.value}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          <p className={result.failed ? "text-xs text-destructive" : "text-xs text-muted-foreground"}>
            {result.summary}
          </p>
        </div>
      )}
    </div>
  );
}
