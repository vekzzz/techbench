# Speed Test uses Cloudflare's public speed endpoints

Speed Test measures against `speed.cloudflare.com` (`/__down`, `/__up`) over plain HTTPS from the existing `ureq` client: no key, no install, and each response names the Cloudflare location that answered. This makes Speed Test the second Tool that calls out to the internet, so the v1 rule "IP Info's public lookup is the only external call" becomes "Tools whose subject is the internet may call out; all others stay offline."

Cloudflare publishes no terms for third-party clients, and its `robots.txt` disallows `/__down`, `/__up` and `/__results`. The endpoints are the ones its own MIT-licensed `@cloudflare/speedtest` library and its Rust CLI `mach` use, and a Technician starting a Run by hand is not a bot, but this is a grey area, not a licence. To stay on the right side of it:

- Every request stays under 100 MB. `speed.cloudflare.com` answers 403 above that unless `Origin`/`Referer` claim Cloudflare's own page; we never send that, and we do not use the `h3.speed` host to get round the limit.
- Results are never posted back (`/__results`, `aim.cloudflare.com/__log`).
- Requests carry a User-Agent naming Techbench.
- Packet loss is not measured: it needs a WebRTC TURN relay that is locked to Cloudflare's own page.
- A 429 is shown to the Technician with Cloudflare's `Retry-After`; there are no automatic retries.

The provider sits behind the network adapter seam, so replacing it later is a change in one place.

## Considered Options

- **Ookla Speedtest CLI** — the name ISPs trust and can pick a server inside the ISP, but its EULA grants personal, non-commercial use only, bars businesses, forbids bundling `speedtest.exe` in our installer and forbids reverse-engineering a native client. Off the table for IT work on client networks. May return later as "use `speedtest.exe` if the Technician installed it".
- **M-Lab NDT7** — open protocol and a nearby server, but it publishes the client's IP address and results publicly and permanently, and access is moving to required API keys.
- **LibreSpeed public servers** — no written terms, but the nearest server to this Technician is in Tokyo; the Rust CLI is LGPL-3.0.
- **fast.com** — no official API; relies on a scraped token that can break at any time.
