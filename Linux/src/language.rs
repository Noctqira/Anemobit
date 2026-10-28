//! Display language chosen in Settings (default: follow the system). It is applied at startup
//! through the `LANGUAGE` environment variable, which gettext reads for our strings as well as
//! GTK's and libadwaita's, so it takes effect after a restart.
//!
//! The variable can't be set inside the process without `unsafe`, so when the setting asks for a
//! different language, the process replaces itself once (`exec`) with `LANGUAGE` set.

use std::ffi::OsString;
use std::io;
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use gtk::prelude::*;
use gtk::{gio, glib};

use crate::config;

/// Command-line option of a process started by "Restart Now".
pub const RELAUNCH_OPTION: &str = "relaunch";

/// The `LANGUAGE` value from before the app changed it, so "Follow System" can restore it.
const SYSTEM_LANGUAGE_VARIABLE: &str = "ANEMOBIT_SYSTEM_LANGUAGE";

/// How long a relaunched process waits for the previous one to quit.
const RELAUNCH_WAIT: Duration = Duration::from_secs(5);

static LAUNCH_LANGUAGE: OnceLock<AppLanguage> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppLanguage {
    #[default]
    System,
    English,
    TraditionalChinese,
    Japanese,
    Korean,
}

impl AppLanguage {
    pub const ALL: [AppLanguage; 5] = [
        AppLanguage::System,
        AppLanguage::English,
        AppLanguage::TraditionalChinese,
        AppLanguage::Japanese,
        AppLanguage::Korean,
    ];

    /// Value stored in GSettings and given to `LANGUAGE` (empty = follow the system).
    pub fn code(self) -> &'static str {
        match self {
            AppLanguage::System => "",
            AppLanguage::English => "en",
            AppLanguage::TraditionalChinese => "zh_TW",
            AppLanguage::Japanese => "ja",
            AppLanguage::Korean => "ko",
        }
    }

    /// Unknown values follow the system.
    pub fn from_code(code: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|language| language.code() == code)
            .unwrap_or_default()
    }

    /// The language's name in that language (not translated); `None` for Follow System.
    pub fn native_name(self) -> Option<&'static str> {
        match self {
            AppLanguage::System => None,
            AppLanguage::English => Some("English"),
            AppLanguage::TraditionalChinese => Some("繁體中文"),
            AppLanguage::Japanese => Some("日本語"),
            AppLanguage::Korean => Some("한국어"),
        }
    }

    pub fn read(settings: &gio::Settings) -> Self {
        Self::from_code(&settings.child("display").string("language"))
    }

    pub fn write(self, settings: &gio::Settings) -> Result<(), glib::BoolError> {
        settings
            .child("display")
            .set_string("language", self.code())
    }
}

