//! Open at Login: XDG autostart (`~/.config/autostart/<app ID>.desktop`), supported by both GNOME and KDE.
//! The file is the source of truth (the user may change it in system settings or other tools); the toggle applies immediately.

use std::io;
use std::path::{Path, PathBuf};

use gtk::glib;

use crate::config;

#[derive(Debug, Clone)]
pub struct Autostart {
    path: PathBuf,
}

impl Default for Autostart {
    fn default() -> Self {
        let file = format!("{}.desktop", config::APP_ID);
        Autostart {
            path: glib::user_config_dir().join("autostart").join(file),
        }
    }
}

impl Autostart {
    /// Uses the given file location (for tests).
    #[cfg(test)]
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Autostart { path: path.into() }
    }

    #[cfg(test)]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The file exists and isn't disabled (`Hidden=true` or `X-GNOME-Autostart-enabled=false`).
    pub fn is_enabled(&self) -> bool {
        let entry = glib::KeyFile::new();
        if entry
            .load_from_file(&self.path, glib::KeyFileFlags::NONE)
            .is_err()
        {
            return false;
        }
        let hidden = entry.boolean("Desktop Entry", "Hidden").unwrap_or(false);
        let enabled = entry
            .boolean("Desktop Entry", "X-GNOME-Autostart-enabled")
            .unwrap_or(true);
        !hidden && enabled
    }

    /// Writes the file (running `executable`) when turning on; deletes it when turning off.
    pub fn set_enabled(&self, enabled: bool, executable: &Path) -> io::Result<()> {
        if enabled {
            if let Some(directory) = self.path.parent() {
                std::fs::create_dir_all(directory)?;
            }
            std::fs::write(&self.path, desktop_entry(executable))
        } else {
            match std::fs::remove_file(&self.path) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
                result => result,
            }
        }
    }
}

/// Exec field of a desktop entry: quoted and escaped when it contains spaces or special characters (Desktop Entry Specification).
fn quote_exec(path: &str) -> String {
    const RESERVED: &[char] = &[
        ' ', '\t', '\n', '"', '\'', '\\', '>', '<', '~', '|', '&', ';', '$', '*', '?', '#', '(',
        ')', '`',
    ];
    if !path.contains(RESERVED) {
        return path.to_owned();
    }
    // Inside quotes, " ` $ \ get a backslash; the string value itself then writes backslashes as \\.
    let mut quoted = String::from("\"");
    for c in path.chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            quoted.push_str("\\\\");
        }
        quoted.push(c);
    }
    quoted.push('"');
    quoted
}

fn desktop_entry(executable: &Path) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=Anemobit\nExec={} --{}\nIcon={}\nX-GNOME-Autostart-enabled=true\n",
        quote_exec(&executable.to_string_lossy()),
        crate::application::STARTUP_OPTION,
        config::APP_ID
    )
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    /// Each test's own temporary folder (never touches the user's ~/.config/autostart).
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "anemobit-autostart-test-{}-{n}",
                std::process::id()
            ));
            TempDir(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn enabling_writes_a_desktop_file_and_disabling_removes_it() {
        let dir = TempDir::new();
        let autostart = Autostart::at(dir.0.join("autostart").join("anemobit.desktop"));
        assert!(!autostart.is_enabled());

        autostart
            .set_enabled(true, Path::new("/usr/bin/anemobit"))
            .unwrap();
        assert!(autostart.is_enabled());
        let content = std::fs::read_to_string(autostart.path()).unwrap();
        assert!(
            content.contains("Exec=/usr/bin/anemobit --startup\n"),
            "{content}"
        );
        assert!(content.starts_with("[Desktop Entry]\n"));

        autostart
            .set_enabled(false, Path::new("/usr/bin/anemobit"))
            .unwrap();
        assert!(!autostart.is_enabled());
        assert!(!autostart.path().exists());
        // Turning off when already off isn't an error.
        autostart
            .set_enabled(false, Path::new("/usr/bin/anemobit"))
            .unwrap();
    }

    #[test]
    fn disabled_entries_count_as_off() {
        let dir = TempDir::new();
        std::fs::create_dir_all(&dir.0).unwrap();
        for disabled in ["Hidden=true", "X-GNOME-Autostart-enabled=false"] {
            let path = dir.0.join("anemobit.desktop");
            std::fs::write(
                &path,
                format!("[Desktop Entry]\nType=Application\nExec=anemobit\n{disabled}\n"),
            )
            .unwrap();
            assert!(!Autostart::at(&path).is_enabled(), "{disabled}");
        }
        let path = dir.0.join("anemobit.desktop");
        std::fs::write(
            &path,
            "[Desktop Entry]\nType=Application\nExec=anemobit\nHidden=false\n",
        )
        .unwrap();
        assert!(Autostart::at(&path).is_enabled());
    }

    #[test]
    fn exec_paths_are_quoted_when_needed() {
        assert_eq!(quote_exec("/usr/bin/anemobit"), "/usr/bin/anemobit");
        assert_eq!(
            quote_exec("/home/me/My Apps/anemobit"),
            "\"/home/me/My Apps/anemobit\""
        );
        assert_eq!(quote_exec("/opt/a$b/anemobit"), "\"/opt/a\\\\$b/anemobit\"");
    }
}
