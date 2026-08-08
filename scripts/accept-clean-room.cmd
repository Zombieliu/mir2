@echo off
setlocal
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0accept-clean-room.ps1" %*
exit /b %ERRORLEVEL%
