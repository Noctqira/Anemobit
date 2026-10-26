# Anemobit for macOS and iOS

Swift + SwiftUI. On macOS, Anemobit lives in the menu bar and has a main window; on iOS / iPadOS it is a regular app. Both apps share one Xcode project, generated with XcodeGen from `project.yml`.

Run all commands in this folder (`Darwin/`).

## Requirements

- macOS 27 and Xcode 27, with the iOS 27 simulator (if it is missing: `xcodebuild -downloadPlatform iOS`)
- [XcodeGen](https://github.com/yonaskolb/XcodeGen) (`brew install xcodegen`): the Xcode project is not in the repository and is generated from `project.yml`

## Building

### Command line (Makefile)

```bash
make            # generate the Xcode project, then build the macOS and iOS (simulator) apps
make test       # run the Core unit tests
```

Output:

- macOS: `DerivedData/Anemobit/Build/Products/Debug/Anemobit.app`
- iOS simulator: `DerivedData/Anemobit/Build/Products/Debug-iphonesimulator/Anemobit.app`

Also: `make mac`, `make ios`, `make CONFIGURATION=Release`, `make clean`. `make help` lists all targets.

### Xcode

```bash
make generate
open Anemobit.xcodeproj
```

`Anemobit.xcodeproj` is not included in the repository: generate it with `make generate` (or any `make` build) first. Choose a scheme and a run destination, then press ⌘R:

- `AnemobitMac` → My Mac
- `AnemobitiOS` → an iPhone or iPad simulator

To change project settings, edit `project.yml` and run `make generate`. Don't change settings in Xcode directly: they are overwritten the next time the project is generated.

## Structure

- `Packages/Core`: Swift package with the core logic (BER, SNMP, SNMPv3 USM, rate calculation, polling, formatting), independent of any UI framework
- `Packages/Core/Tests/CoreTests`: Swift Testing unit tests for Core
- `App/Shared`: SwiftUI views, view models, settings and styling shared by both apps
- `App/macOS`: the macOS app (menu bar, main window, About panel)
- `App/iOS`: the iOS / iPadOS app (dashboard, settings sheet, About page)
- `Config`: bundle ID prefix and code signing settings
- `project.yml`: XcodeGen project definition; `Anemobit.xcodeproj` is generated from it and is not committed
- `ExportOptions-AppStore.plist`, `ExportOptions-DeveloperID.plist`: export settings for the App Store and for Developer ID distribution

## Code signing

By default the apps are signed to run locally, so the macOS app and the iOS simulator need no Apple developer account.

To sign with your own developer account (for the App Store, a notarized DMG, or running on an iPhone or iPad), copy the example settings and enter your Team ID (developer.apple.com → Account → Membership details):

```bash
cp Config/Signing.local.xcconfig.example Config/Signing.local.xcconfig
```

This file is ignored by git (see `.gitignore`), and both Xcode (Product → Archive) and `make archive` read it, so your Team ID never ends up in the project file. You can also skip the file and run `make archive TEAM=<your Team ID>`. Don't choose a team in Xcode's Signing & Capabilities tab: that writes the Team ID into `Anemobit.xcodeproj`.

## Publishing to the App Store (macOS and iOS)

Requires an Apple Developer Program membership. Set up code signing (above), then run:

```bash
make archive
```

```bash
make export
```

`build/Export/` then contains the macOS `.pkg` and the iOS `.ipa`. Upload them to App Store Connect with Transporter or the Xcode Organizer. Increase `CURRENT_PROJECT_VERSION` in `project.yml` before each upload.

## Distributing the macOS app outside the App Store (Developer ID + notarization)

For downloads from GitHub Releases rather than the Mac App Store. The app is signed with Developer ID and notarized by Apple, so Gatekeeper lets users open it. iOS has no equivalent; it is available only through the App Store.

Requires an Apple Developer Program membership and this one-time setup:

1. **Developer ID Application certificate**: create it in Xcode → Settings → Accounts → Manage Certificates.
2. **Notarization keychain profile**: create an app-specific password at account.apple.com, then run the command below. It asks for the password and stores it in your keychain.

   ```bash
   xcrun notarytool store-credentials Anemobit --apple-id <your Apple ID> --team-id <your Team ID>
   ```

3. Set `ANEMOBIT_TEAM` and `ANEMOBIT_NOTARY_PROFILE` in `Config/Signing.local.xcconfig` (see "Code signing").

Then run:

```bash
make dmg
```

This makes a Release archive, exports it with Developer ID, notarizes and staples the app, builds the DMG, then signs, notarizes and staples the DMG. The result is `../Outputs/Anemobit-<version>-macOS.dmg`. An existing DMG of the same version is never overwritten: increase `MARKETING_VERSION` in `project.yml` first. Both distributions use the same bundle ID and App Sandbox settings and behave the same.

### DMG with a background image

`make create-dmg` does the same as `make dmg` (same notarized app, same signing, notarization and stapling, same output file name), but the DMG window has a background image with the app and the Applications link laid out on it. It needs [create-dmg](https://github.com/create-dmg/create-dmg) and two background images:

```bash
brew install create-dmg
```

- `DMG/background.png`: 660 × 400 pixels (the window size in points)
- `DMG/background@2x.png`: 1320 × 800 pixels, for Retina displays

They're combined into one HiDPI TIFF. The icons are 128 points, centered at (165, 185) for the app and (495, 185) for Applications, measured from the top-left corner of the image in points; leave room for their names below them. Finder doesn't switch the background for Dark Mode, so check that the icon names stay readable in both appearances.

Finder lays out the window, so the first run asks to allow Terminal (or Xcode) to control Finder. To preview the design without a developer account, build an unsigned DMG from any build of the app:

```bash
make create-dmg-layout APP=DerivedData/Build/Products/Debug/Anemobit.app OUT=build/preview.dmg
```

## Distributing a modified version

If you only build and use Anemobit on your own computer, you don't need to change anything. If you distribute a modified version, replace the following so it isn't confused or in conflict with the official app (for example, sharing its keychain items):

- **Name and icon**: "Anemobit" and its icon are not covered by the license (see `NOTICE` in the repository root).
- **`ANEMOBIT_BUNDLE_ID_PREFIX`** in `Config/Signing.xcconfig`: the bundle ID and the keychain service name are derived from it.
- **`NSHumanReadableCopyright`** in `project.yml` (run `make generate` afterwards).
- **Copyright** in `App/iOS/AboutView.swift`.
- **Website links** in `App/Shared/AppLinks.swift`.
