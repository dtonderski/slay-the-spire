@echo off
setlocal
set REPO=D:\dev\slay-the-spire
set TRACE_AUTO_STATE_MS=1000
"C:\Program Files\nodejs\node.exe" "%REPO%\tools\communication\trace_client.js"
