# Ookla Speedtest is an optional provider, run from the Technician's own install

Speed Test can measure with Ookla's own `speedtest.exe` instead of Cloudflare (ADR 0003), when the Technician has installed it. ISPs recognise Ookla numbers and it can pick a server inside the Technician's ISP.

Ookla's licence grants a limited, non-commercial licence to use the program "through a command line interface for your personal, non-commercial use on a single personal computer", and its terms bar businesses and organisations. So:

- The app never bundles or downloads the program and never passes `--accept-license` or `--accept-gdpr`: the Technician installs it and accepts Ookla's terms themselves, once, in a terminal. Without that the run fails with Ookla's own message and a hint.
- Cloudflare stays the default; Ookla is chosen per Run and only offered when an Ookla `speedtest.exe` is found (a Python `speedtest.exe` of the same name is rejected by its `--version` output).
- The screen says Ookla allows personal use only and uploads each result to Ookla. Whether IT work on a company PC counts as personal use is the Technician's call, not the app's.
- The program runs as a subprocess with `-f jsonl`; its lines become the same Result, ratings and Copy as text as a Cloudflare Run. Ookla does not let the app force an IP version, so the IPv4/IPv6 toggle is hidden for it.