/// An environment variable to set (`Some`) or remove (`None`).
type EnvironmentChange = (&'static str, Option<String>);

/// Changes needed for this process to run in `language`; `None` if it already does.
/// `current` is `LANGUAGE`, `saved_system` is [`SYSTEM_LANGUAGE_VARIABLE`].
fn startup_environment(
    language: AppLanguage,
    current: Option<&str>,
    saved_system: Option<&str>,
) -> Option<Vec<EnvironmentChange>> {
    let code = language.code();
    if language == AppLanguage::System || current == Some(code) {
        return None;
    }
    let system = saved_system.or(current).unwrap_or_default();
    Some(vec![
        ("LANGUAGE", Some(code.to_owned())),
        (SYSTEM_LANGUAGE_VARIABLE, Some(system.to_owned())),
    ])
}

/// Environment of a relaunched process: the system's `LANGUAGE`, so it starts from the same
/// state as a normal launch and applies the setting itself.
fn relaunch_environment(saved_system: Option<&str>) -> Vec<EnvironmentChange> {
    let Some(system) = saved_system else {
        return Vec::new();
    };
    let language = (!system.is_empty()).then(|| system.to_owned());
    vec![("LANGUAGE", language), (SYSTEM_LANGUAGE_VARIABLE, None)]
}

/// The language this process runs in (the setting at startup).
pub fn launch_language() -> AppLanguage {
    LAUNCH_LANGUAGE.get().copied().unwrap_or_default()
}

/// Called first in `main`: applies the language setting, replacing the process if needed.
pub fn apply_at_startup() {
    let language = AppLanguage::read(&gio::Settings::new(config::APP_ID));
    LAUNCH_LANGUAGE.set(language).ok();
    let current = std::env::var("LANGUAGE").ok();
    let saved_system = std::env::var(SYSTEM_LANGUAGE_VARIABLE).ok();
    let Some(changes) = startup_environment(language, current.as_deref(), saved_system.as_deref())
    else {
        return;
    };
    let result = std::env::current_exe().map(|exe| {
        let mut command = Command::new(exe);
        command.args(std::env::args_os().skip(1));
        apply(&mut command, &changes);
        // Only returns on failure.
        command.exec()
    });
    let error = match result {
        Ok(error) | Err(error) => error,
    };
    glib::g_warning!(
        "anemobit",
        "Could not apply the display language, using the system language: {error}"
    );
}

/// A relaunched process waits until the previous one has quit (released the app ID on D-Bus);
/// otherwise it would hand its launch over to the quitting process.
pub fn wait_for_previous_instance() {
    let relaunch = format!("--{RELAUNCH_OPTION}");
    if !std::env::args_os().any(|arg| arg == OsString::from(&relaunch)) {
        return;
    }
    let Ok(connection) = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE) else {
        return;
    };
    let deadline = Instant::now() + RELAUNCH_WAIT;
    while Instant::now() < deadline {
        let owned = connection
            .call_sync(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus",
                "NameHasOwner",
                Some(&(config::APP_ID,).to_variant()),
                Some(glib::VariantTy::new("(b)").expect("valid type")),
                gio::DBusCallFlags::NONE,
                -1,
                gio::Cancellable::NONE,
            )
            .ok()
            .and_then(|reply| reply.get::<(bool,)>())
            .is_some_and(|(owned,)| owned);
        if !owned {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Starts a new instance with `--relaunch`; the caller quits once this succeeds.
pub fn relaunch() -> io::Result<()> {
    // Settings are written asynchronously; make sure the new process reads the new language.
    gio::Settings::sync();
    let mut command = Command::new(std::env::current_exe()?);
    command.arg(format!("--{RELAUNCH_OPTION}"));
    let saved_system = std::env::var(SYSTEM_LANGUAGE_VARIABLE).ok();
    apply(&mut command, &relaunch_environment(saved_system.as_deref()));
    command.spawn().map(drop)
}

fn apply(command: &mut Command, changes: &[EnvironmentChange]) {
    for (name, value) in changes {
        match value {
            Some(value) => command.env(name, value),
            None => command.env_remove(name),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// GSettings with a memory backend (never reads or writes the user's dconf).
    fn memory_settings() -> gio::Settings {
        gio::Settings::with_backend(config::APP_ID, &gio::memory_settings_backend_new())
    }

    #[test]
    fn codes_round_trip() {
        for language in AppLanguage::ALL {
            assert_eq!(AppLanguage::from_code(language.code()), language);
        }
        assert_eq!(AppLanguage::System.code(), "");
        assert_eq!(AppLanguage::TraditionalChinese.code(), "zh_TW");
    }

    #[test]
    fn unknown_codes_follow_the_system() {
        for code in ["fr", "zh_CN", "EN", " ja", "zh-Hant"] {
            assert_eq!(AppLanguage::from_code(code), AppLanguage::System, "{code}");
        }
    }

    #[test]
    fn native_names_are_not_translated() {
        let names: Vec<_> = AppLanguage::ALL
            .iter()
            .map(|language| language.native_name())
            .collect();
        assert_eq!(
            names,
            [
                None,
                Some("English"),
                Some("繁體中文"),
                Some("日本語"),
                Some("한국어")
            ]
        );
    }

    #[test]
    fn settings_default_to_the_system_and_round_trip() {
        let settings = memory_settings();
        assert_eq!(AppLanguage::read(&settings), AppLanguage::System);
        for language in AppLanguage::ALL {
            language.write(&settings).unwrap();
            assert_eq!(AppLanguage::read(&settings), language);
        }
        settings
            .child("display")
            .set_string("language", "fr")
            .unwrap();
        assert_eq!(AppLanguage::read(&settings), AppLanguage::System);
    }

    #[test]
    fn following_the_system_never_changes_the_environment() {
        for current in [None, Some("ja"), Some("zh_TW:zh")] {
            assert_eq!(
                startup_environment(AppLanguage::System, current, None),
                None
            );
        }
    }

    #[test]
    fn a_chosen_language_sets_language_and_keeps_the_system_value() {
        assert_eq!(
            startup_environment(AppLanguage::Japanese, Some("zh_TW:zh"), None),
            Some(vec![
                ("LANGUAGE", Some("ja".into())),
                (SYSTEM_LANGUAGE_VARIABLE, Some("zh_TW:zh".into())),
            ])
        );
        // Not set by the system: saved as empty (gettext treats an empty LANGUAGE as unset).
        assert_eq!(
            startup_environment(AppLanguage::Korean, None, None),
            Some(vec![
                ("LANGUAGE", Some("ko".into())),
                (SYSTEM_LANGUAGE_VARIABLE, Some(String::new())),
            ])
        );
    }

    #[test]
    fn already_running_in_the_chosen_language_needs_no_change() {
        // After the exec: no loop.
        assert_eq!(
            startup_environment(AppLanguage::Japanese, Some("ja"), Some("")),
            None
        );
    }

    #[test]
    fn the_first_saved_system_value_is_kept() {
        assert_eq!(
            startup_environment(AppLanguage::English, Some("ja"), Some("ko")),
            Some(vec![
                ("LANGUAGE", Some("en".into())),
                (SYSTEM_LANGUAGE_VARIABLE, Some("ko".into())),
            ])
        );
    }

    #[test]
    fn relaunch_restores_the_system_language() {
        assert_eq!(
            relaunch_environment(Some("zh_TW:zh")),
            vec![
                ("LANGUAGE", Some("zh_TW:zh".into())),
                (SYSTEM_LANGUAGE_VARIABLE, None),
            ]
        );
        assert_eq!(
            relaunch_environment(Some("")),
            vec![("LANGUAGE", None), (SYSTEM_LANGUAGE_VARIABLE, None)]
        );
        // Not changed by the app: the environment is already the system's.
        assert_eq!(relaunch_environment(None), vec![]);
    }
}
