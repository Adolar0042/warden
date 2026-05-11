#!/bin/env bash
set -euxo pipefail

export DEBIAN_FRONTEND=noninteractive

PACKAGES="ca-certificates curl git build-essential"

# Usage: install-deps.sh [PKG1] [PKG2] ...
for dep in "$@"; do
    PACKAGES+=" $dep"
done

apt-get update
apt-get install -y --no-install-recommends $PACKAGES
rm -rf /var/lib/apt/lists/*
