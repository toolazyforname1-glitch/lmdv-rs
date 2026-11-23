@echo off
setlocal EnableExtensions
set "HERE=%~dp0"
set "EXE=%HERE%target\release\lmdv-tui.exe"

if not exist "%EXE%" (
  echo lmdv-tui: release binary not found.
  echo.
  echo Build it first:
  echo     cargo build --release --manifest-path "%HERE%Cargo.toml"
  echo.
  pause
  exit /b 1
)

if not "%~1"=="" if exist "%~1\" (
  pushd "%~1"
  "%EXE%"
  set "CODE=%ERRORLEVEL%"
  popd
  exit /b %CODE%
)

"%EXE%" %*
exit /b %ERRORLEVEL%
