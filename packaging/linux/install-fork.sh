#!/bin/sh
# One-step build + install of this fork on a Linux desktop (written for Arch/CachyOS).
#
# Usage, from anywhere inside a checkout:
#   sh packaging/linux/install-fork.sh [--no-build] [--keep-package]
#
# What it does:
#   1. stops any running OpenLogi agent / overlay / GUI
#   2. offers to remove a distro package that owns /usr/bin/openlogi-agent
#      (otherwise its files would be overwritten behind the package manager)
#   3. builds the four binaries in release mode (skipped with --no-build)
#   4. runs install.sh --from-source --prefix /usr (binaries, udev rules, user
#      unit, desktop entry, icons) and starts the agent for the current user
#
# Your config in ~/.config/openlogi is left alone.

set -eu

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
BUILD=1
KEEP_PACKAGE=0

for arg in "$@"; do
  case "$arg" in
    --no-build) BUILD=0 ;;
    --keep-package) KEEP_PACKAGE=1 ;;
    --help | -h)
      sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *)
      echo "Unknown argument: $arg" >&2
      exit 1
      ;;
  esac
done

if [ "$(id -u)" -eq 0 ]; then
  echo "Run this as your normal user, not root (it calls sudo where needed)." >&2
  exit 1
fi

cd "$REPO_ROOT"

# ── 1. stop what is running ──────────────────────────────────────────────────

echo "==> Stopping running OpenLogi processes"
systemctl --user disable --now openlogi-agent.service 2>/dev/null || true
pkill -f 'openlogi-(agent|overlay|desktop)' 2>/dev/null || true
sleep 1

# ── 2. a distro package would be overwritten ─────────────────────────────────

if [ "$KEEP_PACKAGE" -eq 0 ] && command -v pacman >/dev/null 2>&1; then
  OWNER="$(pacman -Qqo /usr/bin/openlogi-agent 2>/dev/null || true)"
  if [ -n "$OWNER" ]; then
    echo "==> /usr/bin/openlogi-agent belongs to the pacman package '$OWNER'."
    printf "    Remove it before installing this build? [y/N] "
    read -r answer
    case "$answer" in
      y | Y | yes | YES) sudo pacman -Rns "$OWNER" ;;
      *)
        echo "    Keeping it; re-run with --keep-package to skip this question." >&2
        echo "    Installing over a package leaves pacman out of sync, aborting." >&2
        exit 1
        ;;
    esac
  fi
fi

# ── 3. build ─────────────────────────────────────────────────────────────────

if [ "$BUILD" -eq 1 ]; then
  if ! command -v cargo >/dev/null 2>&1; then
    echo "cargo not found. On Arch: sudo pacman -S --needed rustup base-devel pkgconf && rustup default stable" >&2
    exit 1
  fi
  echo "==> Building (the GUI takes several minutes the first time)"
  cargo build --release -p openlogi -p openlogi-desktop -p openlogi-overlay -p openlogi-agent
fi

# ── 4. install and start ─────────────────────────────────────────────────────

# The release installer's source mode copies the binaries, udev rules, user
# unit, desktop entry and icons, then enables and starts the agent.
echo "==> Installing to /usr"
sh "$SCRIPT_DIR/install.sh" --from-source --prefix /usr

# On a fresh system the udev rule only reaches /dev/uinput after the next
# boot or login, and the agent just started without it. Grant the access now
# so the buttons work immediately; the rule keeps it after a reboot.
if [ -e /dev/uinput ] && [ ! -w /dev/uinput ] && command -v setfacl >/dev/null 2>&1; then
  echo "==> Granting $USER access to /dev/uinput for this session"
  sudo setfacl -m "u:$USER:rw" /dev/uinput || true
  systemctl --user restart openlogi-agent.service || true
fi

echo ""
echo "Done. Open OpenLogi from the application menu, or run: openlogi-desktop"
echo "If the mouse is not picked up right away, log out and back in once so the"
echo "new udev rules apply to your session."
