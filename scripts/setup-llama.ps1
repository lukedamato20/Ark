# setup-llama.ps1
# Downloads the llama.cpp built-in runtime binaries for Windows.
# Run once from the repo root before starting the dev environment:
#   .\scripts\setup-llama.ps1

$ErrorActionPreference = "Stop"

$release = "b9859"
$dest = "$PSScriptRoot\..\src-tauri\binaries\llama"
$tmp = "$env:TEMP\ark-llama-setup"

New-Item -ItemType Directory -Force $dest | Out-Null
New-Item -ItemType Directory -Force $tmp | Out-Null

# Detect architecture
$arch = if ([System.Environment]::Is64BitOperatingSystem) {
    if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { "arm64" } else { "x64" }
} else { "x64" }

$zipName = "llama-$release-bin-win-cpu-$arch.zip"
$url = "https://github.com/ggerganov/llama.cpp/releases/download/$release/$zipName"
$zipPath = "$tmp\$zipName"

Write-Host "Downloading llama.cpp $release (Windows CPU $arch)..."
Invoke-WebRequest -Uri $url -OutFile $zipPath -UseBasicParsing
Write-Host "Downloaded. Extracting..."

$extracted = "$tmp\extracted"
Remove-Item -Recurse -Force $extracted -ErrorAction SilentlyContinue
Expand-Archive -Path $zipPath -DestinationPath $extracted -Force

# Copy everything except unneeded tools (keep server + all deps)
$keep = @("llama-server.exe", "*.dll", "ggml-rpc-server.exe")
$srcDir = Get-ChildItem $extracted -Directory | Select-Object -First 1

Get-ChildItem $srcDir.FullName | Where-Object {
    $n = $_.Name
    ($n -eq "llama-server.exe") -or ($n -like "*.dll") -or ($n -eq "ggml-rpc-server.exe")
} | ForEach-Object {
    Copy-Item $_.FullName "$dest\$($_.Name)" -Force
}

Write-Host ""
Write-Host "Done. Files in src-tauri/binaries/llama/:"
Get-ChildItem $dest | Where-Object { $_.Name -ne ".gitkeep" } |
    Format-Table Name, @{n='KB';e={[math]::Round($_.Length/1KB,0)}}
Write-Host "Built-in runtime ready. Run 'pnpm tauri:dev' to start Ark."
