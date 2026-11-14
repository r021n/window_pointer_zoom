# Building and verifying interactive zoom on Windows

Fullscreen magnification and input routing are separate Windows operations. Since Windows 10 1703, mouse routing also requires `MagSetInputTransform`, which requires a process with UIAccess. An ordinary administrator launch does not supply that flag.

References: [MagSetInputTransform](https://learn.microsoft.com/en-us/windows/win32/api/magnification/nf-magnification-magsetinputtransform), [Microsoft UIAccess requirements](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-securityoverview).

## Development

Use `npm run tauri dev`. Without `WPZ_UIACCESS=1`, the build keeps Tauri's default manifest (and `src-tauri/tauri.conf.json` intentionally omits signing settings), so unsigned local development remains possible. Input mapping is denied in this mode; visual magnification alone is not evidence that clicks are accurate.

`WPZ_UIACCESS=1` opts into `src-tauri/uiaccess.manifest.xml`. The manifest preserves Tauri's Common Controls dependency, sets `<requestedExecutionLevel level="asInvoker" uiAccess="true" />`, and declares `PerMonitorV2` DPI awareness. `src-tauri/build.rs` watches `WPZ_UIACCESS`, so switching modes rebuilds application resources.

## Signed installers (NSIS & MSI)

### Prerequisites

- Normal project build dependencies (Node.js, Rust MSVC toolchain, and the chosen Windows target).
- Windows SDK signing tools (`signtool.exe`, available to the Tauri bundler).
- **WiX Toolset v3** on `PATH` (required when building `.msi` bundles, including the default `nsis,msi` build).
- An existing, currently valid Authenticode Code Signing certificate, with an accessible private key, in `Cert:\CurrentUser\My`. The certificate chain must be trusted by both the build machine and any destination QA machine.
- An RFC 3161 timestamp server URL (`-TimestampUrl`), plus network access for timestamping and build dependencies.

### Build command

From the repository root, substitute your certificate thumbprint and timestamp server:

```powershell
.\scripts\build-uiaccess.ps1 `
  -CertificateThumbprint 'YOUR_40_HEX_CHARACTER_THUMBPRINT' `
  -TimestampUrl 'http://timestamp.digicert.com'
```

Optional parameters:

- `-Target`: `'x86_64-pc-windows-msvc'` (default) or `'aarch64-pc-windows-msvc'`.
- `-Bundles`: `'nsis'`, `'msi'`, or `@('nsis', 'msi')` (default: `@('nsis', 'msi')`). For example, pass `-Bundles nsis` to build only the NSIS `.exe` installer, or `-Bundles msi` to build only the WiX `.msi` installer without running both.

What `scripts/build-uiaccess.ps1` does:

1. Validates that the certificate in `Cert:\CurrentUser\My` has a private key, is within its validity period, includes the Code Signing EKU (`1.3.6.1.5.5.7.3.3`), and builds a trusted `X509Chain`.
2. Sets `WPZ_UIACCESS=1` and isolates Cargo artifacts in `src-tauri\target\uiaccess`.
3. Uses static packaging settings from `src-tauri/tauri.conf.json` (`bundle.targets`: `["nsis", "msi"]`, `bundle.windows.nsis.installMode`: `"perMachine"`, `bundle.windows.wix.language`: `"en-US"`) and injects dynamic SHA-256 + RFC 3161 signing keys via a temporary `--config` JSON.
4. Verifies `Get-AuthenticodeSignature` (`Status -eq 'Valid'`, matching thumbprint, and non-null `TimeStamperCertificate`) on every produced installer under `src-tauri\target\uiaccess\<Target>\release\bundle\nsis\*.exe` and `...\bundle\msi\*.msi`. When `msi` is built, it also extracts the `.msi` to a temporary directory (`msiexec.exe /a`) and verifies the packed `window-pointer-zoom.exe` inside it (since Tauri restores the unpatched build-folder `.exe` after bundling).
5. Restores `WPZ_UIACCESS`, `CARGO_TARGET_DIR`, and deletes the temporary config file in its `finally` block.

`scripts/build-uiaccess.ps1` never creates certificates, installs the app, or modifies Windows trust stores or security policies.

### Local self-signed test certificate (QA only)

For clean-room Windows 10/11 QA without a commercial certificate, create and trust a local test certificate manually in an elevated PowerShell session (never use a self-signed certificate for public distribution):

```powershell
# 1. Create a Code Signing certificate in the current user's Personal store
$cert = New-SelfSignedCertificate `
  -Type CodeSigningCert `
  -Subject 'CN=Window Pointer Zoom Local QA' `
  -CertStoreLocation 'Cert:\CurrentUser\My'

$cert.Thumbprint

# 2. Export the public certificate (.cer)
$cerPath = Join-Path $env:TEMP 'wpz-local-qa.cer'
Export-Certificate -Cert $cert -FilePath $cerPath | Out-Null

# 3. Trust it on the build/test machine(s) (requires Administrator)
Import-Certificate -FilePath $cerPath -CertStoreLocation 'Cert:\LocalMachine\Root' | Out-Null
Import-Certificate -FilePath $cerPath -CertStoreLocation 'Cert:\LocalMachine\TrustedPublisher' | Out-Null
```

If testing on a separate Windows 10/11 VM, copy `wpz-local-qa.cer` to that VM and run the two `Import-Certificate` commands there as Administrator before running the installer. Then build with `.\scripts\build-uiaccess.ps1 -CertificateThumbprint $cert.Thumbprint -TimestampUrl '<RFC3161_TIMESTAMP_URL>'`.

> [!IMPORTANT]
> Install the resulting `.exe` or `.msi` using its standard UAC elevation prompt into `%ProgramFiles%\Window Pointer Zoom\`. Do not launch `window-pointer-zoom.exe` directly from `src-tauri\target\uiaccess\`, `Downloads`, `Desktop`, or any user-writable directory — Windows grants `UIAccess` only when the manifest, trusted signature, and protected `%ProgramFiles%` location are all satisfied together.

## Release verification (Windows 10 1703+ & Windows 11)

Run this checklist for both the NSIS (`.exe`) and WiX (`.msi`) per-machine installers:

### 1. Clean install & binary checks

1. Uninstall any prior per-user (`%LOCALAPPDATA%`) dev/test build first and ensure no old `window-pointer-zoom.exe` process is running.
2. Run the signed `.exe` or `.msi` installer and accept the elevation prompt to install into `%ProgramFiles%\Window Pointer Zoom\`.
3. Verify the installed binary's Authenticode signature and embedded manifest:
   ```powershell
   Get-AuthenticodeSignature 'C:\Program Files\Window Pointer Zoom\window-pointer-zoom.exe'
   # Optional (requires Windows SDK mt.exe): extract manifest and confirm uiAccess="true"
   mt.exe -inputresource:"C:\Program Files\Window Pointer Zoom\window-pointer-zoom.exe";#1 -out:"$env:TEMP\wpz.manifest"
   ```
4. Check that the application icon (`icon.ico`) renders cleanly in the installer UI, Start Menu / Explorer, Taskbar, and System Tray.

### 2. UIAccess & interactive input-mapping checks

1. Launch the installed app, enable zoom (`Ctrl + Alt + Z`), and open **Settings → Mouse input diagnostics**. Verify `Input mapping: active` and `UIAccess: true` (any `denied`/`error` status is a failed release check).
2. In File Explorer **Details** view, select each of the first five rows at `1.0x`, `1.25x`, `1.5x`, `2.0x`, `2.5x`, `3.0x`, and `5.0x` (or configured maximum). Check single-click, double-click, `Ctrl`/`Shift` selection, right-click context menus, and drag selection, including during and after zoom/pan animation.
3. Repeat with Explorer maximized and restored, at different screen positions, with the settings window hidden to tray, and in another application's controls. Compare **Center-locked** and **Edge Panning**.
4. Repeat at `100%`, `125%`, `150%`, and `200%` display scaling, across mixed-DPI monitors, and with a secondary monitor positioned above/left of the primary display (negative virtual-desktop coordinates). Record resolution, DPI, monitor layout, scale, and mapping status for any failure.

### 3. Autostart (`--minimized`) check

1. In **Settings → System & Appearance**, enable **Start on Boot**.
2. Sign out and sign back in (or reboot).
3. Confirm the app starts automatically in the system tray with its main window hidden (`--minimized`), without an interactive UAC block, and that **Settings → Mouse input diagnostics** still reports `UIAccess: true` and `Input mapping: active`.

### 4. In-place upgrade check

1. Customize a setting (e.g., `Default Toggle Zoom` or theme) so `%APPDATA%\com.windowpointerzoom.app\settings.json` is written.
2. Right-click the tray icon and choose **Exit** (confirm zoom resets to `1.00x` and no process remains).
3. Run the newer/rebuilt `.exe` or `.msi` installer over the existing `%ProgramFiles%` installation.
4. Launch the upgraded app and confirm your saved settings persist and `UIAccess: true` / `Input mapping: active` still hold.

### 5. Clean uninstall & negative dev-build check

1. Disable **Start on Boot** in **Settings** (clears `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`) if you want a clean registry state, then right-click the tray icon and choose **Exit**.
2. Uninstall **Window Pointer Zoom** via Windows **Settings → Apps → Installed apps**.
3. Confirm `%ProgramFiles%\Window Pointer Zoom\`, the Start Menu shortcut, and the `HKLM` uninstall entry are removed and no `window-pointer-zoom.exe` process remains. *(Optionally delete `%APPDATA%\com.windowpointerzoom.app\` to clear `settings.json`.)*
4. Separately verify an unsigned `npm run tauri dev` build: `InputMappingNotice` and **Settings → Mouse input diagnostics** must visibly report denied input mapping rather than presenting it as healthy interactive zoom.

Automated geometry tests cannot replace this installed, signed, interactive regression check.
