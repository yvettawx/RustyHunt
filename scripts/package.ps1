# 打发布包：便携版 zip、安装版 exe、SHA256SUMS，输出到 dist\。
#
#   powershell -ExecutionPolicy Bypass -File scripts\package.ps1
#
# 需要：Rust（含 wasm32-unknown-unknown 目标，用来编示例插件）、Windows SDK 的 rc.exe（给 exe 加图标）。

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$name = "RustyHunt-$version"
$dist = Join-Path $root 'dist'
$stage = Join-Path $dist $name
Write-Host "打包 $name"

if (Test-Path $dist) { Remove-Item -Recurse -Force $dist }
New-Item -ItemType Directory -Force $stage | Out-Null

# 1. 程序、注入用的 DLL、主题校验工具
cargo build --release -p rustyhunt -p rustyhunt-native -p ui-kit
if ($LASTEXITCODE) { throw 'cargo build 失败' }
Copy-Item target\release\rustyhunt.exe $stage
Copy-Item target\release\rustyhunt_native.dll (Join-Path $stage 'rustyhunt-native.dll')
Copy-Item target\release\theme-check.exe $stage

# 2. 资源：地址表、文案、文档、示例
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

# 示例插件：编成 wasm，和清单、模板一起打成可以直接安装的 zip
Push-Location examples\plugins\split-timer
cargo build --release --target wasm32-unknown-unknown
if ($LASTEXITCODE) { Pop-Location; throw '示例插件编译失败' }
Pop-Location
$plug = Join-Path $dist 'split-timer'
New-Item -ItemType Directory -Force $plug | Out-Null
Copy-Item examples\plugins\split-timer\plugin.toml, examples\plugins\split-timer\split.xml $plug
Copy-Item examples\plugins\split-timer\target\wasm32-unknown-unknown\release\split_timer.wasm (Join-Path $plug 'plugin.wasm')
New-Item -ItemType Directory -Force (Join-Path $examples 'plugins') | Out-Null
Compress-Archive -Path (Join-Path $plug '*') -DestinationPath (Join-Path $examples 'plugins\split-timer.zip')
Remove-Item -Recurse -Force $plug

# 3. 许可：字体和第三方库
$lic = Join-Path $stage 'licenses'
New-Item -ItemType Directory -Force $lic | Out-Null
Copy-Item assets\fonts\OFL.txt (Join-Path $lic 'Montserrat-OFL.txt')
$meta = cargo metadata --format-version 1 --locked | ConvertFrom-Json
$deps = cargo tree -p rustyhunt -e normal --prefix none --format '{p}' | ForEach-Object { ($_ -split ' ')[0..1] -join ' ' } | Sort-Object -Unique
$lines = @("RustyHunt $version 链接的第三方 Rust 库", "")
foreach ($p in $meta.packages) {
    if ($p.source -and ($deps -contains "$($p.name) v$($p.version)")) {
        $lines += "$($p.name) $($p.version)  $($p.license)  $($p.repository)"
    }
}
$lines | Set-Content -Encoding utf8 (Join-Path $lic 'THIRD-PARTY.txt')

# 4. 便携版：程序旁边有 portable 标记时数据放在程序旁边
New-Item -ItemType File (Join-Path $stage 'portable') | Out-Null
$portable = Join-Path $dist "$name-portable.zip"
Compress-Archive -Path $stage -DestinationPath $portable

# 5. 安装版：把便携版的 zip 嵌进安装程序（安装时去掉 portable 标记）
$env:RH_PAYLOAD = $portable
cargo build --release -p rustyhunt-setup
$code = $LASTEXITCODE
Remove-Item Env:\RH_PAYLOAD
if ($code) { throw '安装程序编译失败' }
$setup = Join-Path $dist "$name-setup.exe"
Copy-Item target\release\rustyhunt-setup.exe $setup

# 6. 地址表单独发布，和程序包一起写进 SHA256SUMS
Copy-Item maps\*.map $dist
$sums = Get-ChildItem $dist -File | Where-Object { $_.Name -ne 'SHA256SUMS' } | Sort-Object Name | ForEach-Object {
    "$((Get-FileHash -Algorithm SHA256 $_.FullName).Hash.ToLower())  $($_.Name)"
}
$sums | Set-Content -Encoding ascii (Join-Path $dist 'SHA256SUMS')
Remove-Item -Recurse -Force $stage

Write-Host ''
Get-ChildItem $dist | ForEach-Object { '{0,-45} {1,10:N0} KB' -f $_.Name, ($_.Length / 1KB) }
