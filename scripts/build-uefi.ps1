# Build the UEFI application -> output\NowWrite.efi
# -CreateBoot also lays out output\boot\EFI\BOOT\BOOTX64.EFI for a FAT32 drive.
param([switch]$CreateBoot)

rustup target add x86_64-unknown-uefi
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

New-Item -ItemType Directory -Force output | Out-Null
cargo build --release --bin nowwrite_uefi --target x86_64-unknown-uefi --no-default-features --features uefi
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Copy-Item target\x86_64-unknown-uefi\release\nowwrite_uefi.efi output\NowWrite.efi -Force -ErrorAction Stop
Write-Host "UEFI build complete: output\NowWrite.efi"

if ($CreateBoot) {
    New-Item -ItemType Directory -Force output\boot\EFI\BOOT | Out-Null
    Copy-Item output\NowWrite.efi output\boot\EFI\BOOT\BOOTX64.EFI -Force -ErrorAction Stop
    # The embedded JetBrains Mono font's licence travels with the binary.
    Copy-Item assets\fonts\OFL.txt output\boot\EFI\BOOT\OFL.txt -Force -ErrorAction Stop
    Write-Host "Boot layout created in output\boot\ (copy its contents to a FAT32 drive)"
}
