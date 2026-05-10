#!/bin/bash
set -euxo pipefail

export DEBIAN_FRONTEND=noninteractive

PACKAGES="ca-certificates curl git build-essential"

# Usage: install-deps.sh [PKG1] [PKG2] ...
while [[ $# -gt 0 ]]; do
  PACKAGES="$PACKAGES $1"
  shift
done

apt-get update
apt-get install -y --no-install-recommends $PACKAGES
rm -rf /var/lib/apt/lists/*
