#!/usr/bin/env bash
# Removes the Anemobit package (apt on Ubuntu / Debian, dnf on Fedora) and deletes user data the package manager doesn't touch:
# settings (dconf), community and passwords (keyring), and the Open at Login file.
# Run as a regular user (the data is in your own home directory and keyring); only the package removal step uses sudo.
set -u

APP_ID=io.github.noctqira.Anemobit

if [ "$(id -u)" -eq 0 ]; then
    echo "Run this as a regular user (not with sudo); otherwise it would clear root's settings and keyring." >&2
    exit 1
fi

# Pick apt or dnf: first check which package manager has the package installed; if neither, see which one the system has.
if command -v dpkg-query >/dev/null &&
    dpkg-query -W -f='${Status}' anemobit 2>/dev/null | grep -q "install ok installed"; then
    manager=apt
    installed=yes
elif command -v rpm >/dev/null && rpm -q anemobit >/dev/null 2>&1; then
    manager=dnf
    installed=yes
elif command -v apt-get >/dev/null; then
    manager=apt
    installed=no
elif command -v dnf >/dev/null; then
    manager=dnf
    installed=no
else
    echo "Neither apt nor dnf was found. Only Ubuntu / Debian and Fedora are supported." >&2
    exit 1
fi
echo "Package manager: $manager"

echo "This will remove Anemobit and delete its settings, passwords and Open at Login setting (including the development build's)."
read -r -p "Continue? [y/N] " answer
case "$answer" in
    [yY] | [yY][eE][sS]) ;;
    *) echo "Cancelled."; exit 0 ;;
esac

# 1. Quit the running app (it stays in the system tray).
if pkill -x anemobit; then
    echo "Quit the running Anemobit."
fi

# 2. Remove the package.
if [ "$installed" = yes ]; then
    sudo "$manager" remove -y anemobit || { echo "Failed to remove the package; user data was not deleted." >&2; exit 1; }
else
    echo "The anemobit package is not installed; skipping."
fi

# 3. Settings (Devel/ underneath holds the development build's settings and is removed too).
if command -v dconf >/dev/null; then
    dconf reset -f "/io/github/noctqira/Anemobit/" && echo "Deleted settings."
else
    echo "dconf not found; settings were not deleted." >&2
fi

# 4. Community and passwords (shared by the release and development builds).
if command -v secret-tool >/dev/null; then
    secret-tool clear application "$APP_ID" && echo "Deleted passwords from the keyring."
else
    if [ "$manager" = apt ]; then
        install_hint="sudo apt install libsecret-tools"
    else
        install_hint="sudo dnf install libsecret"
    fi
    echo "secret-tool not found ($install_hint); passwords were not deleted." >&2
    echo "You can also delete the Anemobit items manually in Passwords and Keys." >&2
fi

# 5. Open at Login.
autostart_dir="${XDG_CONFIG_HOME:-$HOME/.config}/autostart"
rm -f "$autostart_dir/$APP_ID.desktop" "$autostart_dir/$APP_ID.Devel.desktop"
echo "Deleted the Open at Login setting."

echo "Done."
