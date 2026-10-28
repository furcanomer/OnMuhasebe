@echo off
setlocal enabledelayedexpansion

echo.
echo [1/3] Release derlemesi...
cargo build --release
if errorlevel 1 exit /b 1

echo.
echo [2/3] cargo-wix kontrol...
cargo wix --version >nul 2>&1
if errorlevel 1 (
    echo cargo-wix bulunamadi. Kurmak icin:
    echo   cargo install cargo-wix
    echo ve tekrar deneyin.
    exit /b 1
)

echo.
echo [3/3] MSI olusturuluyor...
for /f "tokens=2 delims== " %%v in ('findstr /r /c:"^version[ ]*=" Cargo.toml') do set "APP_VERSION=%%~v"
if not defined APP_VERSION (
    echo Cargo.toml icinden surum okunamadi.
    exit /b 1
)
if not exist dist mkdir dist
cargo wix -v --nocapture --output dist\OnMuhasebe-%APP_VERSION%.msi -C -dCargoPackageVersion=%APP_VERSION%
if errorlevel 1 exit /b 1

echo.
echo Tamamlandi.
dir /b dist\*.msi
endlocal
