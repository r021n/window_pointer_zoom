# Window Pointer Zoom

A lightweight Windows desktop utility for smooth **full-screen magnification** that dynamically pans to follow your mouse pointer using the native Windows Magnification API (`MagSetFullscreenTransform` / `MagSetInputTransform`).

Built with Tauri 2, React + TypeScript, and Rust.

## Requirements

- **OS:** Windows 10 (version 1703 or later) or Windows 11, 64-bit (`x64` / `arm64`)
- **Runtime:** Microsoft Edge WebView2 Runtime (included on Windows 11 and most Windows 10 systems; installed automatically by the NSIS/MSI installer if missing)

## Building from Source

This repository does not ship prebuilt installers. Build the app locally with the steps below.

### Prerequisites

- Windows 10 (version 1703 or later) or Windows 11, 64-bit
- Node.js 20+ and npm
- Rust with the MSVC toolchain (`x86_64-pc-windows-msvc` or `aarch64-pc-windows-msvc`)
- Visual Studio Build Tools with the **Desktop development with C++** workload
- Microsoft Edge WebView2 Runtime (required at runtime)
- [WiX Toolset v3](https://github.com/wixtoolset/wix3) on `PATH` — only needed to build the `.msi` bundle

```powershell
npm install

# Development mode (unsigned, UIAccess disabled)
npm run tauri dev

# Standalone executable, no installers
npm run tauri build -- --no-bundle
# -> src-tauri\target\release\window-pointer-zoom.exe

# Installers, unsigned unless you supply a signing certificate
npm run tauri build
# -> src-tauri\target\release\bundle\nsis\*.exe
# -> src-tauri\target\release\bundle\msi\*.msi
```

### Limitations of locally built binaries

- **UIAccess is disabled in unsigned builds.** Visual zoom still works, but Windows denies `MagSetInputTransform`, so mouse clicks can miss their targets while zoomed. The app reports this as `Input mapping: denied` under **Settings → Mouse input diagnostics**. Only mouse routing is affected; zooming and panning are unaffected.
- **Do not redistribute the installers from the commands above.** They are unsigned, so Windows shows an "Unknown publisher" prompt and SmartScreen warnings, and UIAccess remains denied on the destination machine.
- **Accurate click routing requires a trusted code signing certificate.** Windows grants `UIAccess` only when all three conditions are met: the binary embeds `uiAccess="true"`, it is signed with a certificate whose chain is trusted on that machine, and it is installed in a protected location such as `%ProgramFiles%`.
- **Self-signed certificates are for local QA only.** A build signed with a self-signed certificate (such as `CN=Window Pointer Zoom Local QA`) will show an untrusted signature and "Unknown publisher" on other machines, and UIAccess will still be denied because the root is not trusted there. Distributing a build that others can use requires a commercial Authenticode code signing certificate or an equivalent trusted signing service.

The signed `UIAccess` release pipeline lives in `scripts/build-uiaccess.ps1`. It sets `WPZ_UIACCESS=1`, injects the signing configuration, isolates artifacts under `src-tauri\target\uiaccess`, and verifies the signatures of both the installers and the packed executable. See [docs/uiaccess-release.md](docs/uiaccess-release.md) for the full procedure and release checklist.

## Installation, Upgrade & Uninstall

### Install

Install using either signed per-machine release installer (build them yourself as described in [Building from Source](#building-from-source)):

- **NSIS installer (`.exe`)** or **Windows Installer (`.msi`)**

Both install per-machine to `%ProgramFiles%\Window Pointer Zoom\` and require standard administrator elevation (UAC). Installing into `%ProgramFiles%` with a valid Authenticode signature and `UIAccess` manifest is required for accurate mouse click routing (`MagSetInputTransform`) while zoomed.

> [!NOTE]
> If you previously installed an older per-user build (under `%LOCALAPPDATA%`), uninstall it first to avoid duplicate entries in **Installed apps** and single-instance conflicts.

### Upgrade

1. Right-click the **Window Pointer Zoom** icon in the system tray and choose **Exit** so the running process closes cleanly and resets zoom to `1.00x`.
2. Run the new `.exe` or `.msi` installer over the existing installation. Your settings in `%APPDATA%\com.windowpointerzoom.app\settings.json` are preserved.

### Clean Uninstall

1. Open **Settings** and turn off **Start on Boot** (removes the `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` entry) if enabled.
2. Right-click the tray icon and choose **Exit**.
3. Uninstall **Window Pointer Zoom** from Windows **Settings → Apps → Installed apps** (or Control Panel).
4. *(Optional)* To remove saved preferences completely, delete `%APPDATA%\com.windowpointerzoom.app\` (which holds `settings.json`).

## Shortcuts & System Tray Cheatsheet

### Default Global Shortcuts

| Action | Default Shortcut | Notes |
| :--- | :--- | :--- |
| **Toggle Zoom** | `Ctrl + Alt + Z` | Toggles between `1.00x` and `Default Toggle Zoom` (`2.00x`) |
| **Zoom In** | `Ctrl + Alt + Up` | Increases zoom by `Step Zoom` (`+0.25x` default) |
| **Zoom Out** | `Ctrl + Alt + Down` | Decreases zoom by `Step Zoom` (`-0.25x` default) |
| **Reset Zoom** | `Ctrl + Alt + 0` | Returns magnification smoothly to `1.00x` |
| **Wheel Zoom** | `Ctrl + Alt + Scroll` | Fixed mouse-wheel shortcut (not remappable) |

### System Tray (`Window Pointer Zoom`)

Closing or minimizing the main window hides it to the system tray. Launching the app again focuses the existing window.

- **Left-click tray icon:** Open **Settings** window
- **Right-click tray menu:**
  - **Reset Zoom** — Reset magnification to `1.00x`
  - **Settings** — Show the main window
  - **Exit** — Reset zoom to `1.00x`, uninitialize the magnification engine, and quit

## UI Guide

The main window is organized into three tabs:

### 1. Zoom

- **Zoom Controls:** Shows active/inactive status, current and target magnification, manual **Zoom In** / **Zoom Out** / **Reset (1.00x)** buttons, and a **Target Zoom Level** slider (`0.05x` increments).
- **Step Zoom:** Quick presets (`0.1x`, `0.25x`, `0.5x`) plus a **Fine Step Increment** slider (`0.05x`–`2.00x`).
- **Zoom Bounds:** Configure **Minimum Zoom** (`1.00x` default), **Default Toggle Zoom** (`2.00x` default), and **Maximum Zoom** (`5.00x` default) within the `1.00x`–`10.00x` envelope (`min <= default <= max`).

### 2. Motion

- **Panning Mode & Deadzone:**
  - **Center-locked** *(default)*: Keeps the mouse pointer centered inside the zoomed viewport.
  - **Edge Panning**: Keeps the viewport steady until the pointer enters the outer **Edge Deadzone** margin (`5%`–`45%`, `15%` default).
- **Animation Speed:** Independent **Zoom Speed** and **Panning Speed** sliders mapped across **Cinematic ↔ Instant** (`Cinematic`, `Balanced`, `Responsive`, `Instant`).

### 3. Settings

- **Mouse input diagnostics:** Expandable diagnostics panel displaying `Input mapping` state (`active`, `pending`, `denied`, `error`, `disabled`), `UIAccess` status, and Win32/HRESULT error codes.
- **Global Shortcuts:** Click any shortcut button to record a new key combination (`Esc` cancels). Global hotkeys pause automatically while recording, and bare non-function keys or duplicates are rejected. Mouse wheel zoom stays fixed to `Ctrl + Alt + Scroll`.
- **System & Appearance:**
  - **Start on Boot:** Launches minimized to the system tray (`--minimized`) at Windows sign-in.
  - **Theme:** `System`, `Light`, or `Dark`.
  - **Reset to Defaults:** Restores zoom bounds, panning settings, and shortcuts.

### Input Mapping Notice & UIAccess

When Windows denies input coordinate mapping (for example in an unsigned local dev build or when launched outside `%ProgramFiles%`), a banner appears at the top of the window explaining that visual magnification still works while mouse clicks may miss targets at higher zoom levels. Check **Settings → Mouse input diagnostics** for live status.

For signed UIAccess builds, local test certificates, and release verification steps, see [docs/uiaccess-release.md](docs/uiaccess-release.md).

## Development & Testing

```powershell
# Run Tauri in development mode (unsigned, without UIAccess)
npm run tauri dev

# Frontend checks & tests
npm run typecheck
npm run lint
npm run test
npm run build

# Rust backend tests
cargo test --manifest-path src-tauri/Cargo.toml
```

See [docs/uiaccess-release.md](docs/uiaccess-release.md) for building signed NSIS/MSI release installers with `UIAccess` enabled.
