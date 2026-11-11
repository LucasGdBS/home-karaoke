# Downloads a static LGPL ffmpeg as the Tauri sidecar (src-tauri\binaries\ffmpeg-<target-triple>.exe).
$ErrorActionPreference = "Stop"
Set-Location "$PSScriptRoot\..\src-tauri"
New-Item -ItemType Directory -Force binaries | Out-Null
$triple = ((rustc -vV) | Select-String "^host: (.*)").Matches[0].Groups[1].Value
$tmp = Join-Path ([IO.Path]::GetTempPath()) "ffmpeg-sidecar"
Remove-Item -Recurse -Force $tmp, "$tmp.zip" -ErrorAction SilentlyContinue
Invoke-WebRequest "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-lgpl.zip" -OutFile "$tmp.zip"
Expand-Archive "$tmp.zip" $tmp
$exe = Get-ChildItem $tmp -Recurse -Filter ffmpeg.exe | Select-Object -First 1
Copy-Item $exe.FullName "binaries\ffmpeg-$triple.exe"
Remove-Item -Recurse -Force $tmp, "$tmp.zip"
Write-Output "ok: src-tauri\binaries\ffmpeg-$triple.exe"
