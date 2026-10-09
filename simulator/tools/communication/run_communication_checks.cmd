@echo off
setlocal
set REPO=D:\dev\slay-the-spire
set NODE=C:\Program Files\nodejs\node.exe
"%NODE%" "%REPO%\simulator\tools\communication\overnight_collector.test.js" || exit /b 1
"%NODE%" "%REPO%\simulator\tools\communication\bridge_probe.test.js" || exit /b 1
"%NODE%" "%REPO%\simulator\tools\communication\overnight_preflight.test.js" || exit /b 1
"%NODE%" "%REPO%\simulator\tools\communication\overnight_supervisor.test.js" || exit /b 1
"%NODE%" "%REPO%\simulator\tools\communication\harvest_status.test.js" || exit /b 1
"%NODE%" "%REPO%\simulator\tools\communication\guided_collect_status.test.js" || exit /b 1
"%NODE%" "%REPO%\simulator\tools\communication\launcher_scripts.test.js" || exit /b 1
"%NODE%" "%REPO%\simulator\tools\communication\trace_tools.test.js" || exit /b 1
"%NODE%" "%REPO%\simulator\tools\communication\trace_client.test.js" || exit /b 1
"%NODE%" "%REPO%\simulator\tools\communication\random_fidelity_collector.test.js" || exit /b 1
"%NODE%" --test "%REPO%\simulator\tools\communication\random_fidelity_fail_closed.test.js" || exit /b 1
"%NODE%" --test "%REPO%\simulator\tools\communication\collection_review_regressions.test.js" || exit /b 1
"%NODE%" --test "%REPO%\simulator\tools\communication\choice_contract.test.js" || exit /b 1
"%NODE%" "%REPO%\simulator\tools\communication\run_random_fidelity_campaign.test.js" || exit /b 1
"%NODE%" "%REPO%\simulator\tools\communication\random_fidelity_game_watchdog.test.js" || exit /b 1
"%NODE%" "%REPO%\simulator\tools\communication\trace_ui\server.test.js" || exit /b 1
