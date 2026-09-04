@echo off
setlocal
set REPO=D:\dev\slay-the-spire
call "%REPO%\simulator\tools\communication\run_guided_collect.cmd" %*
set RESULT=%ERRORLEVEL%
call "%REPO%\simulator\tools\communication\run_guided_collect_status.cmd"
exit /b %RESULT%
