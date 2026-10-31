//! The characters the keys of the user's keyboard layout type.
//!
//! A custom shortcut is stored as a physical key, a USB HID usage, so it means
//! the same key on every layout and the config text never changes with it.
//! What the key *prints* does change: the key a US keyboard prints `[` on prints
//! `ü` on a German one, and the one printing `Y` prints `Z`. A settings UI that
//! records and shows shortcuts needs that mapping; a process that only sends
//! them does not. The UI process reads its layout once and installs it here.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use super::key_combo::KeyboardUsage;

/// What each key types without modifiers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeyLayout {
    typed: BTreeMap<u8, char>,
}

impl KeyLayout {
    /// A layout from the character each key types.
    pub fn new(typed: impl IntoIterator<Item = (KeyboardUsage, char)>) -> Self {
        Self {
            typed: typed
                .into_iter()
                .map(|(usage, typed)| (usage.code(), typed))
                .collect(),
        }
    }

    /// The character `usage` types, or `None` for a key that types none.
    #[must_use]
    pub fn typed_by(&self, usage: KeyboardUsage) -> Option<char> {
        self.typed.get(&usage.code()).copied()
    }

    /// The key that types `typed`. Matching ignores case.
    #[must_use]
    pub fn usage_for(&self, typed: char) -> Option<KeyboardUsage> {
        let wanted = typed.to_lowercase().next()?;
        self.typed
            .iter()
            .find(|(_, candidate)| candidate.to_lowercase().next() == Some(wanted))
            .and_then(|(code, _)| KeyboardUsage::try_from(*code).ok())
    }
}

static INSTALLED: OnceLock<KeyLayout> = OnceLock::new();

/// Install the process's layout. The first call wins; later ones are ignored,
/// because a layout read once at startup is the only owner.
pub fn install_key_layout(layout: KeyLayout) {
    let _ = INSTALLED.set(layout);
}

/// The installed layout, or `None` in a process that installed none.
#[must_use]
pub fn installed_key_layout() -> Option<&'static KeyLayout> {
    INSTALLED.get()
}

pub(super) fn installed() -> Option<&'static KeyLayout> {
    installed_key_layout()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage(code: u8) -> KeyboardUsage {
        KeyboardUsage::try_from(code).expect("supported usage")
    }

    #[test]
    fn a_layout_finds_the_key_that_types_a_character_in_either_case() {
        // A German keyboard: ü on the US `[` key, Z on the US `Y` key.
        let layout = KeyLayout::new([(usage(0x2f), 'ü'), (usage(0x1c), 'z')]);
        assert_eq!(layout.usage_for('ü'), Some(usage(0x2f)));
        assert_eq!(layout.usage_for('Ü'), Some(usage(0x2f)));
        assert_eq!(layout.usage_for('Z'), Some(usage(0x1c)));
        assert_eq!(layout.typed_by(usage(0x2f)), Some('ü'));
        assert_eq!(layout.usage_for('ö'), None);
        assert_eq!(layout.typed_by(usage(0x04)), None);
    }
}
