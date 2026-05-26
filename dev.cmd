@echo off
REM Windows entrypoint: forwards to the implementation in docker/dev.ps1.
REM %~dp0 is this file's dir (repo root), so it works from any directory.
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0docker\dev.ps1" %*
