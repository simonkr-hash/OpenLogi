# Fork notes: G502 X PLUS on Linux (COSMIC / Wayland)

This fork of OpenLogi adds what a Logitech **G502 X PLUS** (Lightspeed receiver,
`wpid 4099`) needs on a Linux desktop. It is kept separate from `README.md` so
upstream merges stay clean. **Personal use only**: it pulls in a GPL-3.0
dependency (see below), so it is not upstreamable as-is.

## What is different from upstream

- **Extra buttons** via the `0x8110` spy (upstream PR #1350): back, forward,
  DPI shift key, DPI up (G8) / down (G7), profile cycle.
- **Wheel tilt left/right** can be remapped (`WheelTiltLeft`, `WheelTiltRight`).
- **GUI**: a G502-shaped drawing with matching button hotspots, including the
  two tilt zones (`crates/openlogi-desktop/src/features/mouse/g502.rs`). Binding
  an arbitrary key uses the original recorder in the button picker.
- **Per-app profiles on COSMIC**: a `cosmic-toplevel` frontmost-window backend
  (`crates/openlogi-hook/src/linux/foreground/cosmic_toplevel.rs`), because `cosmic-comp`
  does not offer the wlroots foreign-toplevel protocol. Profile keys are the
  Wayland `app_id` (e.g. `org.mozilla.firefox`), not the X11 `WM_CLASS`.

## Install (Arch / CachyOS, systemd)

```sh
sudo pacman -S --needed rustup base-devel pkgconf systemd-libs
rustup default stable
git clone -b g502x-plus-port https://github.com/simonkr-hash/OpenLogi
cd OpenLogi
sh packaging/linux/install-fork.sh
```

The script stops running instances, offers to remove a pacman package that owns
`/usr/bin/openlogi-agent`, builds the four binaries, runs `install.sh
--prefix=/usr` and enables `openlogi-agent.service` for your user. The first
build takes about ten minutes. Your config in `~/.config/openlogi` is not
touched. Remove everything with `sh packaging/linux/uninstall.sh`.

Status: the script has been run end to end on CachyOS (COSMIC, Wayland) with a
G502 X PLUS; the agent starts at login and the GUI opens from the menu.

## Useful details

- The agent reads its log level from `OPENLOGI_LOG` (not `RUST_LOG`):
  `OPENLOGI_LOG=debug openlogi-agent`.
- To see which window backend was chosen:
  `OPENLOGI_LOG=debug openlogi-agent 2>&1 | grep -i frontmost` should show
  `using 'cosmic-toplevel' backend` on COSMIC.
- A distro package and this build use different protocol versions; do not run
  both. The install script removes the package for that reason.

## Known limits

- The mouse drawing is hand-made and approximate.
- Custom shortcuts support letters, digits, most punctuation, F1-F24, the
  navigation block and arrows. No numpad, Insert, Print or a bare Super key.
- The COSMIC backend binds `zcosmic_toplevel_info_v1` at version 1 (deprecated
  but working). If COSMIC drops it, move to the version 2+ flow with
  `ext_foreign_toplevel_list`.
- Left, right and middle click are not remappable (lock-out risk).

## Licence note

`cosmic-protocols` is GPL-3.0-only. Builds of this fork are therefore
effectively GPL-3.0. Before sending anything upstream, replace it (for example
with bindings generated from the protocol XML) or drop the COSMIC backend.
