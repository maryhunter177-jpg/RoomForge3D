@echo off
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0tools\stop-native.ps1"
if errorlevel 1 (
  echo Nao foi possivel encerrar. Consulte a mensagem acima.
  pause
  exit /b 1
)
