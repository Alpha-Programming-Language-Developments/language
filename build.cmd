@echo off
setlocal enabledelayedexpansion

echo ====================================================================
echo           ALPHA LANGUAGE FULL RELEASE BUILD SCRIPT (CMD)
echo ====================================================================
echo.

REM 1. Check for Cargo
where cargo >nul 2>nul
if %errorlevel% neq 0 (
    echo [ERROR] 'cargo' is not found in PATH!
    echo Please install Rust from https://rustup.rs/
    exit /b 1
)

REM 2. Check for Clang / LLVM
where clang >nul 2>nul
if %errorlevel% neq 0 (
    if not exist "C:\Program Files\LLVM\bin\clang.exe" (
        echo [WARNING] Clang / LLVM not found in PATH.
        echo Direct LLVM backend requires Clang. Download: https://github.com/llvm/llvm-project/releases
    )
)

REM 3. Compile full workspace in release mode
echo [1/3] Building Alpha Compiler, LSP Server, and Precompiled Static Runtime...
cargo build --workspace --release
if %errorlevel% neq 0 (
    echo.
    echo [ERROR] Workspace compilation failed!
    exit /b %errorlevel%
)

REM 4. Verify outputs
set "TARGET_DIR=%~dp0target\release"
if not exist "%TARGET_DIR%\alpha.exe" (
    echo [ERROR] Expected binary not found: %TARGET_DIR%\alpha.exe
    exit /b 1
)

if not exist "%TARGET_DIR%\alpha-lsp.exe" (
    echo [ERROR] Expected binary not found: %TARGET_DIR%\alpha-lsp.exe
    exit /b 1
)

if not exist "%TARGET_DIR%\alpha_rt.lib" (
    echo [ERROR] Expected static runtime library not found: %TARGET_DIR%\alpha_rt.lib
    exit /b 1
)

REM 5. Install to Cargo bin for instant system-wide availability
echo.
echo [2/3] Installing binaries to %%USERPROFILE%%\.cargo\bin ...
set "INSTALL_DIR=%USERPROFILE%\.cargo\bin"
if not exist "%INSTALL_DIR%" mkdir "%INSTALL_DIR%"

taskkill /F /IM alpha.exe /T >nul 2>nul
taskkill /F /IM alpha-lsp.exe /T >nul 2>nul

copy /Y "%TARGET_DIR%\alpha.exe" "%INSTALL_DIR%\alpha.exe" >nul
copy /Y "%TARGET_DIR%\alpha-lsp.exe" "%INSTALL_DIR%\alpha-lsp.exe" >nul
copy /Y "%TARGET_DIR%\alpha_rt.lib" "%INSTALL_DIR%\alpha_rt.lib" >nul

REM 6. Done
echo [3/3] Installation complete!
echo.
echo ====================================================================
echo [SUCCESS] Alpha Full Release Build Completed:
echo   - Compiler:       %INSTALL_DIR%\alpha.exe
echo   - LSP Server:     %INSTALL_DIR%\alpha-lsp.exe
echo   - Static Runtime: %INSTALL_DIR%\alpha_rt.lib
echo ====================================================================
echo.
echo Run 'alpha -v' or 'alpha help' to get started.
