param([string]$PublicUrl = 'http://127.0.0.1:4173', [string]$Bind = '127.0.0.1:4173', [string]$KeyFile = '')
$ErrorActionPreference = 'Stop'
$serverArgs = @('--bind', $Bind, '--public-url', $PublicUrl, '--db', (Join-Path $PSScriptRoot 'data\lattice.sqlite3'), '--allow-origin', 'tauri://localhost', '--allow-origin', 'http://tauri.localhost', '--allow-origin', 'https://tauri.localhost')
if ($KeyFile) { $serverArgs += @('--token-file', $KeyFile) }
& (Join-Path $PSScriptRoot 'bedrock.exe') @serverArgs
