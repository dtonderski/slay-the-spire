@echo off
setlocal
set REPO=D:\dev\slay-the-spire
set NODE=C:\Program Files\nodejs\node.exe
"%NODE%" "%REPO%\tools\communication\overnight_preflight.js" || exit /b 1
"%NODE%" "%REPO%\tools\communication\overnight_supervisor.js"
