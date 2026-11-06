//! Wine and Proton windows that carry no identity of their own.
//!
//! A launcher that starts every game through one Proton prefix (Lutris and
//! `umu-run` without a per-game id) gives every window the same class,
//! `steam_app_default`, so a per-app profile could never tell World of Warcraft
//! from another game. The process behind such a window does: its command line
//! starts with the Windows path of the executable. That path is the identifier,
//! in the same lower-cased form the Windows backend reports, so a profile keyed
//! `exe:wow.exe` matches it through `openlogi_core::app::overlay_for`.

use openlogi_core::app::executable_selector;

/// Classes that name a launcher's shared prefix rather than one application.
const GENERIC_CLASSES: [&str; 1] = ["steam_app_default"];

/// Class prefixes of launchers that derive the class from a per-prefix id, so
/// every program started in that prefix (the Battle.net client and the game it
/// launches alike) shares one class. Faugus uses `steam_app_faugus<hash>`.
const GENERIC_CLASS_PREFIXES: [&str; 1] = ["steam_app_faugus"];

pub(super) fn is_generic_class(class: &str) -> bool {
    GENERIC_CLASSES.contains(&class)
        || GENERIC_CLASS_PREFIXES
            .iter()
            .any(|prefix| class.starts_with(prefix))
}

/// The lower-cased Windows executable path the process `pid` was started
/// with, or `None` when it was not started from an `.exe` path.
pub(super) fn executable_of(pid: u32) -> Option<String> {
    let cmdline = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    executable_from_cmdline(&cmdline)
}

/// What a person calls the program an identifier [`executable_of`] produced:
/// its file name without `.exe`, in the lower case the identifier is in.
pub(super) fn program_name(identifier: &str) -> Option<String> {
    executable_selector(identifier)?;
    let (_, file) = identifier.rsplit_once(['\\', '/'])?;
    Some(file.strip_suffix(".exe").unwrap_or(file).to_string())
}

fn executable_from_cmdline(cmdline: &[u8]) -> Option<String> {
    let program = std::str::from_utf8(cmdline.split(|byte| *byte == 0).next()?).ok()?;
    executable_selector(program)?;
    Some(program.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_windows_program_path_is_the_identifier() {
        let cmdline =
            b"C:\\Blizzard\\World of Warcraft\\_retail_\\WoW.exe\0-launcherlogin\0-uid\0wow\0";
        assert_eq!(
            executable_from_cmdline(cmdline).as_deref(),
            Some("c:\\blizzard\\world of warcraft\\_retail_\\wow.exe")
        );
    }

    #[test]
    fn a_process_not_started_from_an_exe_path_has_no_identifier() {
        assert_eq!(executable_from_cmdline(b"/usr/bin/firefox\0"), None);
        assert_eq!(executable_from_cmdline(b"wine64-preloader\0"), None);
        assert_eq!(executable_from_cmdline(b""), None);
    }

    #[test]
    fn a_program_is_named_by_its_file_without_the_extension() {
        assert_eq!(
            program_name("c:\\blizzard\\world of warcraft\\_retail_\\wow.exe").as_deref(),
            Some("wow")
        );
        assert_eq!(program_name("firefox"), None);
        assert_eq!(program_name("org.mozilla.firefox"), None);
    }

    #[test]
    fn only_the_shared_prefix_class_is_generic() {
        assert!(is_generic_class("steam_app_default"));
        assert!(is_generic_class("steam_app_faugus5dd8f7d6"));
        assert!(!is_generic_class("steam_app_2679100"));
        assert!(!is_generic_class("wow.exe"));
    }
}
