@echo off
setlocal EnableExtensions
pushd "%~dp0"
set "SRC_EXE=%~dp0target\release\openpart-gui.exe"

echo OpenPart GUI build started

echo Building...
cargo build --release -p openpart-gui
if errorlevel 1 goto :build_failed

if not exist "%SRC_EXE%" goto :exe_missing

echo Copying project to C:\Openpart (excluding target)...
if not exist "C:\Openpart" mkdir "C:\Openpart"
robocopy "%~dp0." "C:\Openpart" /E /XD target node_modules .git .github /NFL /NDL /NJH /NJS
if %ERRORLEVEL% GEQ 8 goto :copy_failed

if exist "C:\Openpart\openpart.exe" del "C:\Openpart\openpart.exe"

echo Copying GUI exe to C:\Openpart\OpenPart.exe
copy /Y "%SRC_EXE%" "C:\Openpart\OpenPart.exe"
if errorlevel 1 goto :exe_copy_failed

echo Launching OpenPart as administrator...
powershell -NoProfile -Command "Start-Process -FilePath 'C:\Openpart\OpenPart.exe' -WorkingDirectory 'C:\Openpart' -Verb RunAs"
if errorlevel 1 goto :launch_failed

echo Done.
popd
pause
endlocal
exit /b 0

:build_failed
echo Build failed.
goto :finish_error

:exe_missing
echo GUI exe not found: %SRC_EXE%
goto :finish_error

:copy_failed
echo Robocopy reported a failure.
goto :finish_error

:exe_copy_failed
echo Failed to copy GUI exe.
goto :finish_error

:launch_failed
echo Failed to launch OpenPart as administrator.
goto :finish_error

:finish_error
popd
pause
endlocal
exit /b 1
