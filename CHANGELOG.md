# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-06-04

### Added
- **Windows Installer (`OpenPartSetup.exe`)**: Built using Inno Setup for a professional installer setup (Start Menu & Desktop shortcuts, automatic UAC detection, uninstaller).
- **CI/CD Releases Pipeline**: Automatically compiles the Inno Setup installer via GitHub Actions and uploads it alongside release ZIP files.
- **Embedded UAC Elevation Manifest**: Added native Windows manifest embedding (`requireAdministrator`) to ensure installed binaries correctly prompt for UAC elevation to allow raw disk API scanning.
- **Favicon Asset**: Copied valid 32x32 BMP-based icon to `ui/favicon.ico` to fix webview asset fallback warnings.
- **Interactive partition map GUI**: Modern visual interface built with HTML/CSS/JS in a Tauri shell.
- **Command-line interface (`OpenPartCLI.exe`)**: Direct CLI access with commands for listing, resizing, moving, creating, deleting, and formatting.
- **Safety Guardrails**: Plan dry-run verification, pre-apply layout backups, BitLocker status checks, AC power status verification, and checksum I/O validation.
- Apache 2.0 license file.
