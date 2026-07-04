@echo off
setlocal
set REPO=D:\dev\slay-the-spire
set NODE=C:\Program Files\nodejs\node.exe
"%NODE%" "%REPO%\tools\communication\guided_collect_status.js" %*
