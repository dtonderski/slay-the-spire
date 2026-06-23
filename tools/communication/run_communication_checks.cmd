@echo off
setlocal
set REPO=D:\dev\slay-the-spire
set NODE=C:\Program Files\nodejs\node.exe
"%NODE%" "%REPO%\tools\communication\overnight_collector.test.js" || exit /b 1
"%NODE%" "%REPO%\tools\communication\overnight_preflight.test.js" || exit /b 1
"%NODE%" "%REPO%\tools\communication\overnight_supervisor.test.js" || exit /b 1
"%NODE%" "%REPO%\tools\communication\harvest_status.test.js" || exit /b 1
"%NODE%" "%REPO%\tools\communication\trace_tools.test.js" || exit /b 1
