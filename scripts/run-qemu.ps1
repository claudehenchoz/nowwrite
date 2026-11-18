# Boot NowWrite in QEMU from a writable FAT16 disk image.
#
# Your test documents live in output\drive\ (created on first run). Each run:
#   1. copies output\drive\ plus output\NowWrite.efi into output\nowwrite-disk.img,
#   2. boots it in QEMU (the image is a normal writable disk),
#   3. after QEMU exits, copies everything from the image back into output\drive\,
#      so saved documents and .nowwrite\ backups show up on the host.
#
# Uses the edk2 (OVMF) firmware bundled with QEMU for Windows unless -Ovmf
# points at another x64 firmware image. Needs Python 3 for scripts\fatimg.py.
# Install QEMU with: winget install SoftwareFreedomConservancy.QEMU
#
# Extra QEMU arguments can be passed with -QemuArgs, e.g. -QemuArgs '-display','none'.
#
# -Resolution sets the screen size the firmware starts in (QEMU's default is
# 1280x800): a preset name or any WIDTHxHEIGHT, e.g. -Resolution fhd or
# -Resolution 1600x900. -Resolution list prints the presets:
#   vga 640x480, svga 800x600   below 1024x768: NowWrite switches to a larger mode
#   xga 1024x768                smallest size NowWrite keeps as is
#   wxga 1280x800               QEMU's default
#   hd 1360x768                 budget laptop panel (1366x768 ones report 1360)
#   fhd 1920x1080, wuxga 1920x1200
#   qhd 2560x1440               above NowWrite's 1920x1200 switch limit
param(
    [string]$Resolution,
    [string]$Ovmf,
    [int]$SizeMb = 64,
    [string[]]$QemuArgs = @()
)

$presets = [ordered]@{
    vga = "640x480"; svga = "800x600"; xga = "1024x768"; wxga = "1280x800"
    hd = "1360x768"; fhd = "1920x1080"; wuxga = "1920x1200"; qhd = "2560x1440"
}
function Show-Presets {
    foreach ($name in $presets.Keys) { Write-Host ("  {0,-6} {1}" -f $name, $presets[$name]) }
}

# Resolve -Resolution to WIDTHxHEIGHT and the matching VGA device.
$displayArgs = @()
if ($Resolution) {
    if ($Resolution -eq "list") {
        Write-Host "Resolution presets (or pass any WIDTHxHEIGHT):"
        Show-Presets
        exit 0
    }
    if ($presets.Contains($Resolution)) { $Resolution = $presets[$Resolution] }
    if ($Resolution -notmatch '^(\d+)x(\d+)$') {
        Write-Host "Unknown resolution '$Resolution'. Use WIDTHxHEIGHT or one of:"
        Show-Presets
        exit 1
    }
    $xres = [int]$Matches[1]
    $yres = [int]$Matches[2]
    # OVMF rounds the width down to a multiple of 8 but QEMU keeps the
    # requested row length, which skews the picture. Round it ourselves.
    if ($xres % 8 -ne 0) {
        $rounded = $xres - ($xres % 8)
        Write-Host "Note: using width $rounded instead of $xres (QEMU/OVMF need a multiple of 8)."
        $xres = $rounded
    }
    $Resolution = "${xres}x${yres}"
    # The firmware starts in this mode (from the VGA device's EDID). 32 MiB
    # of video memory covers everything up to 2560x1600 with room to spare.
    $displayArgs = @("-vga", "none", "-device", "VGA,vgamem_mb=32,xres=$xres,yres=$yres")
}

$efi = "output\NowWrite.efi"
$drive = "output\drive"
$image = "output\nowwrite-disk.img"
$fatimg = Join-Path $PSScriptRoot "fatimg.py"

if (-not (Test-Path $efi)) {
    Write-Error "Run .\scripts\build-uefi.ps1 first."
    exit 1
}

$python = (Get-Command python -ErrorAction SilentlyContinue).Source
if (-not $python) { $python = (Get-Command py -ErrorAction SilentlyContinue).Source }
if (-not $python) {
    Write-Error "Python 3 is required (for scripts\fatimg.py)."
    exit 1
}

$qemu = (Get-Command qemu-system-x86_64 -ErrorAction SilentlyContinue).Source
if (-not $qemu) { $qemu = "C:\Program Files\qemu\qemu-system-x86_64.exe" }
if (-not (Test-Path $qemu)) {
    Write-Error "qemu-system-x86_64 not found."
    exit 1
}
$share = Join-Path (Split-Path $qemu) "share"

if ($Ovmf) {
    $firmware = @("-bios", $Ovmf)
} else {
    $code = Join-Path $share "edk2-x86_64-code.fd"
    if (-not (Test-Path $code)) {
        Write-Error "No bundled firmware at $code; pass -Ovmf <path to OVMF.fd>."
        exit 1
    }
    # Writable copy of the variable store so boot settings can be saved.
    if (-not (Test-Path output\qemu-vars.fd)) {
        Copy-Item (Join-Path $share "edk2-i386-vars.fd") output\qemu-vars.fd
    }
    $firmware = @(
        "-drive", "if=pflash,format=raw,readonly=on,file=$code",
        "-drive", "if=pflash,format=raw,file=output/qemu-vars.fd"
    )
}

New-Item -ItemType Directory -Force $drive | Out-Null

# If a previous run didn't sync back (e.g. QEMU was killed), recover its changes first.
if (Test-Path $image) {
    & $python $fatimg extract $image $drive --skip EFI
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

& $python $fatimg create $image $drive --efi $efi --size-mb $SizeMb
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

if ($Resolution) {
    Write-Host "Booting NowWrite at $Resolution; documents are synced from/to $drive"
} else {
    Write-Host "Booting NowWrite; documents are synced from/to $drive"
}
& $qemu -machine q35 -m 256M @firmware @displayArgs -drive "format=raw,file=$image" -net none @QemuArgs

& $python $fatimg extract $image $drive --skip EFI
exit $LASTEXITCODE
