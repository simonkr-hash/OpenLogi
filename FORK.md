# Fork notes: G502 X PLUS on Linux (COSMIC / Wayland)

This fork of OpenLogi adds what a Logitech **G502 X PLUS** (Lightspeed receiver,
`wpid 4099`) needs on a Linux desktop. It is kept separate from `README.md` so
upstream merges stay clean. **Personal use only**: it pulls in a GPL-3.0
dependency (see below), so it is not upstreamable as-is.

## What is different from upstream

- **Extra buttons** via the `0x8110` spy (upstream PR #1350): back, forward,
  DPI shift (G8), DPI up/down, profile cycle.
- **Wheel tilt left/right** can be remapped (`WheelTiltLeft`, `WheelTiltRight`).
- **GUI**: a G502-shaped drawing with matching button hotspots (including the
  two tilt zones), and a free-form **Custom shortcut** field in the action
  picker, so any key (e.g. `9`) can be bound to a mouse button.
- **Per-app profiles on COSMIC**: a `cosmic-toplevel` frontmost-window backend
  (`crates/openlogi-hook/src/linux/cosmic_toplevel.rs`), because `cosmic-comp`
  does not offer the wlroots foreign-toplevel protocol. Profile keys are the
  Wayland `app_id` (e.g. `org.mozilla.firefox`), not the X11 `WM_CLASS`.

## Install (Arch / CachyOS, systemd)

```sh
sudo pacman -S --needed rustup base-devel pkgconf systemd-libs
rustup default stable
git clone -b g502x-plus-buttons https://github.com/simonkr-hash/OpenLogi
cd OpenLogi
sh packaging/linux/install-fork.sh
```

The script stops running instances, offers to remove a pacman package that owns
`/usr/bin/openlogi-agent`, builds the four binaries, runs `install.sh
--prefix=/usr` and enables `openlogi-agent.service` for your user. The first
build takes about ten minutes. Your config in `~/.config/openlogi` is not
touched. Remove everything with `sh packaging/linux/uninstall.sh`.

Status: written for Arch/CachyOS and run there by hand step by step; the script
itself is only syntax-checked.

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
