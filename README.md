# OpenPart

> **A free, open-source partition manager for Windows.**  
> Resize, move, create, and delete disk partitions with a modern GUI — no license keys, no telemetry, no nonsense.

[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![GitHub Repo](https://img.shields.io/badge/GitHub-mahmadabid%2FOpenPart-181717?logo=github)](https://github.com/mahmadabid/OpenPart)
[![Platform](https://img.shields.io/badge/platform-Windows-0078D6?logo=windows)](https://github.com/mahmadabid/OpenPart)
[![Built with Rust](https://img.shields.io/badge/built%20with-Rust-orange?logo=rust)](https://www.rust-lang.org/)

---

## Why OpenPart?

Most partition management utilities require persistent background services, registration keys, or outbound telemetry. OpenPart is built from scratch in **Rust** with transparency, security, and safety as top priorities:

- **Plan-before-write model** — Every operation compiles into a step-by-step execution plan that is reviewed before any changes are written to disk.
- **Checksum-verified moves** — Partition moves use sample-based SHA-256 read-back checksums under exclusive volume locks to verify data integrity.
- **Auto-backup snapshots** — The GUI saves a layout backup before every real write operation so you can restore if anything goes wrong.
- **Modern UI** — Interactive topology view built with HTML/JS/CSS inside a Tauri shell.
- **No background services** — Runs only when you open it. Scans disks via PowerShell on demand.
- **Fully open source** — Apache 2.0 licensed. Audit every line of code.

---

## Features

| Feature | Status |
|---|---|
| Resize partitions (extend / shrink) | ✅ |
| Move partitions (sector-by-sector copy) | ✅ |
| Create new partitions (NTFS, exFAT, FAT32) | ✅ |
| Delete partitions | ✅ |
| Format partitions | ✅ |
| Assign drive letters | ✅ |
| Wipe disk | ✅ |
| Mark partition active | ✅ |
| Restore from layout snapshot | ✅ |
| GPT partition table support | ✅ |
| Dry-run / preview mode | ✅ |
| CLI interface | ✅ |

---

## Getting Started

### Installation & Downloads

For general users, pre-compiled binaries and installers are available under the **[GitHub Releases](https://github.com/mahmadabid/OpenPart/releases)** page:

1. **Windows Installer (Recommended)**: Download and run **`OpenPartSetup.exe`** as Administrator. This installs OpenPart directly into your system, adds Start Menu and Desktop shortcuts, and sets up a clean uninstaller.
2. **Standalone Package**: Download the `OpenPart-vX.Y.Z-windows.zip` archive, extract the files, and launch `OpenPart.exe` (GUI) or run commands using `OpenPartCLI.exe` (CLI) directly.

> 🛡️ **Note on Windows Defender SmartScreen:**  
> Since OpenPart is a free, open-source project and does not use a commercial EV Code Signing Certificate, Windows SmartScreen will display a warning ("Unknown publisher") when running the downloaded installer.  
> To run it, click **"More info"** on the SmartScreen dialog, and then click **"Run anyway"**. The program requires administrator elevation to interact with physical storage drives.

---

### Prerequisites (For Building from Source)

- **Rust toolchain** — Install via [rustup.rs](https://rustup.rs)
- **PowerShell** — Must be on system PATH (ships with Windows 10/11)
- **WebView2** — Ships with Windows 10/11; [download here](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) if missing

### Build from Source

```bash
# Build everything (CLI + GUI)
cargo build --workspace --release

# Or build just the GUI
cargo build --release -p openpart-gui

# Or build just the CLI
cargo build --release -p openpart-cli
```

---

## Quick Start (Development Script)

The easiest way to build, deploy, and launch during development:

```bat
.\build_gui_copy_c.bat
```

> **Requires Administrator privileges.** The script will prompt for elevation automatically via PowerShell.

### What `build_gui_copy_c.bat` Does

| Step | What happens |
|---|---|
| **1. Build** | Runs `cargo build --release -p openpart-gui` |
| **2. Verify** | Checks that `target\release\openpart-gui.exe` exists |
| **3. Copy project** | `robocopy` mirrors the repo to `C:\Openpart`, skipping `target\`, `node_modules\`, `.git\`, `.github\` |
| **4. Deploy exe** | Copies the compiled binary to `C:\Openpart\OpenPart.exe` |
| **5. Launch as Admin** | Runs `Start-Process -Verb RunAs` to launch the app elevated |

Each step is individually guarded — if any step fails, the script prints a clear message and exits with code `1`.

```powershell
# Run from the project root
.\build_gui_copy_c.bat
```

The deployed app lives at `C:\Openpart\OpenPart.exe`.

---

## How to Use OpenPart

> ⚠️ **Modifying disk layouts can result in data loss. Always back up important data before applying changes.**

### Option A — Graphical User Interface (GUI)

The GUI provides a visual, interactive way to stage and apply partition changes.

1. **Launch as Administrator** — Right-click `OpenPart.exe` → **Run as Administrator**. Admin rights are required to lock volumes and modify partition tables.
2. **Select your disk** — Use the disk selector to pick your physical drive.
3. **Stage changes** in the topology view:
   - **Resize** — Drag partition edges to shrink or extend.
   - **Move** — Drag a partition block to shift it.
   - **Create** — Click unallocated space to create a new partition.
   - **Delete** — Right-click a partition to delete it.
4. **Preview** — Click **Dry Run** to see the execution plan without writing anything.
5. **Apply** — Click **Apply** to execute. The GUI automatically saves a layout backup snapshot before writing.

---

### Option B — Command Line Interface (CLI)

> **All CLI commands must be run from an elevated prompt (Run as Administrator).**

The CLI accepts a **global** `--disk` flag (defaults to `\\.\PhysicalDrive0`) and an optional `--dry-run` flag that applies to every command.

#### Global Flags

| Flag | Default | Description |
|---|---|---|
| `--disk <PATH>` | `\\.\PhysicalDrive0` | Physical disk path |
| `--dry-run` | off | Preview the plan without writing to disk |
| `--chunk-mib <N>` | `64` | Chunk size in MiB for move operations |
| `--keep-traces` | off | Save plan/report JSON files after execution |
| `--out-dir <PATH>` | system temp dir | Directory for trace output files |

#### Commands

**List disks and partitions**

```bash
# Human-readable output
OpenPartCLI.exe list

# Machine-readable JSON
OpenPartCLI.exe list --json
```

**Resize a partition**

Target can be a drive letter (e.g. `D`) or partition index (e.g. `2`).

```bash
# Dry run — preview only, no writes
OpenPartCLI.exe --dry-run resize D 120GB

# Apply resize
OpenPartCLI.exe resize D 120GB

# Size units: GB (default), MB, TB
OpenPartCLI.exe resize 2 500MB
```

**Move a partition**

`delta_mib` is the number of MiB to shift. Positive = right, negative = left.

```bash
# Preview moving partition D 512 MiB to the right
OpenPartCLI.exe --dry-run move D 512

# Apply
OpenPartCLI.exe move D 512
```

**Create a partition**

```bash
# Create a 50 GB NTFS partition using all available unallocated space
OpenPartCLI.exe create ntfs 50GB

# With label and drive letter
OpenPartCLI.exe create ntfs 50GB --label "Data" --letter E

# Supported filesystems: ntfs, exfat, fat32
OpenPartCLI.exe create exfat 10GB --label "USB"
```

**Delete a partition**

```bash
OpenPartCLI.exe delete D
OpenPartCLI.exe delete 3
```

**Format a partition**

```bash
OpenPartCLI.exe format D ntfs
OpenPartCLI.exe format D ntfs --label "NewLabel"
```

**Assign a drive letter**

```bash
OpenPartCLI.exe assign 2 E
```

**Wipe a disk** *(destroys all data)*

```bash
OpenPartCLI.exe wipe "\\.\PhysicalDrive1"
```

**Mark a partition active** *(MBR boot flag)*

```bash
OpenPartCLI.exe active C
OpenPartCLI.exe active 1
```

**Clone a disk**

```bash
OpenPartCLI.exe clone "\\.\PhysicalDrive0" "\\.\PhysicalDrive1"
```

**Restore a snapshot**

```bash
OpenPartCLI.exe restore 0 C:\path\to\snapshot.json
```

#### Targeting a Different Disk

Pass `--disk` before the subcommand to target any physical drive:

```bash
OpenPartCLI.exe --disk "\\.\PhysicalDrive1" list
OpenPartCLI.exe --disk "\\.\PhysicalDrive1" resize E 200GB
```

#### Using in Development (cargo run)

```bash
cargo run -p openpart-cli -- list
cargo run -p openpart-cli -- --dry-run resize D 120GB
cargo run -p openpart-cli -- --disk "\\.\PhysicalDrive1" create ntfs 50GB
```

---

## Safety Guardrails

| Guardrail | Description |
|---|---|
| **Administrator required** | Windows rejects low-level storage access without elevated privileges. |
| **Dry-run preview** | Pass `--dry-run` to the CLI (or use the Dry Run button in the GUI) to see the full plan before any disk writes occur. |
| **Pre-write backup** | The GUI saves a layout snapshot before every real apply operation. You can restore from it via the Restore command. |
| **BitLocker check** | Move operations refuse to proceed if BitLocker is active on the target volume. |
| **AC power check** | Move operations check that the system is on AC power before starting a long sector copy. |
| **Checksum verification** | Move operations capture SHA-256 hashes of sampled regions before and after copying to confirm data integrity. |
| **Overlap validation** | Move operations verify the new offset range doesn't overlap any other partition before writing. |

---

## Project Structure

```
├── crates/
│   ├── openpart-core/          # Core: plan engine, executors, sector I/O, models
│   │   └── src/
│   │       ├── engine.rs       # build_plan() — composes the execution plan
│   │       ├── executor.rs     # SimulatedExecutor (dry-run) and RealExecutor (live writes)
│   │       ├── models.rs       # PlanInputs, PlanAction, Plan, ApplyReport, DiskState
│   │       └── scanner.rs      # Disk/partition scanning
│   ├── openpart-cli/           # CLI binary (clap-based)
│   │   └── src/lib.rs          # All subcommands: list, resize, move, create, delete, ...
│   └── openpart-service/       # PowerShell disk scanner (scan_disks())
├── src-tauri/
│   └── src/main.rs             # Tauri commands: get_state, apply_plan_command, restore, ...
├── ui/
│   ├── index.html              # Single-page GUI shell
│   └── app.js                  # UI logic and Tauri invoke calls
├── icons/                      # Application icons
└── build_gui_copy_c.bat        # Dev script: build → deploy to C:\Openpart → launch as admin
```

---

## Contributing

Contributions are welcome! Please open an issue first to discuss what you'd like to change.

1. Fork the repository: [github.com/mahmadabid/OpenPart](https://github.com/mahmadabid/OpenPart)
2. Create your feature branch: `git checkout -b feature/amazing-feature`
3. Commit your changes: `git commit -m 'Add amazing feature'`
4. Push to the branch: `git push origin feature/amazing-feature`
5. Open a Pull Request

---

## License

Licensed under the **Apache License, Version 2.0**.  
See the full license text in [LICENSE](LICENSE) or at [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0).