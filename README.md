# Anti-Bar

Windows desktop shell featuring a glass Dock, Dynamic Island Notch, `Alt + Space` Spotlight launcher, WASAPI per-app volume mixer, Wi-Fi/Control Center, Direct2D desktop widgets, and a showcase website.

## Download & Run (Windows 10 / 11 x64)

- **Portable ZIP (Recommended):** [`release/Anti-Bar-portable-win64.zip`](./release/Anti-Bar-portable-win64.zip) (6.0 MB) — extract both files and run `anti-bar.exe`.
- **Direct Executable:** [`release/anti-bar.exe`](./release/anti-bar.exe) (22.3 MB) — requires [`release/WebView2Loader.dll`](./release/WebView2Loader.dll) in the same folder and the Microsoft Edge WebView2 Runtime.

### Quick Controls
- **`Alt + Space`:** Toggle the Spotlight launcher.
- **System Tray:** Toggle **Hide / Show Windows Taskbar**, **Show / Hide Dock**, **Anti-Bar Settings**, or **Quit Anti-Bar** (restores the Windows taskbar automatically on exit if hidden).
- **Taskbar Replacement:** Enable **Hide / Show Windows Taskbar** from the tray menu or **Anti-Bar Settings → Behavior → Hide Windows Taskbar (Opt-in)**.

## Repository Structure

- `release/` — Prebuilt Windows x64 binary (`anti-bar.exe`), `WebView2Loader.dll`, and `Anti-Bar-portable-win64.zip`.
- `app/` — Tauri v2 + React + TypeScript + Rust Windows application (`com.antibar.app`).
- `site/` — Product showcase website (`site/index.html`).
- `.github/workflows/release.yml` — GitHub Actions workflow that builds the portable ZIP and NSIS installer on `v*` tags.

## Publishing a GitHub Release Installer

Pushing a version tag triggers `.github/workflows/release.yml` on `windows-latest` to compile and attach `Anti-Bar-portable-win64.zip`, `anti-bar.exe`, and the NSIS `*-setup.exe` installer to your GitHub **Releases** page:

```powershell
git tag v0.1.0
git push origin v0.1.0
```

## Build from Source

### Prerequisites
- Node.js 22+ and npm
- Rust toolchain (`stable-x86_64-pc-windows-msvc` with Visual Studio C++ Build Tools, or `stable-x86_64-pc-windows-gnu` with MinGW/WinLibs)
- Edge WebView2 Runtime (preinstalled on Windows 11)

### Frontend (`app/`)
```powershell
cd app
npm install
npx tsc --noEmit
npm run build
```

### Rust Backend & Tests (`app/src-tauri/`)
With MSVC (`x86_64-pc-windows-msvc`):
```powershell
cd app/src-tauri
cargo test --bin anti-bar
cargo build --release --features custom-protocol
```

With portable WinLibs GNU (`x86_64-pc-windows-gnu`) when the repo path contains spaces, set `CARGO_TARGET_DIR` to a spaceless path:
```powershell
$wbin = "$env:LOCALAPPDATA\winlibs\mingw64\bin"
$dbg = "$env:LOCALAPPDATA\antibar-target\debug"
$cleanPath = ($env:Path -split ';' | Where-Object { $_ -and ($_ -notlike '*MinGW*') }) -join ';'
$env:Path = "$dbg;$env:USERPROFILE\.cargo\bin;$wbin;$cleanPath"
$env:CARGO_TARGET_DIR = "$env:LOCALAPPDATA\antibar-target"
cd app/src-tauri
cargo +stable-x86_64-pc-windows-gnu test --bin anti-bar
cargo +stable-x86_64-pc-windows-gnu build --release --features custom-protocol
```
