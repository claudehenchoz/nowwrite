# Build the terminal version (Windows) -> output\nowwrite.exe
New-Item -ItemType Directory -Force output | Out-Null

cargo build --release --bin nowwrite
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Copy-Item target\release\nowwrite.exe output\nowwrite.exe -Force -ErrorAction Stop
Write-Host "Terminal build complete: output\nowwrite.exe"
