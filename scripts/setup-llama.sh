#!/usr/bin/env bash
# setup-llama.sh
# Installs the reviewed llama.cpp built-in runtime artifact for macOS or Linux.
# Run once from the repo root before starting the dev environment:
#   bash scripts/setup-llama.sh

set -euo pipefail
exec node "$(dirname "$0")/runtime-supply-chain.mjs" install
