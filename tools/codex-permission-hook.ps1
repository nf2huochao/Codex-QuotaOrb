$ErrorActionPreference = 'Stop'
$pairingPath = Join-Path $env:APPDATA 'com.codex.quota-floating-window\pairing.json'
$payload = [Console]::In.ReadToEnd()
$diagnosticPath = Join-Path $env:APPDATA 'com.codex.quota-floating-window\hook-events.jsonl'
$sessionId = $null
$turnId = $null
$parseError = $null
try {
  $json = $payload | ConvertFrom-Json
  if ($json.session_id) { $sessionId = [string]$json.session_id }
  elseif ($json.sessionId) { $sessionId = [string]$json.sessionId }
  elseif ($json.thread_id) { $sessionId = [string]$json.thread_id }
  elseif ($json.threadId) { $sessionId = [string]$json.threadId }
  if ($json.turn_id) { $turnId = [string]$json.turn_id }
  elseif ($json.turnId) { $turnId = [string]$json.turnId }
} catch {
  $parseError = 'payload_parse_failed'
  Write-Error 'Codex permission hook payload could not be parsed.' -ErrorAction Continue
}

function Write-HookDiagnostic {
  param([int]$HttpStatus, [bool]$Delivered, [string]$ErrorMessage)
  try {
    $parent = Split-Path -Parent $diagnosticPath
    New-Item -ItemType Directory -Force -Path $parent | Out-Null
    $record = [ordered]@{
      event = 'PermissionRequest'
      session_id = if ($sessionId) { $sessionId } else { $null }
      turn_id = if ($turnId) { $turnId } else { $null }
      received_at = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
      http_status = $HttpStatus
      delivered = $Delivered
      error = if ($ErrorMessage) { $ErrorMessage.Substring(0, [Math]::Min(160, $ErrorMessage.Length)) } else { $null }
    }
    [IO.File]::AppendAllText($diagnosticPath, (($record | ConvertTo-Json -Compress) + [Environment]::NewLine), [Text.UTF8Encoding]::new($false))
    if ((Get-Item -LiteralPath $diagnosticPath).Length -gt 1MB) {
      $text = [IO.File]::ReadAllText($diagnosticPath)
      $keep = $text.Substring([Math]::Max(0, $text.Length - 400000))
      $newline = $keep.IndexOf([Environment]::NewLine)
      if ($newline -ge 0) { $keep = $keep.Substring($newline + [Environment]::NewLine.Length) }
      [IO.File]::WriteAllText($diagnosticPath, $keep, [Text.UTF8Encoding]::new($false))
    }
  } catch {
    Write-Error ("Codex hook diagnostic write failed: " + $_.Exception.Message) -ErrorAction Continue
  }
}

$httpStatus = 0
$delivered = $false
$errorMessage = $parseError
try {
  $pairing = Get-Content -LiteralPath $pairingPath -Raw | ConvertFrom-Json
  $response = Invoke-RestMethod -Method Post `
    -Uri 'http://127.0.0.1:18765/api/hooks/permission' `
    -Headers @{ 'X-Codex-Hook-Token' = [string]$pairing.session_token; 'Content-Type' = 'application/json' } `
    -Body $payload
  $httpStatus = 200
  $delivered = $true
  $response | ConvertTo-Json -Compress
} catch {
  $errorMessage = $_.Exception.Message
  ('{}')
} finally {
  Write-HookDiagnostic -HttpStatus $httpStatus -Delivered $delivered -ErrorMessage $errorMessage
}
