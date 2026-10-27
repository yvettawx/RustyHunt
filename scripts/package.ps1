# Builds the release packages: portable zip, installer exe, SHA256SUMS, written to dist\.
#
#   powershell -ExecutionPolicy Bypass -File scripts\package.ps1
#
# Needs Rust (with the wasm32-unknown-unknown target, for the example plugin) and rc.exe from the Windows SDK (for the exe icons).

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$name = "RustyHunt-$version"
$dist = Join-Path $root 'dist'
$stage = Join-Path $dist $name
Write-Host "Packaging $name"

if (Test-Path $dist) { Remove-Item -Recurse -Force $dist }
New-Item -ItemType Directory -Force $stage | Out-Null

# 1. The app, the injected DLL, the theme checker
cargo build --release -p rustyhunt -p rustyhunt-native -p ui-kit
if ($LASTEXITCODE) { throw 'cargo build failed' }
Copy-Item target\release\rustyhunt.exe $stage
Copy-Item target\release\rustyhunt_native.dll (Join-Path $stage 'rustyhunt-native.dll')
Copy-Item target\release\theme-check.exe $stage

# 2. Resources: address maps, copy, docs, examples
Copy-Item -Recurse maps (Join-Path $stage 'maps')
Copy-Item -Recurse locales (Join-Path $stage 'locales')
New-Item -ItemType Directory -Force (Join-Path $stage 'docs') | Out-Null
foreach ($f in 'fields.md', 'events.md', 'theme.md', 'plugins.md', 'theme.xsd') {
    Copy-Item (Join-Path docs $f) (Join-Path $stage 'docs')
}
Copy-Item LICENSE, NOTICE, README.md $stage

$examples = Join-Path $stage 'examples'
New-Item -ItemType Directory -Force (Join-Path $examples 'themes') | Out-Null
Copy-Item -Recurse examples\themes\* (Join-Path $examples 'themes')

# Example plugin: built to wasm and zipped with its manifest and template, ready to install
Push-Location examples\plugins\split-timer
cargo build --release --target wasm32-unknown-unknown
if ($LASTEXITCODE) { Pop-Location; throw 'Example plugin build failed' }
Pop-Location
$plug = Join-Path $dist 'split-timer'
New-Item -ItemType Directory -Force $plug | Out-Null
Copy-Item examples\plugins\split-timer\plugin.toml, examples\plugins\split-timer\split.xml $plug
Copy-Item examples\plugins\split-timer\target\wasm32-unknown-unknown\release\split_timer.wasm (Join-Path $plug 'plugin.wasm')
New-Item -ItemType Directory -Force (Join-Path $examples 'plugins') | Out-Null
Compress-Archive -Path (Join-Path $plug '*') -DestinationPath (Join-Path $examples 'plugins\split-timer.zip')
Remove-Item -Recurse -Force $plug

# 3. Licenses: fonts and third-party crates
$lic = Join-Path $stage 'licenses'
New-Item -ItemType Directory -Force $lic | Out-Null
Copy-Item assets\fonts\OFL.txt (Join-Path $lic 'Montserrat-OFL.txt')
$meta = cargo metadata --format-version 1 --locked | ConvertFrom-Json
$deps = cargo tree -p rustyhunt -e normal --prefix none --format '{p}' | ForEach-Object { ($_ -split ' ')[0..1] -join ' ' } | Sort-Object -Unique
$lines = @("Third-party Rust crates linked into RustyHunt $version", "")
foreach ($p in $meta.packages) {
    if ($p.source -and ($deps -contains "$($p.name) v$($p.version)")) {
        $lines += "$($p.name) $($p.version)  $($p.license)  $($p.repository)"
    }
}
$lines | Set-Content -Encoding utf8 (Join-Path $lic 'THIRD-PARTY.txt')

# 4. Portable build: with a portable marker next to the program, data lives next to the program
New-Item -ItemType File (Join-Path $stage 'portable') | Out-Null
$portable = Join-Path $dist "$name-portable.zip"
Compress-Archive -Path $stage -DestinationPath $portable

# 5. Installer: embed the portable zip into the installer (the portable marker is dropped on install)
$env:RH_PAYLOAD = $portable
cargo build --release -p rustyhunt-setup
$code = $LASTEXITCODE
Remove-Item Env:\RH_PAYLOAD
if ($code) { throw 'Installer build failed' }
$setup = Join-Path $dist "$name-setup.exe"
Copy-Item target\release\rustyhunt-setup.exe $setup

# 6. Address maps are published separately and listed in SHA256SUMS with the packages
Copy-Item maps\*.map $dist
$sums = Get-ChildItem $dist -File | Where-Object { $_.Name -ne 'SHA256SUMS' } | Sort-Object Name | ForEach-Object {
    "$((Get-FileHash -Algorithm SHA256 $_.FullName).Hash.ToLower())  $($_.Name)"
}
$sums | Set-Content -Encoding ascii (Join-Path $dist 'SHA256SUMS')
Remove-Item -Recurse -Force $stage

Write-Host ''
Get-ChildItem $dist | ForEach-Object { '{0,-45} {1,10:N0} KB' -f $_.Name, ($_.Length / 1KB) }
