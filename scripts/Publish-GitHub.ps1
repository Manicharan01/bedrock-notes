param([string]$Assets = (Join-Path $PSScriptRoot '..\..'))
$ErrorActionPreference = 'Stop'
$project = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$assetsPath = (Resolve-Path -LiteralPath $Assets).Path
$repo = 'Manicharan01/bedrock-notes'
$tag = 'v0.4.1'
$description = 'Self-hosted Markdown notes with backlinks, a graph atlas, and native Windows/Android clients. Rust + SQLite.'
function Invoke-Checked([string]$Command, [string[]]$Arguments) {
    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Command failed with exit code $LASTEXITCODE." }
}
$login = & gh api user --jq .login
if ($LASTEXITCODE -ne 0 -or $login -ne 'Manicharan01') { throw 'Run from a terminal authenticated to GitHub as Manicharan01.' }
$dirty = & git -C $project status --porcelain
if ($LASTEXITCODE -ne 0 -or $dirty) { throw 'Commit or review local changes before publishing.' }
$releaseFiles = @('Bedrock-source.zip','Bedrock-windows-server.zip','Bedrock-windows-desktop.zip','Bedrock-android-arm64-debug.apk','Bedrock-SHA256SUMS.txt')
foreach ($name in $releaseFiles) {
    if (!(Test-Path -LiteralPath (Join-Path $assetsPath $name))) { throw "Missing release asset: $name" }
}
foreach ($line in Get-Content -LiteralPath (Join-Path $assetsPath 'Bedrock-SHA256SUMS.txt')) {
    if ($line -notmatch '^([a-f0-9]{64})  ([^/\\]+)$') { throw 'Invalid checksum manifest.' }
    $expected = $Matches[1]; $name = $Matches[2]
    $actual = (Get-FileHash -LiteralPath (Join-Path $assetsPath $name) -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $expected) { throw "Checksum mismatch: $name" }
}
$existing = & gh repo view $repo --json nameWithOwner --jq .nameWithOwner 2>$null
if ($LASTEXITCODE -ne 0) {
    Invoke-Checked gh @('repo','create',$repo,'--public','--source',$project,'--remote','origin','--description',$description)
} else {
    $visibility = & gh repo view $repo --json visibility --jq .visibility
    if ($LASTEXITCODE -ne 0 -or $visibility -ne 'PUBLIC') { throw 'An existing repository is not public; inspect it before continuing.' }
    $remote = & git -C $project remote get-url origin 2>$null
    if ($LASTEXITCODE -ne 0) { Invoke-Checked git @('-C',$project,'remote','add','origin',"https://github.com/$repo.git") }
    elseif ($remote -ne "https://github.com/$repo.git") { throw 'Origin points elsewhere; inspect it before publishing.' }
}
# Use this terminal's existing GitHub login without changing global Git configuration.
Invoke-Checked git @('-C',$project,'config','--local','credential.https://github.com.helper','')
Invoke-Checked git @('-C',$project,'config','--local','--add','credential.https://github.com.helper','!gh auth git-credential')
& git -C $project push -u origin main
if ($LASTEXITCODE -ne 0) {
    Write-Host 'If GitHub reports missing workflow scope, run: gh auth refresh --hostname github.com --scopes workflow'
    throw 'Push failed. No force-push was attempted. Resolve the reported error and rerun.'
}
Invoke-Checked gh @('repo','edit',$repo,'--description',$description,'--enable-issues=true','--enable-wiki=false','--default-branch','main','--add-topic','self-hosted,markdown,note-taking,rust,tauri,sqlite,knowledge-management,backlinks,pwa,android,windows,obsidian-alternative')
& gh api --method PUT "repos/$repo/private-vulnerability-reporting" 2>$null
if ($LASTEXITCODE -ne 0) { Write-Warning 'Private vulnerability reporting could not be enabled; check repository Security settings.' }
$localTag = & git -C $project tag --list $tag
if (!$localTag) { Invoke-Checked git @('-C',$project,'tag','-a',$tag,'-m','Bedrock 0.4.1') }
Invoke-Checked git @('-C',$project,'push','origin',$tag)
$release = & gh release view $tag --repo $repo --json isDraft 2>$null
if ($LASTEXITCODE -eq 0) {
    if (!(($release | ConvertFrom-Json).isDraft)) { Write-Host "Already published: https://github.com/$repo/releases/tag/$tag"; exit 0 }
} else {
    Invoke-Checked gh @('release','create',$tag,'--repo',$repo,'--verify-tag','--draft','--title','Bedrock 0.4.1 — Your notes. Solid ground.','--notes-file',(Join-Path $project 'docs\releases\v0.4.1.md'))
}
$uploadArgs = @('release','upload',$tag,'--repo',$repo,'--clobber')
foreach ($name in $releaseFiles) { $uploadArgs += Join-Path $assetsPath $name }
Invoke-Checked gh $uploadArgs
Invoke-Checked gh @('release','edit',$tag,'--repo',$repo,'--draft=false','--latest')
Write-Host "Published: https://github.com/$repo"
Write-Host "Downloads: https://github.com/$repo/releases/tag/$tag"
