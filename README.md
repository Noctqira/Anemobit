# Anemobit

**English** | [繁體中文](README.zh-TW.md) | [日本語](README.ja.md) | [한국어](README.ko.md)

Anemobit shows the live traffic of your network device's interfaces, read over SNMP (v2c or v3). It works with any device that supports the standard IF-MIB with 64-bit interface counters. Each platform has its own native app: macOS, iOS / iPadOS, Windows and Linux.

<p align="center">
  <img src="Screenshots/macOS.png" width="720" alt="Anemobit on macOS">
</p>

## Features

- Live download and upload rates for one or more interfaces, each with a gauge and a 5-minute chart
- Current rate, 5-minute average and 5-minute peak
- The rate of one chosen interface always visible: in the menu bar (macOS), a floating window and the system tray (Windows), or the system tray and top bar (Linux)
- SNMPv2c, and SNMPv3 with authentication (MD5, SHA1, SHA224, SHA256, SHA384, SHA512) and privacy (DES, AES-128); SHA224 is not available on Windows
- Interfaces are found by scanning the device; you pick them by name
- Rates in bits (Mbps) or bytes (MB/s), updated every 1, 5 or 10 seconds
- The community and SNMPv3 passwords are stored in the system's secure storage
- English, Traditional Chinese, Japanese and Korean; follows the system language by default, or choose one in Settings
- No analytics, ads or tracking; Anemobit connects only to the device you enter

## Screenshots

| iOS | Windows | Linux |
|:-:|:-:|:-:|
| <img src="Screenshots/iOS.png" width="160" alt="Anemobit on iOS"> | <img src="Screenshots/Windows.png" width="280" alt="Anemobit on Windows"> | <img src="Screenshots/Linux.png" width="280" alt="Anemobit on Linux"> |

## Download

| Platform | Requirements | Download |
|---|---|---|
| macOS | macOS 27 | Mac App Store (coming soon) · DMG from [GitHub Releases](https://github.com/Noctqira/Anemobit/releases) |
| iOS / iPadOS | iOS 27 / iPadOS 27 | App Store (coming soon) |
| Windows | Windows 11, x64 or ARM64, with the [.NET 10 Desktop Runtime](https://dotnet.microsoft.com/download/dotnet/10.0) | MSI installer from [GitHub Releases](https://github.com/Noctqira/Anemobit/releases) |
| Linux | Ubuntu 26.04 or Fedora 44, x86_64 or ARM64 | .deb / .rpm from [GitHub Releases](https://github.com/Noctqira/Anemobit/releases) |

## Setting up your device

Anemobit reads the interface counters through the device's SNMP agent. The steps differ between devices; see your device's documentation for the details.

1. **Turn on SNMP** on the device (often called the SNMP agent or SNMP service).
2. **Choose the security:**
   - **SNMPv2c**: set a community name. Use something other than the default `public`.
   - **SNMPv3**: create a user with an authentication protocol and password, and optionally a privacy protocol and password. Passwords must be at least 8 characters.
3. **Limit access:** read-only access is enough. If the device allows it, accept SNMP requests only from the computer or phone running Anemobit.
4. **Allow the traffic:** the device's firewall must accept SNMP (UDP port 161) from your local network.

Then open Settings in Anemobit, enter the device's address and the same security settings, scan the interfaces and choose the ones to watch. The device must provide the 64-bit counters from IF-MIB (`ifHCInOctets` / `ifHCOutOctets`).

## Building from source

Each platform is built on its own, with the platform's native tools:

| Folder | Platform | Technology | Build instructions |
|---|---|---|---|
| `Darwin/` | macOS, iOS / iPadOS | Swift + SwiftUI | [Darwin/README.md](Darwin/README.md) |
| `Windows/` | Windows | C# + WPF (.NET 10) | [Windows/README.md](Windows/README.md) |
| `Linux/` | Linux | Rust + GTK4 + libadwaita | [Linux/README.md](Linux/README.md) |

## License

Anemobit is licensed under the [Apache License 2.0](LICENSE), copyright © 2026 Noctqira (see [NOTICE](NOTICE)). The name "Anemobit" and its icon are not covered by the license: if you distribute a modified version, use a different name and icon. Each platform's README lists what else to change.

Privacy policy: [PRIVACY.md](PRIVACY.md)
