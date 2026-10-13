# IT Tools

A Windows toolbox of network diagnostics for an IT Technician. Every Tool runs locally on the Technician's own PC, without Administrator rights.

The domain language is in [CONTEXT.md](CONTEXT.md), architectural decisions are in [docs/adr/](docs/adr/), and the v1 spec and tickets are in [.scratch/it-tools-v1/](.scratch/it-tools-v1/).

## Prerequisites (to build)

- Windows 10 or 11
- [Node.js](https://nodejs.org/) 20 or later
- [Rust](https://rustup.rs/) with the `x86_64-pc-windows-msvc` toolchain
- Visual Studio Build Tools 2022 with the **Desktop development with C++** workload:

  ```powershell
  winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
  ```

Then install the JavaScript dependencies once:

```powershell
npm install
```

## Commands

| Command | What it does |
| --- | --- |
| `npm run tauri dev` | Runs the app with hot reload. |
| `npm test` | Runs every Rust test in the workspace. |
| `npm run package` | Builds the Windows installer. |

## Building the installer

```powershell
npm run package
```

This produces an unsigned NSIS installer at:

```
target\release\bundle\nsis\IT Tools_<version>_x64-setup.exe
```

The first run downloads Tauri's NSIS toolchain, so it needs internet access.

## Installing

Run the `-setup.exe`. Because the installer is unsigned, Windows SmartScreen shows a warning the first time: choose **More info → Run anyway**.

- It installs into the signed-in Windows account only, under `%LOCALAPPDATA%\IT Tools`, so it needs no Administrator rights and shows no UAC prompt.
- It adds **IT Tools** to the Start menu.
- If the PC lacks the WebView2 runtime, the installer downloads it silently. Windows 11 already includes it.

To uninstall, use **Settings → Apps → Installed apps → IT Tools → Uninstall**.

## Project layout

| Path | Contents |
| --- | --- |
| `crates/toolbox-core/` | Every Tool's behaviour, with no dependency on Tauri or the UI. Tests live here. |
| `src-tauri/` | The Tauri shell, which exposes the toolbox core to the UI. |
| `src/` | The React + TypeScript UI. |
