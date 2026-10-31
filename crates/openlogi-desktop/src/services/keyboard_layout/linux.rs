//! The Linux layout, read from the same place the compositor reads it and
//! compiled with xkbcommon, which says what each key types.

use std::path::PathBuf;

use openlogi_core::binding::{KeyLayout, KeyboardUsage, install_key_layout};
use tracing::{debug, info};
use xkbcommon::xkb;

/// evdev codes and xkb keycodes differ by a fixed offset.
const XKB_KEYCODE_OFFSET: u32 = 8;

/// The XKB names a keymap is compiled from: rules, model, layout, variant,
/// options. Empty names stand for the library's defaults.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Rmlvo {
    rules: String,
    model: String,
    layout: String,
    variant: String,
    options: Option<String>,
}

pub(super) fn install() {
    let Some(names) = detect_names() else {
        debug!("keyboard layout: none configured, keeping US key names");
        return;
    };
    let Some(layout) = layout_for(&names) else {
        debug!(?names, "keyboard layout: could not compile a keymap");
        return;
    };
    info!(layout = %names.layout, variant = %names.variant, "keyboard layout installed");
    install_key_layout(layout);
}

/// The layout names, from the most specific source that sets them: the
/// environment, the COSMIC compositor's own setting, then the system default.
fn detect_names() -> Option<Rmlvo> {
    from_environment()
        .or_else(|| cosmic_config_path().and_then(|path| read_names(&path, parse_cosmic)))
        .or_else(|| read_names(&PathBuf::from("/etc/default/keyboard"), parse_etc_default))
}

fn from_environment() -> Option<Rmlvo> {
    let layout = std::env::var("XKB_DEFAULT_LAYOUT").ok()?;
    let var = |name: &str| std::env::var(name).unwrap_or_default();
    Some(Rmlvo {
        rules: var("XKB_DEFAULT_RULES"),
        model: var("XKB_DEFAULT_MODEL"),
        layout,
        variant: var("XKB_DEFAULT_VARIANT"),
        options: std::env::var("XKB_DEFAULT_OPTIONS").ok(),
    })
}

fn cosmic_config_path() -> Option<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(config.join("cosmic/com.system76.CosmicComp/v1/xkb_config"))
}

fn read_names(path: &std::path::Path, parse: fn(&str) -> Option<Rmlvo>) -> Option<Rmlvo> {
    parse(&std::fs::read_to_string(path).ok()?)
}

/// COSMIC's `xkb_config`, a RON struct: `layout: "de",`, `options: Some("…"),`.
fn parse_cosmic(text: &str) -> Option<Rmlvo> {
    let field = |name: &str| {
        text.lines().find_map(|line| {
            let value = line.trim().strip_prefix(name)?.strip_prefix(':')?;
            let value = value.trim().trim_end_matches(',').trim();
            let value = value
                .strip_prefix("Some(")
                .and_then(|inner| inner.strip_suffix(')'))
                .unwrap_or(value);
            value
                .strip_prefix('"')
                .and_then(|quoted| quoted.strip_suffix('"'))
                .map(str::to_string)
        })
    };
    Some(Rmlvo {
        rules: field("rules").unwrap_or_default(),
        model: field("model").unwrap_or_default(),
        layout: field("layout").filter(|layout| !layout.is_empty())?,
        variant: field("variant").unwrap_or_default(),
        options: field("options"),
    })
}

/// `/etc/default/keyboard`: shell-style `XKBLAYOUT="de"` assignments.
fn parse_etc_default(text: &str) -> Option<Rmlvo> {
    let field = |name: &str| {
        text.lines().find_map(|line| {
            let value = line.trim().strip_prefix(name)?.strip_prefix('=')?;
            Some(value.trim().trim_matches('"').to_string())
        })
    };
    Some(Rmlvo {
        rules: String::new(),
        model: field("XKBMODEL").unwrap_or_default(),
        layout: field("XKBLAYOUT").filter(|layout| !layout.is_empty())?,
        variant: field("XKBVARIANT").unwrap_or_default(),
        options: field("XKBOPTIONS").filter(|options| !options.is_empty()),
    })
}

/// What every key types without modifiers under `names`' first layout.
fn layout_for(names: &Rmlvo) -> Option<KeyLayout> {
    let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
    let keymap = xkb::Keymap::new_from_names(
        &context,
        &names.rules,
        &names.model,
        &names.layout,
        &names.variant,
        names.options.clone(),
        xkb::KEYMAP_COMPILE_NO_FLAGS,
    )?;
    let typed = (0..=u8::MAX)
        .filter_map(|code| KeyboardUsage::try_from(code).ok())
        .filter_map(|usage| {
            let keycode = xkb::Keycode::new(u32::from(usage.evdev_code()?) + XKB_KEYCODE_OFFSET);
            let symbol = *keymap.key_get_syms_by_level(keycode, 0, 0).first()?;
            let typed = char::from_u32(xkb::keysym_to_utf32(symbol))?;
            (!typed.is_control() && !typed.is_whitespace()).then_some((usage, typed))
        });
    Some(KeyLayout::new(typed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage(code: u8) -> KeyboardUsage {
        KeyboardUsage::try_from(code).expect("supported usage")
    }

    fn german() -> Rmlvo {
        Rmlvo {
            layout: "de".into(),
            ..Rmlvo::default()
        }
    }

    #[test]
    fn cosmic_settings_name_the_layout() {
        let text = "(\n    rules: \"\",\n    model: \"\",\n    layout: \"de\",\n    variant: \"nodeadkeys\",\n    options: Some(\"caps:escape\"),\n    repeat_delay: 600,\n)";
        assert_eq!(
            parse_cosmic(text),
            Some(Rmlvo {
                layout: "de".into(),
                variant: "nodeadkeys".into(),
                options: Some("caps:escape".into()),
                ..Rmlvo::default()
            })
        );
        assert_eq!(parse_cosmic("(layout: \"\",)"), None, "no layout set");
        assert_eq!(parse_cosmic("(options: None,)"), None);
    }

    #[test]
    fn the_system_default_names_the_layout() {
        let text = "XKBMODEL=\"pc105\"\nXKBLAYOUT=\"de\"\nXKBVARIANT=\"\"\nXKBOPTIONS=\"\"\n";
        assert_eq!(
            parse_etc_default(text),
            Some(Rmlvo {
                model: "pc105".into(),
                layout: "de".into(),
                ..Rmlvo::default()
            })
        );
        assert_eq!(parse_etc_default("XKBMODEL=\"pc105\"\n"), None);
    }

    #[test]
    fn a_german_keymap_types_umlauts_and_swaps_y_and_z() {
        let Some(layout) = layout_for(&german()) else {
            return; // xkeyboard-config is not installed on this machine
        };
        assert_eq!(layout.usage_for('ü'), Some(usage(0x2f)));
        assert_eq!(layout.usage_for('ö'), Some(usage(0x33)));
        assert_eq!(layout.usage_for('ä'), Some(usage(0x34)));
        assert_eq!(layout.usage_for('ß'), Some(usage(0x2d)));
        assert_eq!(layout.usage_for('z'), Some(usage(0x1c)));
        assert_eq!(layout.usage_for('y'), Some(usage(0x1d)));
        assert_eq!(layout.usage_for('<'), Some(usage(0x64)));
    }
}
