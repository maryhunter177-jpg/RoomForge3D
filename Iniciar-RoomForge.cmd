@echo off
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0tools\start-native.ps1"
if errorlevel 1 (
  echo Nao foi possivel iniciar. Consulte os logs em .tools.
  pause
  exit /b 1
)
start "" "http://localhost:5173"
