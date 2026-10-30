#!/bin/sh
# Fetch (or update) this fork's checkout and install it in one step.
#
#   sh -c "$(curl -fsSL https://raw.githubusercontent.com/simonkr-hash/OpenLogi/master/packaging/linux/bootstrap-fork.sh)"
#
# Pass --no-build or --keep-package through to install-fork.sh if needed.
# OPENLOGI_DIR overrides the checkout location (default: ~/OpenLogi).

set -eu

DIR="${OPENLOGI_DIR:-$HOME/OpenLogi}"
BRANCH=master
URL=https://github.com/simonkr-hash/OpenLogi

if [ -d "$DIR/.git" ]; then
  git -C "$DIR" fetch origin "$BRANCH"
  git -C "$DIR" checkout "$BRANCH"
  git -C "$DIR" pull --ff-only origin "$BRANCH"
else
  git clone -b "$BRANCH" "$URL" "$DIR"
fi

exec sh "$DIR/packaging/linux/install-fork.sh" "$@"
