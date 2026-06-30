@echo off
setlocal
set REPO=D:\dev\slay-the-spire
cd /d "%REPO%\simulator" || exit /b 1
uv run python -m sts.guided_collect --report-output target\guided-collect\latest.json --archive-report-dir target\guided-collect\reports --fail-on-not-ok --preflight-timeout-seconds 30 %*
