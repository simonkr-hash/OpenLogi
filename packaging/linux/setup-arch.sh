#!/bin/sh
# Fresh Arch / CachyOS machine to a working OpenLogi fork in one step:
# build tools and libraries, a Rust toolchain, then the fork itself.
#
#   sh -c "$(curl -fsSL https://raw.githubusercontent.com/simonkr-hash/OpenLogi/g502x-plus-port/packaging/linux/setup-arch.sh)"
#
# Run as your normal user (it calls sudo for pacman). Extra arguments are
# passed on to install-fork.sh (--no-build, --keep-package).

set -eu

if [ "$(id -u)" -eq 0 ]; then
  echo "Run this as your normal user, not root." >&2
  exit 1
fi

if ! command -v pacman >/dev/null 2>&1; then
  echo "This script is for Arch-based systems (pacman not found)." >&2
  exit 1
fi

echo "==> Installing build tools and libraries"
sudo pacman -S --needed --noconfirm \
  git curl base-devel clang pkgconf rustup \
  systemd-libs fontconfig freetype2 openssl zstd \
  wayland libxkbcommon libxkbcommon-x11 libx11 libxcb \
  vulkan-icd-loader

echo "==> Setting up the Rust toolchain"
rustup default stable
# rustup puts cargo in ~/.cargo/bin; make it visible to this script.
PATH="$HOME/.cargo/bin:$PATH"
export PATH
cargo --version

BOOTSTRAP=https://raw.githubusercontent.com/simonkr-hash/OpenLogi/g502x-plus-port/packaging/linux/bootstrap-fork.sh
echo "==> Fetching and installing the fork"
exec sh -c "$(curl -fsSL "$BOOTSTRAP")" bootstrap-fork "$@"
