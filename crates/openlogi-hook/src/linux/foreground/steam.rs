//! The name of a Steam game, for windows that carry only its id.
//!
//! Proton gives a Steam game's window the class `steam_app_<appid>`, which is
//! unique per game and so is the profile key, but it names nothing a person
//! knows. Steam records the game's name in the `appmanifest_<appid>.acf` of the
//! library it is installed in.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

const CLASS_PREFIX: &str = "steam_app_";

/// Names already read, by app id. A game's name does not change under a
/// running Steam, and a miss is not cached: Steam may simply not be mounted yet.
static NAMES: LazyLock<Mutex<HashMap<u32, String>>> = LazyLock::new(Mutex::default);

/// The name of the Steam game whose window class is `class`, or `None` for any
/// other class and for a game no library on this machine holds.
pub(super) fn game_name(class: &str) -> Option<String> {
    let app_id = class.strip_prefix(CLASS_PREFIX)?.parse::<u32>().ok()?;
    let mut names = NAMES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(name) = names.get(&app_id) {
        return Some(name.clone());
    }
    let name = steam_roots()
        .into_iter()
        .flat_map(|root| libraries(&root))
        .find_map(|library| manifest_name(&library, app_id))?;
    names.insert(app_id, name.clone());
    Some(name)
}

/// Where a Steam client keeps its data: the native install and the Flatpak one.
fn steam_roots() -> Vec<PathBuf> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };
    vec![
        home.join(".local/share/Steam"),
        home.join(".steam/steam"),
        home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
    ]
}

/// Every library folder a Steam root knows: itself, then those its
/// `libraryfolders.vdf` lists.
fn libraries(root: &Path) -> Vec<PathBuf> {
    let listed = std::fs::read_to_string(root.join("steamapps/libraryfolders.vdf"))
        .map(|text| library_paths(&text))
        .unwrap_or_default();
    std::iter::once(root.to_path_buf()).chain(listed).collect()
}

fn manifest_name(library: &Path, app_id: u32) -> Option<String> {
    let text = std::fs::read_to_string(library.join(format!("steamapps/appmanifest_{app_id}.acf")))
        .ok()?;
    manifest_game_name(&text)
}

/// The `"path"` entries of a `libraryfolders.vdf`.
fn library_paths(text: &str) -> Vec<PathBuf> {
    text.lines()
        .filter_map(quoted_pair)
        .filter(|(key, _)| *key == "path")
        .map(|(_, value)| PathBuf::from(value.replace("\\\\", "\\")))
        .collect()
}

/// The `"name"` of an `appmanifest_<appid>.acf`.
fn manifest_game_name(text: &str) -> Option<String> {
    text.lines()
        .filter_map(quoted_pair)
        .find(|(key, _)| *key == "name")
        .map(|(_, value)| value.to_string())
        .filter(|name| !name.is_empty())
}

/// `"key"<whitespace>"value"`, the only line shape of Valve's text KeyValues
/// that carries data; section headers and braces have no second quoted string.
fn quoted_pair(line: &str) -> Option<(&str, &str)> {
    let rest = line.trim().strip_prefix('"')?;
    let (key, rest) = rest.split_once('"')?;
    let value = rest.trim().strip_prefix('"')?.strip_suffix('"')?;
    Some((key, value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_manifest_names_its_game() {
        let text = "\"AppState\"\n{\n\t\"appid\"\t\t\"2015240\"\n\t\"name\"\t\t\"Dwarven Realms\"\n\t\"StateFlags\"\t\t\"4\"\n}\n";
        assert_eq!(manifest_game_name(text).as_deref(), Some("Dwarven Realms"));
        assert_eq!(manifest_game_name("\"AppState\"\n{\n}\n"), None);
        assert_eq!(manifest_game_name("\"name\"\t\t\"\"\n"), None);
    }

    #[test]
    fn library_folders_list_their_paths() {
        let text = "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"/home/simon/.local/share/Steam\"\n\t\t\"label\"\t\t\"\"\n\t}\n\t\"1\"\n\t{\n\t\t\"path\"\t\t\"/games/SteamLibrary\"\n\t}\n}\n";
        assert_eq!(
            library_paths(text),
            [
                PathBuf::from("/home/simon/.local/share/Steam"),
                PathBuf::from("/games/SteamLibrary")
            ]
        );
    }

    #[test]
    fn only_a_steam_app_class_names_a_game() {
        assert_eq!(game_name("firefox"), None);
        assert_eq!(game_name("steam_app_default"), None);
        assert_eq!(game_name("steam_app_"), None);
    }
}
