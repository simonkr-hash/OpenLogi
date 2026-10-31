//! The installed keyboard layout is process-wide, so its tests get a process.

#![expect(
    clippy::tests_outside_test_module,
    clippy::expect_used,
    reason = "integration test file; its own process owns the installed layout, and its \
              `usage` helper sits outside any `#[test]` fn"
)]

use openlogi_core::binding::{KeyCombo, KeyLayout, KeyboardUsage, install_key_layout};

fn usage(code: u8) -> KeyboardUsage {
    KeyboardUsage::try_new(code).expect("supported usage")
}

#[test]
fn an_installed_layout_prints_and_parses_its_own_characters() {
    // A German keyboard: ü on the US `[` key, Z on the US `Y` key.
    install_key_layout(KeyLayout::new([(usage(0x2f), 'ü'), (usage(0x1c), 'z')]));

    let umlaut = "Ctrl+ü"
        .parse::<KeyCombo>()
        .expect("layout character parses");
    assert_eq!(umlaut.key(), usage(0x2f));
    assert_eq!(umlaut.display_label(), "Ctrl+Ü");
    assert_eq!(
        umlaut.rendered_label(),
        "Ctrl+[",
        "what is stored does not depend on the layout"
    );

    let y_key = "Y".parse::<KeyCombo>().expect("ascii stays layout-neutral");
    assert_eq!(y_key.key(), usage(0x1c));
    assert_eq!(y_key.display_label(), "Z");
    assert_eq!(y_key.rendered_label(), "Y");

    // A typed or pressed character names the key the layout types it on.
    let pressed = KeyCombo::parse_typed("Shift+z").expect("typed character parses");
    assert_eq!(pressed.key(), usage(0x1c));
    assert_eq!(pressed.display_label(), "Shift+Z");
    let shown = KeyCombo::parse_typed(&pressed.display_label()).expect("display text parses back");
    assert_eq!(shown, pressed, "what the UI shows is what it reads back");

    "ö".parse::<KeyCombo>()
        .expect_err("a character the layout does not type stays unsupported");
}
