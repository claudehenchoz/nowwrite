$ErrorActionPreference = 'Continue'

Write-Host "Building NowWrite for Windows..."

New-Item -ItemType Directory -Force output | Out-Null

cargo build --release --bin nowwrite_windows --no-default-features --features windows
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Copy-Item target\release\nowwrite_windows.exe output\nowwrite-windows.exe -Force -ErrorAction Stop

Write-Host "Windows build complete: output\nowwrite-windows.exe"
Write-Host "Run it from Windows Terminal or a console window: .\output\nowwrite-windows.exe"
