# For Fedora: `make rpm` (run on Fedora). Rust crates are downloaded from crates.io by cargo, so the build needs network access.
Name:           anemobit
Version:        1.0.0
Release:        1%{?dist}
Summary:        Real-time interface traffic of network devices over SNMP

License:        Apache-2.0
URL:            https://github.com/noctqira/Anemobit
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  meson >= 1.5
BuildRequires:  ninja-build
BuildRequires:  cargo >= 1.88
BuildRequires:  rust >= 1.88
BuildRequires:  pkgconfig(gtk4) >= 4.22
BuildRequires:  pkgconfig(libadwaita-1) >= 1.9
BuildRequires:  pkgconfig(glib-2.0) >= 2.88
BuildRequires:  gettext
BuildRequires:  desktop-file-utils
BuildRequires:  appstream
BuildRequires:  python3

Requires:       gtk4 >= 4.22
Requires:       libadwaita >= 1.9
Recommends:     (gnome-keyring or kf6-kwallet)

%description
Anemobit reads interface counters from a device over SNMP and shows the
download and upload rates with gauges and a five-minute chart, in the
system tray and in a main window.

It works with devices that provide the standard IF-MIB with 64-bit interface
counters over SNMPv2c or SNMPv3 (authentication: MD5, SHA1, SHA224, SHA256,
SHA384, SHA512; privacy: DES, AES-128). Passwords are kept in the Secret
Service keyring.

%prep
%autosetup

%build
%meson -Dprofile=default
%meson_build

%install
%meson_install
%find_lang %{name}

%check
# clippy (suite lint) is a development check; Fedora packages it separately from cargo.
%meson_test --no-suite lint

%files -f %{name}.lang
%license LICENSE NOTICE
%{_bindir}/anemobit
%{_datadir}/applications/io.github.noctqira.Anemobit.desktop
%{_datadir}/metainfo/io.github.noctqira.Anemobit.metainfo.xml
%{_datadir}/glib-2.0/schemas/io.github.noctqira.Anemobit.gschema.xml
%{_datadir}/icons/hicolor/*/apps/io.github.noctqira.Anemobit*

%changelog
* Mon Oct 12 2026 Noctqira <340502527+Noctqira@users.noreply.github.com> - 1.0.0-1
- Initial release.
