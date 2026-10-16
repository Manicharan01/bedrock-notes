param(
    [Parameter(Mandatory=$true)][string]$Library,
    [string]$Gradle = 'gradle.bat'
)
$ErrorActionPreference = 'Stop'
# This script packages an existing library; compile it first with Tauri's Android CLI.
$libraryPath = (Resolve-Path -LiteralPath $Library).Path
if ([IO.Path]::GetFileName($libraryPath) -ne 'libapp_lib.so') { throw 'Supply the ARM64 libapp_lib.so produced by this native project.' }
$stream=[IO.File]::OpenRead($libraryPath)
try { $elfHeader=New-Object byte[] 20; if ($stream.Read($elfHeader,0,20) -ne 20 -or $elfHeader[0] -ne 127 -or $elfHeader[1] -ne 69 -or $elfHeader[2] -ne 76 -or $elfHeader[3] -ne 70 -or $elfHeader[4] -ne 2 -or $elfHeader[5] -ne 1 -or $elfHeader[18] -ne 183 -or $elfHeader[19] -ne 0) { throw 'Expected an ARM64 little-endian ELF library.' } } finally { $stream.Dispose() }
$project = Join-Path $PSScriptRoot 'src-tauri\gen\android'
$destination = Join-Path $project 'app\src\main\jniLibs\arm64-v8a'
New-Item -ItemType Directory -Force -Path $destination | Out-Null
Copy-Item -LiteralPath $libraryPath -Destination (Join-Path $destination 'libapp_lib.so') -Force
& $Gradle -p $project assembleArm64Debug -x rustBuildArm64Debug --no-daemon --no-configuration-cache --console=plain
if ($LASTEXITCODE -ne 0) { throw 'Gradle packaging failed.' }
