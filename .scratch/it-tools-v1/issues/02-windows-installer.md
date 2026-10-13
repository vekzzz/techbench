# 02 — Windows installer

**What to build:** The Technician installs IT Tools on a Windows PC from a single unsigned NSIS installer produced by Tauri's bundler, and the installed app launches without a UAC prompt. Doing this early proves packaging works before the app grows, and every later ticket keeps it working.

Spec: user stories 5, 9; Implementation Decisions › Shape of the application.

**Blocked by:** 01 — Walking skeleton: Subnet Calculator end to end.

**Status:** ready-for-agent
**Category:** enhancement

- [ ] One documented build command produces an NSIS installer.
- [ ] The installer runs on a clean Windows 11 PC, handles the WebView2 runtime, and adds a Start menu entry.
- [ ] The installed app launches without a UAC prompt (no Administrator manifest).
- [ ] The installed app shows the sidebar and a working Subnet Calculator.
- [ ] Uninstalling removes the app.
