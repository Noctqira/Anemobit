# Anemobit for Windows

C# + WPF (.NET 10). Anemobit runs in the background with a floating rate window on the desktop and a system tray icon; the main window shows one card per interface.

Run all commands in this folder (`Windows/`).

## Requirements

- Windows 11
- [.NET 10 SDK](https://dotnet.microsoft.com/download/dotnet/10.0) (`winget install Microsoft.DotNet.SDK.10`)

Visual Studio is not required. To run the published Anemobit.exe or install the MSI, a computer only needs the [.NET 10 Desktop Runtime](https://dotnet.microsoft.com/download/dotnet/10.0) for its architecture (the SDK includes it).

## Building

```
dotnet build                          # build the whole solution
dotnet test                           # run the unit tests
dotnet run --project src/Anemobit     # run the app
```

Installer (MSI; written to `Outputs/` in the repository root). The executable is framework-dependent, so it doesn't include .NET: the MSI stops with a download link when the .NET 10 Desktop Runtime for its architecture isn't installed.

```
dotnet build installer/Anemobit.Installer -c Release -p:Platform=x64
dotnet build installer/Anemobit.Installer -c Release -p:Platform=arm64
```

You can also use the `Makefile` (requires GNU make): `make build` (executables), `make installer` (MSIs), `make test`, `make clean`. Besides the build output, `make clean` quits a running Anemobit and deletes your Anemobit data (the settings in `%APPDATA%\Anemobit`, the community and passwords in Windows Credential Manager, and Open at Login), including that of an installed copy. `make help` lists all targets; add `ARCHS=x64` or `ARCHS=arm64` to build only one architecture.

## Structure

- `src/Anemobit.Core`: core logic (BER, SNMP, rate calculation), independent of WPF
- `src/Anemobit`: the WPF app
- `tests/Anemobit.Core.Tests`: xUnit unit tests for Core
- `tests/Anemobit.Tests`: unit tests for the app (settings, view models, localization)
- `installer/Anemobit.Installer`: WiX MSI installer

## Distributing a modified version

If you only build and use Anemobit on your own computer, you don't need to change anything. If you distribute a modified version, replace the following so it isn't confused or in conflict with the official app:

- **Name and icon**: "Anemobit" and its icon are not covered by the license (see `NOTICE` in the repository root).
- **`Company`** in `Directory.Build.props`.
- **`Manufacturer` and the links** in `installer/Anemobit.Installer/Package.wxs`.
- **`UpgradeCode`** in `Package.wxs`: use a new GUID, otherwise your installer is treated as an upgrade of the official one.
- **Links and copyright** in `src/Anemobit/Views/AboutWindow.xaml`.
- **Credential and settings names**: the Windows Credential Manager target name (`WindowsCredentialStore.cs`) and the settings folder (`SettingsStore.cs`).
