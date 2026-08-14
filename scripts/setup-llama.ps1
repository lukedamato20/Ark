# setup-llama.ps1
# Installs the reviewed llama.cpp built-in runtime artifact for Windows.
# Run once from the repo root before starting the dev environment:
#   .\scripts\setup-llama.ps1

$ErrorActionPreference = "Stop"
& node "$PSScriptRoot\runtime-supply-chain.mjs" install
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
