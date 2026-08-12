param([string]$ProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')))
$installer = Get-ChildItem -LiteralPath (Join-Path $ProjectRoot 'src-tauri/target/debug/bundle/nsis') -Filter '*-setup.exe' -ErrorAction Stop | Select-Object -First 1
$app = Join-Path $ProjectRoot 'src-tauri/target/debug/app.exe'
if (-not (Test-Path -LiteralPath $app)) { throw 'Missing app.exe' }
if ($installer.Length -lt 100000) { throw 'Installer is unexpectedly small' }
Write-Output "Installer exists: $($installer.FullName)"
Write-Output 'Smoke test only checks local artifacts; it does not install or touch Codex credentials.'
