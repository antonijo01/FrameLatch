# Builds dist\FrameLatch-Setup-<version>.exe: release executable + Inno Setup installer.
# Requires Inno Setup 7 (ISCC.exe). Set $env:ISCC to override its location.
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$version = (Select-String -Path Cargo.toml -Pattern '^version\s*=\s*"([^"]+)"' |
    Select-Object -First 1).Matches[0].Groups[1].Value
if (-not $version) { throw 'Could not read the version from Cargo.toml.' }

$candidates = @(
    $env:ISCC,
    "$env:LOCALAPPDATA\Programs\Inno Setup 7\ISCC.exe",
    "${env:ProgramFiles}\Inno Setup 7\ISCC.exe",
    "${env:ProgramFiles(x86)}\Inno Setup 7\ISCC.exe",
    "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe",
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"
)
$iscc = $candidates | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
if (-not $iscc) { throw 'ISCC.exe not found. Install Inno Setup 7 or set $env:ISCC.' }

$cargo = (Get-Command cargo -ErrorAction SilentlyContinue).Source
if (-not $cargo) { $cargo = "$env:USERPROFILE\.cargo\bin\cargo.exe" }
if (-not (Test-Path $cargo)) { throw 'cargo not found. Install Rust (rustup) first.' }
& $cargo build --release
if ($LASTEXITCODE -ne 0) { throw 'cargo build failed.' }

& $iscc "/DAppVersion=$version" installer\framelatch.iss
if ($LASTEXITCODE -ne 0) {
    # Typically antivirus blocking the setup-icon resource update.
    Write-Warning 'Compilation with the custom setup icon failed; retrying without it.'
    & $iscc "/DAppVersion=$version" /DNoSetupIcon installer\framelatch.iss
    if ($LASTEXITCODE -ne 0) { throw 'Inno Setup compilation failed.' }
}

Get-Item "dist\FrameLatch-Setup-$version.exe" | Select-Object FullName, Length
