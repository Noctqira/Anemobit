# Anemobit for Linux

Rust + GTK4 + libadwaita. Anemobit shows the rate of one interface in the system tray (and in the top bar on desktops that support tray labels, such as Ubuntu); the main window shows one card per interface. An optional compact window shows the rate on desktops without a system tray.

Run all commands in this folder (`Linux/`).

## Requirements

- Ubuntu 26.04 or Fedora 44 (GNOME 50: GTK 4.22, libadwaita 1.9, GLib 2.88) or later
- Rust 1.88 or later (stable, from [rustup](https://rustup.rs) or the distribution)
- Meson, Ninja, pkg-config, gettext and the GTK4 / libadwaita development files

On Ubuntu:

```bash
sudo apt install meson ninja-build pkgconf libgtk-4-dev libadwaita-1-dev libglib2.0-dev libglib2.0-dev-bin gettext desktop-file-utils appstream python3
```

On Fedora:

```bash
sudo dnf install meson ninja-build pkgconf-pkg-config gtk4-devel libadwaita-devel gettext desktop-file-utils appstream python3 cargo clippy rustfmt
```

Fedora ships clippy and rustfmt separately from cargo; `make test` needs clippy (the .rpm build skips the clippy check), and code is formatted with `cargo fmt`.

To build the .deb, also install `debhelper`. To build the .rpm on Fedora, install `rpm-build`.

The system tray uses the StatusNotifierItem protocol. On Ubuntu it works out of the box; stock GNOME needs the AppIndicator extension. The community and SNMPv3 passwords are stored through the Secret Service (GNOME Keyring or KWallet).

## Building

Meson is the build system; the `Makefile` only provides shortcuts. The first run sets up `build/` with the development profile automatically.

```bash
make build      # build
make test       # run all tests (cargo test, clippy, translation and .desktop / metainfo / schema checks)
make run        # build, then launch the app in the development environment
```

Use `make run` rather than running the built executable directly: the uninstalled GSettings schema, translations and icons are found only in the Meson development environment. The development build uses the app ID `io.github.noctqira.Anemobit.Devel`, so it can run next to an installed release.

Packages (written to `Outputs/` in the repository root):

```bash
make deb        # .deb for Ubuntu / Debian
make rpm        # .rpm, run on Fedora
```

`make clean` removes everything the build produces, including this version's packages in `Outputs/`; `make help` lists all targets.

To remove an installed package together with your settings, passwords and the login item, run `./uninstall.sh` as your normal user.

## Structure

- `core/`: the `anemobit-core` library (BER, SNMP, SNMPv3 USM, rate calculation, polling), depending only on GLib / GIO and RustCrypto
- `src/`: the GTK app
- `data/`: .desktop file, metainfo, GSettings schema, icons, UI files and style sheet
- `po/`: translations
- `debian/`, `build-aux/anemobit.spec`: packaging for .deb and .rpm

## Distributing a modified version

If you only build and use Anemobit on your own computer, you don't need to change anything. If you distribute a modified version, replace the following so it isn't confused or in conflict with the official app:

- **Name and icon**: "Anemobit" and its icon are not covered by the license (see `NOTICE` in the repository root).
- **App ID** in `meson.build`; rename the files in `data/` that are named after the app ID as well. The GSettings path follows the app ID.
- **Resource path** in `src/application.rs`.
- **Developer and links** in the metainfo file in `data/`, and the links in `src/about.rs`.
- **`authors`** in `Cargo.toml`.
- **Maintainer** in `debian/control`, `debian/changelog`, `debian/copyright` and `build-aux/anemobit.spec`.
- **Secret Service name** in `src/secrets.rs`.
