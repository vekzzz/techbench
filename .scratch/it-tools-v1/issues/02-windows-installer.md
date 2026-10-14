# 02 — Windows installer

**What to build:** The Technician installs IT Tools on a Windows PC from a single unsigned NSIS installer produced by Tauri's bundler, and the installed app launches without a UAC prompt. Doing this early proves packaging works before the app grows, and every later ticket keeps it working.

Spec: user stories 5, 9; Implementation Decisions › Shape of the application.

**Blocked by:** 01 — Walking skeleton: Subnet Calculator end to end.

**Status:** ready-for-agent
**Category:** enhancement

- [x] One documented build command produces an NSIS installer.
- [ ] The installer runs on a clean Windows 11 PC, handles the WebView2 runtime, and adds a Start menu entry.
- [x] The installed app launches without a UAC prompt (no Administrator manifest).
- [x] The installed app shows the sidebar and a working Subnet Calculator.
- [x] Uninstalling removes the app.

## Comments

**2026-10-13 — implementation pass (Claude).** `npm run package` (documented in the README) builds `target\release\bundle\nsis\IT Tools_0.1.0_x64-setup.exe`, 1.71 MiB. It installs per user (`currentUser` mode), uses the WebView2 download bootstrapper, and statically links the Visual C++ runtime. The app now embeds an explicit `asInvoker` manifest; before this, it declared no execution level at all. Verified on this PC:
- Both the installer and the app manifests request `asInvoker`.
- A silent install (`/S`) creates `%LOCALAPPDATA%\IT Tools`, the Start menu entry `IT Tools.lnk` (originally inside an `IT Tools` folder; see follow-up), and an HKCU uninstall entry.
- The installed app launched without UAC from a non-elevated shell, and the Subnet Calculator calculated correctly.
- A silent uninstall removed the folder, the shortcut and the registry entry.

Not verified: a truly clean Windows 11 PC, because this dev PC already has WebView2.

**2026-10-13 — review follow-up (Claude).** I dropped `startMenuFolder`: setting it added a Start-menu wizard page, which offered "Do not create shortcuts". I also dropped `category`, which does nothing for NSIS. After rebuilding, I reran the silent install, launch and uninstall checks; all passed. The interactive wizard itself has not been clicked through yet.

**2026-10-14 — still open (Claude).** "Runs on a clean Windows 11 PC" can't be checked from this machine: it already has WebView2, and Windows 11 Home has no Windows Sandbox to give a clean one. Needed: a PC or VM without WebView2. Run the installer from `npm run package` (now 2.44 MiB), then confirm it fetches WebView2, adds the Start menu entry, and opens the app.

