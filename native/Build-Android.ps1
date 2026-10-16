param([string]$Gradle = 'gradle.bat')
$ErrorActionPreference = 'Stop'
if (!$env:ANDROID_HOME -or !$env:NDK_HOME -or !$env:JAVA_HOME) { throw 'Set ANDROID_HOME, NDK_HOME and JAVA_HOME first. See README.' }
$nativeRoot=$PSScriptRoot
$project=Join-Path $nativeRoot 'src-tauri\gen\android'
$toolchain=Join-Path $env:NDK_HOME 'toolchains\llvm\prebuilt\windows-x86_64\bin'
$env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=Join-Path $toolchain 'aarch64-linux-android24-clang.cmd'
$env:CC_aarch64_linux_android=$env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER
$env:AR_aarch64_linux_android=Join-Path $toolchain 'llvm-ar.exe'
$env:TAURI_ANDROID_PROJECT_PATH=$project
$env:WRY_ANDROID_PACKAGE='org.lattice.notes'
$env:WRY_ANDROID_LIBRARY='app_lib'
$env:TAURI_ANDROID_PACKAGE_UNESCAPED='org.lattice.notes'
$env:WRY_ANDROID_KOTLIN_FILES_OUT_DIR=Join-Path $project 'app\src\main\java\org\lattice\notes\generated'
New-Item -ItemType Directory -Force -Path $env:WRY_ANDROID_KOTLIN_FILES_OUT_DIR | Out-Null
if (!$env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR=Join-Path $nativeRoot 'src-tauri\target' }
if (!(Test-Path -LiteralPath (Join-Path $project 'tauri.settings.gradle'))) {
    # Regenerate Cargo-dependent Gradle paths when reusing an existing Rust build cache.
    & cargo clean --manifest-path (Join-Path $nativeRoot 'src-tauri\Cargo.toml') --target aarch64-linux-android -p app
    if ($LASTEXITCODE -ne 0) { throw 'Unable to refresh native project metadata.' }
}
& cargo build --locked --manifest-path (Join-Path $nativeRoot 'src-tauri\Cargo.toml') --target aarch64-linux-android --lib --features tauri/custom-protocol
if ($LASTEXITCODE -ne 0) { throw 'Rust Android compilation failed; no APK was packaged.' }
$library=Join-Path $env:CARGO_TARGET_DIR 'aarch64-linux-android\debug\libapp_lib.so'
& (Join-Path $nativeRoot 'Build-AndroidFromLibrary.ps1') -Library $library -Gradle $Gradle
