@echo off
setlocal
cd /d "%~dp0"

if not exist "node_modules\.bin\tauri.cmd" (
    echo Ark development dependencies are not installed.
    echo Run pnpm install from an unrestricted terminal, then try again.
    pause
    exit /b 1
)

call "node_modules\.bin\tauri.cmd" dev
set "ARK_DEV_EXIT_CODE=%ERRORLEVEL%"
if not "%ARK_DEV_EXIT_CODE%"=="0" (
    echo.
    echo Ark development startup failed with exit code %ARK_DEV_EXIT_CODE%.
    echo See the error above.
    pause
)
exit /b %ARK_DEV_EXIT_CODE%
