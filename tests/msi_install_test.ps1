#Requires -RunAsAdministrator

<#
.SYNOPSIS
    Aşama 14-5: MSI gerçek kurulum / yükseltme / kaldırma otomatik testi.

.DESCRIPTION
    dist\OnMuhasebe-1.0.0.msi dosyasını kurar, Programlar listesinde tek giriş
    olduğunu doğrular, kullanıcıdan ilk veriyi oluşturmasını ister, 1.0.1 MSI
    üretir, yükseltir, verinin korunduğunu doğrular, kaldırır ve ortamı temizler.

    Yönetici PowerShell'de çalıştırılmalıdır. UAC onayları kullanıcı tarafından
    verilir. Kritik noktalarda Enter beklenir.

.NOTES
    Bu script uygulama kodunu değiştirmez; yalnızca kurulum davranışını test eder.
    Gerçek kullanıcı verisi (%LOCALAPPDATA%\OnMuhasebe) yedeklenir ve test sonunda
    geri yüklenir.
#>

[CmdletBinding()]
param(
    # Otomatik mod: ADIM 3'te kullanıcı etkileşimi yerine uygulama bir kez
    # açılıp şema oluşturulur, kapatılır ve TEST-CAR-001 kaydı SQL ile eklenir.
    # ADIM 11'de temizlik onayı otomatik verilir. Varsayılan (bu anahtar
    # verilmezse) davranış etkileşimlidir.
    [switch]$Otomatik
)

$ErrorActionPreference = 'Stop'

# ------------------------------------------------------------------ sabitler
$Script:Root         = Split-Path -Parent $PSScriptRoot
$Script:Stamp        = Get-Date -Format 'yyyyMMdd-HHmmss'
$Script:LogDir       = Join-Path $Script:Root 'tests\msi-test-logs'
$Script:LogPath      = Join-Path $Script:LogDir "msi-test-$Script:Stamp.log"
$Script:Results      = @()
$Script:DataRoot     = Join-Path $env:LOCALAPPDATA 'OnMuhasebe'
$Script:DataBackup   = Join-Path $env:LOCALAPPDATA "OnMuhasebe.pre-msi-test-$Script:Stamp"
$Script:HadData      = $false
$Script:CargoToml    = Join-Path $Script:Root 'Cargo.toml'
$Script:CargoBackup  = Join-Path $Script:Root 'Cargo.toml.bak'
$Script:Msi100       = Join-Path $Script:Root 'dist\OnMuhasebe-1.0.0.msi'
$Script:Msi101       = Join-Path $Script:Root 'dist\OnMuhasebe-1.0.1.msi'
$Script:ProgramFiles = if ($env:ProgramW6432) { $env:ProgramW6432 } else { $env:ProgramFiles }
$Script:InstallDir   = Join-Path $Script:ProgramFiles 'OnMuhasebe'
$Script:ExePath      = Join-Path $Script:InstallDir 'OnMuhasebe.exe'
$Script:DbPath       = Join-Path $Script:DataRoot 'OnMuhasebe\data\on_muhasebe.db'
$Script:DbSizeStep3  = 0
$Script:WixBin       = $null
$Script:InstallAttempted = $false

# ------------------------------------------------------------------ yardımcı
function Write-Log {
    param(
        [Parameter(Mandatory = $true)][string]$Message,
        [ValidateSet('INFO', 'OK', 'WARN', 'FAIL', 'STEP')][string]$Level = 'INFO'
    )

    $line = '[{0}] [{1,-4}] {2}' -f (Get-Date -Format 'HH:mm:ss'), $Level, $Message
    switch ($Level) {
        'OK'   { Write-Host $line -ForegroundColor Green }
        'WARN' { Write-Host $line -ForegroundColor Yellow }
        'FAIL' { Write-Host $line -ForegroundColor Red }
        'STEP' { Write-Host $line -ForegroundColor Cyan }
        default { Write-Host $line }
    }
    Add-Content -LiteralPath $Script:LogPath -Value $line -Encoding UTF8
}

function Add-Result {
    param(
        [string]$Step,
        [bool]$Success,
        [double]$Seconds,
        [string]$Note = ''
    )

    $Script:Results += [pscustomobject]@{
        Step  = $Step
        Sonuc = if ($Success) { 'OK' } else { 'FAIL' }
        Sure  = ('{0:N1} sn' -f $Seconds)
        Not   = $Note
    }
}

function Invoke-TestStep {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][scriptblock]$Body
    )

    Write-Log ('=' * 68) 'STEP'
    Write-Log $Name 'STEP'
    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    try {
        & $Body
        $watch.Stop()
        Add-Result -Step $Name -Success $true -Seconds $watch.Elapsed.TotalSeconds
        Write-Log "$Name tamamlandı." 'OK'
    }
    catch {
        $watch.Stop()
        Add-Result -Step $Name -Success $false -Seconds $watch.Elapsed.TotalSeconds -Note $_.Exception.Message
        Write-Log "$Name BAŞARISIZ: $($_.Exception.Message)" 'FAIL'
        throw
    }
}

function Get-ObjectProperty {
    param($Object, [string]$Name)

    if ($null -eq $Object) { return $null }
    $property = $Object.PSObject.Properties[$Name]
    if ($property) { return $property.Value }
    return $null
}

function Get-MuhasebeEntries {
    $keys = @(
        'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall',
        'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall',
        'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall'
    )

    $items = @()
    foreach ($key in $keys) {
        if (-not (Test-Path -LiteralPath $key)) { continue }
        foreach ($child in (Get-ChildItem -LiteralPath $key -ErrorAction SilentlyContinue)) {
            $properties = Get-ItemProperty -LiteralPath $child.PSPath -ErrorAction SilentlyContinue
            $displayName = Get-ObjectProperty $properties 'DisplayName'
            if (-not $displayName) { continue }
            if ($displayName -notlike '*Muhasebe*') { continue }

            $items += [pscustomobject]@{
                ProductCode     = $child.PSChildName
                DisplayName     = [string]$displayName
                DisplayVersion  = [string](Get-ObjectProperty $properties 'DisplayVersion')
                Publisher       = [string](Get-ObjectProperty $properties 'Publisher')
                InstallLocation = [string](Get-ObjectProperty $properties 'InstallLocation')
                KeyPath         = $child.Name
            }
        }
    }

    return $items
}

function Write-EntriesToLog {
    param([string]$Label, $Entries)

    if ($Entries.Count -eq 0) {
        Write-Log "$Label kayıt: yok" 'INFO'
        return
    }
    foreach ($entry in $Entries) {
        Write-Log ("{0} kayıt: '{1}' | sürüm {2} | yayıncı {3} | {4}" -f `
                $Label, $entry.DisplayName, $entry.DisplayVersion, $entry.Publisher, $entry.ProductCode) 'INFO'
    }
}

function Test-MsiSuccess {
    param([int]$Code)

    return @(0, 3010, 1641) -contains $Code
}

<#
.SYNOPSIS
    SQLite veritabanında tek bir SQL ifadesi çalıştırır.

.DESCRIPTION
    Önce sqlite3.exe aranır; yoksa Python'un yerleşik sqlite3 modülü kullanılır.
    Hiçbiri yoksa $null döner. İlk sütunun ilk satırı döndürülür.
#>
function Invoke-Sqlite {
    param(
        [Parameter(Mandatory = $true)][string]$Database,
        [Parameter(Mandatory = $true)][string]$Query
    )

    $sqlite = Get-Command 'sqlite3.exe' -ErrorAction SilentlyContinue
    if ($sqlite) {
        $output = @(& $sqlite.Source $Database $Query 2>&1)
        if ($LASTEXITCODE -ne 0) {
            throw "sqlite3 sorgusu başarısız: $($output -join ' ')"
        }
        return ($output | Select-Object -First 1)
    }

    $python = Get-Command 'python.exe' -ErrorAction SilentlyContinue
    if (-not $python) { return $null }

    $code = 'import sqlite3,sys; con=sqlite3.connect(sys.argv[1]); cur=con.execute(sys.argv[2]); con.commit(); print(cur.fetchone()[0] if cur.description else None)'
    $output = @(& $python.Source '-c' $code $Database $Query 2>&1)
    if ($LASTEXITCODE -ne 0) {
        throw "python sqlite3 sorgusu başarısız: $($output -join ' ')"
    }
    return ($output | Select-Object -First 1)
}

function Invoke-MsiExec {
    param(
        [Parameter(Mandatory = $true)][string]$Arguments,
        [Parameter(Mandatory = $true)][string]$What
    )

    Write-Log "msiexec $Arguments" 'INFO'
    $process = Start-Process -FilePath 'msiexec.exe' -ArgumentList $Arguments -Wait -PassThru -NoNewWindow
    Write-Log "$What çıkış kodu: $($process.ExitCode)" 'INFO'
    return [int]$process.ExitCode
}

function Resolve-WixBin {
    $candidates = New-Object System.Collections.Generic.List[string]
    if ($env:WIX) { [void]$candidates.Add((Join-Path $env:WIX 'bin')) }

    $candle = Get-Command 'candle.exe' -ErrorAction SilentlyContinue
    if ($candle) { [void]$candidates.Add((Split-Path -Parent $candle.Source)) }

    [void]$candidates.Add((Join-Path $env:TEMP 'opencode\wix314'))
    [void]$candidates.Add('C:\Program Files (x86)\WiX Toolset v3.14\bin')
    [void]$candidates.Add('C:\Program Files (x86)\WiX Toolset v3.11\bin')
    [void]$candidates.Add('C:\Program Files\WiX Toolset v3.14\bin')

    foreach ($candidate in $candidates) {
        if (-not $candidate) { continue }
        if (Test-Path -LiteralPath (Join-Path $candidate 'candle.exe')) {
            if (Test-Path -LiteralPath (Join-Path $candidate 'light.exe')) {
                return $candidate
            }
        }
    }
    return $null
}

function Restore-CargoToml {
    if (-not (Test-Path -LiteralPath $Script:CargoBackup)) { return }
    Copy-Item -LiteralPath $Script:CargoBackup -Destination $Script:CargoToml -Force
    Remove-Item -LiteralPath $Script:CargoBackup -Force
    Write-Log 'Cargo.toml geri yüklendi (version = "1.0.0").' 'OK'
}

# ------------------------------------------------------------------ adımlar
function Assert-Preconditions {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object -TypeName Security.Principal.WindowsPrincipal -ArgumentList $identity
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'Bu script yönetici PowerShell gerektirir. PowerShell''i "Yönetici olarak çalıştır" ile açın.'
    }
    Write-Log 'Yönetici yetkisi doğrulandı.' 'OK'

    if (-not (Test-Path -LiteralPath $Script:Msi100)) {
        throw "dist\OnMuhasebe-1.0.0.msi bulunamadı. Önce build-msi.bat çalıştırın."
    }
    Write-Log "1.0.0 MSI bulundu: $($Script:Msi100)" 'OK'

    $cargoText = Get-Content -LiteralPath $Script:CargoToml -Raw
    if ($cargoText -notmatch '(?m)^version\s*=\s*"1\.0\.0"\s*$') {
        throw 'Cargo.toml sürümü 1.0.0 değil. Test başlamadan önce version = "1.0.0" yapın.'
    }
    Write-Log 'Cargo.toml sürümü 1.0.0.' 'OK'

    $entries = @(Get-MuhasebeEntries)
    if ($entries.Count -gt 0) {
        Write-EntriesToLog 'Mevcut kurulum' $entries
        throw 'Mevcut bir On Muhasebe kurulumu var. Önce mevcut kurulumu kaldırın.'
    }
    Write-Log 'Mevcut kurulum yok.' 'OK'

    if (Get-Process -Name 'OnMuhasebe' -ErrorAction SilentlyContinue) {
        throw 'OnMuhasebe çalışıyor. Lütfen kapatıp tekrar deneyin.'
    }
    Write-Log 'OnMuhasebe süreci çalışmıyor.' 'OK'

    $cargo = Get-Command 'cargo.exe' -ErrorAction SilentlyContinue
    if (-not $cargo) {
        throw 'cargo.exe PATH üzerinde bulunamadı. Rust araç zincirini kontrol edin.'
    }

    try {
        $wixVersion = (& cargo wix --version 2>&1 | Out-String).Trim()
        $wixExit = $LASTEXITCODE
    }
    catch {
        $wixVersion = $_.Exception.Message
        $wixExit = 1
    }
    if ($wixExit -ne 0) {
        throw "cargo-wix kurulu değil veya çalıştırılamadı. 'cargo install cargo-wix' çalıştırın. Ayrıntı: $wixVersion"
    }
    Write-Log "cargo-wix: $wixVersion" 'OK'

    $Script:WixBin = Resolve-WixBin
    if (-not $Script:WixBin) {
        throw 'candle.exe/light.exe bulunamadı. WiX Toolset 3.14 kurun veya WIX ortam değişkenini ayarlayın.'
    }
    Write-Log "WiX araç seti: $($Script:WixBin)" 'OK'

    if (Test-Path -LiteralPath $Script:DataRoot) {
        Write-Log "Mevcut kullanıcı verisi bulundu ve yedeklenecek: $($Script:DataRoot)" 'WARN'
    }
    else {
        Write-Log 'Mevcut kullanıcı verisi yok; test temiz veriyle başlayacak.' 'INFO'
    }
}

function Backup-UserData {
    if (-not (Test-Path -LiteralPath $Script:DataRoot)) {
        $Script:HadData = $false
        Write-Log '%LOCALAPPDATA%\OnMuhasebe yok; yedek gerekmedi.' 'INFO'
        return
    }
    if (Test-Path -LiteralPath $Script:DataBackup) {
        throw "Yedek hedefi zaten var: $($Script:DataBackup)"
    }

    Move-Item -LiteralPath $Script:DataRoot -Destination $Script:DataBackup -Force
    $Script:HadData = $true
    Write-Log "Mevcut veri güvenli yedekte: $($Script:DataBackup)" 'OK'
}

function Install-Msi100 {
    $Script:InstallAttempted = $true
    $log = Join-Path $Script:LogDir 'install-1.0.0.log'
    $code = Invoke-MsiExec "/i `"$($Script:Msi100)`" /qn /norestart /l*v `"$log`"" '1.0.0 kurulumu'
    if (-not (Test-MsiSuccess $code)) {
        Write-Log "Kurulum logu: $log" 'FAIL'
        throw "1.0.0 kurulumu başarısız (çıkış kodu $code)."
    }
    Write-Log '1.0.0 kurulumu tamamlandı.' 'OK'
}

function Assert-InstallState {
    param(
        [Parameter(Mandatory = $true)][string]$ExpectedVersion,
        [Parameter(Mandatory = $true)][string]$Label
    )

    $entries = @(Get-MuhasebeEntries)
    Write-EntriesToLog $Label $entries

    if ($entries.Count -ne 1) {
        throw "$Label : Programlar listesinde tam 1 giriş bekleniyordu, bulunan $($entries.Count)."
    }

    $entry = $entries[0]
    if ($entry.DisplayVersion -ne $ExpectedVersion) {
        throw "$Label : DisplayVersion '$($entry.DisplayVersion)', beklenen '$ExpectedVersion'."
    }
    if ($entry.Publisher -ne 'furcanomer') {
        throw "$Label : Publisher '$($entry.Publisher)', beklenen 'furcanomer'."
    }
    if (-not (Test-Path -LiteralPath $Script:ExePath)) {
        throw "$Label : exe bulunamadı: $($Script:ExePath)"
    }

    $exeVersion = (Get-Item -LiteralPath $Script:ExePath).VersionInfo.ProductVersion
    if ($exeVersion -ne $ExpectedVersion) {
        throw "$Label : exe ProductVersion '$exeVersion', beklenen '$ExpectedVersion'."
    }

    Write-Log "$Label doğrulandı: tek giriş, sürüm $ExpectedVersion, yayıncı furcanomer." 'OK'
}

function Initialize-TestData {
    if (-not (Test-Path -LiteralPath $Script:ExePath)) {
        throw "Kurulu uygulama bulunamadı: $($Script:ExePath)"
    }

    if ($Otomatik) {
        Write-Log 'Otomatik mod: uygulama şemayı oluşturmak için açılıyor.' 'INFO'
        Start-Process -FilePath $Script:ExePath | Out-Null

        $deadline = (Get-Date).AddSeconds(90)
        while ((Get-Date) -lt $deadline) {
            if (Test-Path -LiteralPath $Script:DbPath) { break }
            Start-Sleep -Milliseconds 500
        }
        Start-Sleep -Seconds 4

        $running = Get-Process -Name 'OnMuhasebe' -ErrorAction SilentlyContinue
        if ($running) {
            $running | Stop-Process -Force
            Start-Sleep -Seconds 2
            Write-Log 'Uygulama şema oluşturulduktan sonra kapatıldı.' 'INFO'
        }

        if (-not (Test-Path -LiteralPath $Script:DbPath)) {
            throw "Veritabanı dosyası oluşmadı: $($Script:DbPath)"
        }

        $seed = "INSERT INTO customers (code, name, kind) VALUES ('TEST-CAR-001', 'Test Cari', 'musteri');"
        $null = Invoke-Sqlite -Database $Script:DbPath -Query $seed
        $null = Invoke-Sqlite -Database $Script:DbPath -Query 'PRAGMA wal_checkpoint(FULL);'

        $count = ([string](Invoke-Sqlite -Database $Script:DbPath -Query "SELECT COUNT(*) FROM customers WHERE code = 'TEST-CAR-001';")).Trim()
        if ($count -ne '1') {
            throw "TEST-CAR-001 kaydı eklenemedi (bulunan '$count')."
        }

        $Script:DbSizeStep3 = (Get-Item -LiteralPath $Script:DbPath).Length
        if ($Script:DbSizeStep3 -le 0) {
            throw 'Veritabanı dosyası boş.'
        }
        Write-Log "Otomatik test verisi hazır: TEST-CAR-001 eklendi, DB $($Script:DbSizeStep3) bayt." 'OK'
        return
    }

    Write-Host ''
    Write-Host '  Uygulama açılıyor...' -ForegroundColor Cyan
    Start-Process -FilePath $Script:ExePath
    Write-Host ''
    Write-Host '  Lütfen uygulamada sırayla:' -ForegroundColor Yellow
    Write-Host '   1) İlk yönetici hesabını oluşturun  (kullanıcı adı: testadmin, şifre: Test1234!)'
    Write-Host '   2) Kurulum sihirbazını atlayın'
    Write-Host '   3) 1 adet cari hesap ekleyin  (kod: TEST-CAR-001, ad: Test Cari)'
    Write-Host '   4) Uygulamayı kapatın'
    Write-Host ''
    $null = Read-Host '  Hazır olduğunuzda Enter''a basın'

    if (Get-Process -Name 'OnMuhasebe' -ErrorAction SilentlyContinue) {
        throw 'Uygulama hâlâ açık. Lütfen uygulamayı kapatıp scripti tekrar çalıştırın.'
    }
    if (-not (Test-Path -LiteralPath $Script:DbPath)) {
        throw "Veritabanı dosyası bulunamadı: $($Script:DbPath)"
    }

    $Script:DbSizeStep3 = (Get-Item -LiteralPath $Script:DbPath).Length
    if ($Script:DbSizeStep3 -le 0) {
        throw 'Veritabanı dosyası boş.'
    }
    Write-Log "İlk veri oluştu: $($Script:DbPath) ($($Script:DbSizeStep3) bayt)" 'OK'
}

function Build-Msi101 {
    if (Test-Path -LiteralPath $Script:CargoBackup) {
        throw 'Cargo.toml.bak zaten var. Önceki başarısız testten kalmış olabilir; kontrol edin.'
    }

    Copy-Item -LiteralPath $Script:CargoToml -Destination $Script:CargoBackup -Force
    $text = Get-Content -LiteralPath $Script:CargoToml -Raw
    $updated = [regex]::Replace($text, '(?m)^version\s*=\s*"1\.0\.0"(?=\r?$)', 'version = "1.0.1"')
    if ($updated -eq $text) {
        throw 'Cargo.toml içinde version = "1.0.0" satırı bulunamadı.'
    }
    [System.IO.File]::WriteAllText($Script:CargoToml, $updated, (New-Object System.Text.UTF8Encoding($false)))
    Write-Log 'Cargo.toml geçici olarak 1.0.1 yapıldı (yedek: Cargo.toml.bak).' 'OK'

    $env:PATH = "$($Script:WixBin);$env:PATH"
    $batch = Join-Path $Script:Root 'build-msi.bat'
    $process = Start-Process -FilePath 'cmd.exe' -ArgumentList '/c', "`"$batch`"" `
        -WorkingDirectory $Script:Root -Wait -PassThru -NoNewWindow
    if ($process.ExitCode -ne 0) {
        throw "build-msi.bat başarısız (çıkış kodu $($process.ExitCode))."
    }
    if (-not (Test-Path -LiteralPath $Script:Msi101)) {
        throw 'dist\OnMuhasebe-1.0.1.msi oluşmadı.'
    }

    $size = (Get-Item -LiteralPath $Script:Msi101).Length
    Write-Log "1.0.1 MSI oluştu: $($Script:Msi101) ($size bayt)" 'OK'
}

function Install-Msi101 {
    $log = Join-Path $Script:LogDir 'upgrade-1.0.1.log'
    $code = Invoke-MsiExec "/i `"$($Script:Msi101)`" /qn /norestart /l*v `"$log`"" '1.0.1 yükseltmesi'
    if (-not (Test-MsiSuccess $code)) {
        Write-Log "Yükseltme logu: $log" 'FAIL'
        throw "1.0.1 yükseltmesi başarısız (çıkış kodu $code)."
    }
    Write-Log '1.0.1 yükseltmesi tamamlandı.' 'OK'
}

function Assert-DataPreserved {
    if (-not (Test-Path -LiteralPath $Script:DbPath)) {
        throw "Yükseltme sonrası veritabanı kayboldu: $($Script:DbPath)"
    }

    $size = (Get-Item -LiteralPath $Script:DbPath).Length
    if ($size -le 0) {
        throw 'Yükseltme sonrası veritabanı boş.'
    }
    if ($size -eq $Script:DbSizeStep3) {
        Write-Log "Veritabanı korundu: $size bayt (Adım 3 ile aynı)." 'OK'
    }
    else {
        Write-Log "Veritabanı korundu: $size bayt (Adım 3: $($Script:DbSizeStep3) bayt)." 'WARN'
    }

    $count = Invoke-Sqlite -Database $Script:DbPath -Query "SELECT COUNT(*) FROM customers WHERE code = 'TEST-CAR-001';"
    if ($null -ne $count) {
        $count = ([string]$count).Trim()
        if ($count -ne '1') {
            throw "TEST-CAR-001 kaydı beklenen 1, bulunan '$count'."
        }
        Write-Log 'SQLite doğrulaması: TEST-CAR-001 kaydı = 1.' 'OK'
        return
    }

    if ($Otomatik) {
        throw 'SQLite aracı (sqlite3.exe veya python.exe) bulunamadı; otomatik doğrulama yapılamıyor.'
    }

    Write-Log 'SQLite aracı bulunamadı; veri kontrolü kullanıcı onayıyla yapılacak.' 'WARN'
    Write-Host ''
    Write-Host '  Uygulama açılıyor; TEST-CAR-001 carisini gördüğünüzü onaylayın.' -ForegroundColor Yellow
    Start-Process -FilePath $Script:ExePath
    $null = Read-Host '  TEST-CAR-001 carisini gördüyseniz Enter''a basın'

    if (Get-Process -Name 'OnMuhasebe' -ErrorAction SilentlyContinue) {
        throw 'Uygulama hâlâ açık. Lütfen kapatıp scripti tekrar çalıştırın.'
    }
    Write-Log 'Kullanıcı verinin korunduğunu onayladı.' 'OK'
}

function Uninstall-TestProduct {
    $entries = @(Get-MuhasebeEntries)
    if ($entries.Count -eq 0) {
        Write-Log 'Kurulum zaten kaldırılmış (1605 senaryosu).' 'WARN'
        return
    }

    $log = Join-Path $Script:LogDir 'uninstall.log'
    foreach ($entry in $entries) {
        $code = Invoke-MsiExec "/x `"$($entry.ProductCode)`" /qn /norestart /l*v `"$log`"" 'Kaldırma'
        if ($code -eq 1605) {
            Write-Log 'Ürün zaten kaldırılmış (1605).' 'WARN'
            continue
        }
        if (-not (Test-MsiSuccess $code)) {
            throw "Kaldırma başarısız (çıkış kodu $code). Log: $log"
        }
    }
    Write-Log 'Kaldırma tamamlandı.' 'OK'
}

function Assert-Removed {
    $entries = @(Get-MuhasebeEntries)
    if ($entries.Count -ne 0) {
        Write-EntriesToLog 'Kaldırma sonrası kalan' $entries
        throw "Kaldırma sonrası Programlar listesinde $($entries.Count) giriş kaldı."
    }
    Write-Log 'Programlar listesinde On Muhasebe girişi kalmadı.' 'OK'

    if (Test-Path -LiteralPath $Script:InstallDir) {
        $remaining = @(Get-ChildItem -LiteralPath $Script:InstallDir -Force -ErrorAction SilentlyContinue)
        if ($remaining.Count -gt 0) {
            throw "Kaldırma sonrası dosyalar kaldı: $($Script:InstallDir)"
        }
        Write-Log "Kurulum klasörü boş kaldı: $($Script:InstallDir)" 'WARN'
    }
    else {
        Write-Log 'Kurulum klasörü silindi.' 'OK'
    }

    if (-not (Test-Path -LiteralPath $Script:DbPath)) {
        throw 'Kaldırma veritabanını sildi; kullanıcı verisi korunmalıydı.'
    }
    Write-Log "Veritabanı kaldırmadan sonra korundu: $($Script:DbPath)" 'OK'
}

function Clear-TestEnvironment {
    Write-Host ''
    if ($Otomatik) {
        $answer = 'E'
        Write-Log 'Otomatik mod: temizlik onayı otomatik verildi.' 'INFO'
    }
    else {
        $answer = Read-Host '  Test ortamı temizlensin mi? (E/H)'
    }
    if ($answer -notmatch '^(e|E|y|Y)') {
        Write-Log 'Temizlik kullanıcı tarafından atlandı.' 'WARN'
        Write-Host "  Test verisi      : $($Script:DataRoot)" -ForegroundColor Yellow
        Write-Host "  Gerçek veri yedeği: $($Script:DataBackup)" -ForegroundColor Yellow
        Write-Host '  Gerçek veriyi geri almak için yedek klasörü %LOCALAPPDATA%\OnMuhasebe olarak yeniden adlandırın.' -ForegroundColor Yellow
        return
    }

    if (Test-Path -LiteralPath $Script:DataRoot) {
        Remove-Item -LiteralPath $Script:DataRoot -Recurse -Force
        Write-Log 'Test verisi silindi.' 'OK'
    }
    if ($Script:HadData -and (Test-Path -LiteralPath $Script:DataBackup)) {
        Move-Item -LiteralPath $Script:DataBackup -Destination $Script:DataRoot -Force
        Write-Log 'Gerçek kullanıcı verisi geri yüklendi.' 'OK'
    }
    if (Test-Path -LiteralPath $Script:Msi101) {
        Remove-Item -LiteralPath $Script:Msi101 -Force
        Write-Log 'dist\OnMuhasebe-1.0.1.msi silindi.' 'OK'
    }
    Write-Log 'Test ortamı temizlendi.' 'OK'
}

function Show-Summary {
    Write-Log ('=' * 68) 'STEP'
    Write-Log 'ÖZET' 'STEP'

    $table = $Script:Results | Format-Table -AutoSize Step, Sonuc, Sure, Not | Out-String -Width 180
    Write-Host $table
    Add-Content -LiteralPath $Script:LogPath -Value $table -Encoding UTF8

    $failed = @($Script:Results | Where-Object { $_.Sonuc -ne 'OK' })
    if ($failed.Count -eq 0) {
        Write-Log 'TÜM ADIMLAR BAŞARILI.' 'OK'
    }
    else {
        Write-Log "$($failed.Count) adım başarısız." 'FAIL'
    }
    Write-Log "Log dosyası: $($Script:LogPath)" 'INFO'
}

function Invoke-Rollback {
    param([string]$Reason)

    Write-Log 'Geri alma başladı.' 'WARN'

    if ($Script:InstallAttempted) {
        try {
            $entries = @(Get-MuhasebeEntries)
            foreach ($entry in $entries) {
                Write-Log "Test kurulumu kaldırılıyor: $($entry.ProductCode)" 'WARN'
                $code = Invoke-MsiExec "/x `"$($entry.ProductCode)`" /qn /norestart /l*v `"$($Script:LogDir)\rollback-uninstall.log`"" 'Geri alma kaldırma'
                if (-not (Test-MsiSuccess $code) -and $code -ne 1605) {
                    Write-Log "Geri alma kaldırma çıkış kodu: $code" 'WARN'
                }
            }
        }
        catch {
            Write-Log "Geri alma kaldırma hatası: $($_.Exception.Message)" 'WARN'
        }

        try {
            $running = Get-Process -Name 'OnMuhasebe' -ErrorAction SilentlyContinue
            if ($running) {
                $running | Stop-Process -Force -ErrorAction SilentlyContinue
                Start-Sleep -Seconds 1
                Write-Log 'Açık OnMuhasebe süreci kapatıldı.' 'WARN'
            }
        }
        catch {
            Write-Log "Süreç kapatma hatası: $($_.Exception.Message)" 'WARN'
        }
    }

    try {
        if (Test-Path -LiteralPath $Script:DataRoot) {
            $failedPath = Join-Path $env:LOCALAPPDATA "OnMuhasebe.msi-test-failed-$($Script:Stamp)"
            Move-Item -LiteralPath $Script:DataRoot -Destination $failedPath -Force
            Write-Log "Test verisi ayrıldı: $failedPath" 'WARN'
        }
        if ($Script:HadData -and (Test-Path -LiteralPath $Script:DataBackup)) {
            Move-Item -LiteralPath $Script:DataBackup -Destination $Script:DataRoot -Force
            Write-Log 'Gerçek kullanıcı verisi geri yüklendi.' 'OK'
        }
    }
    catch {
        Write-Log "Veri geri yükleme hatası: $($_.Exception.Message)" 'FAIL'
        Write-Log "Gerçek veri yedeği burada: $($Script:DataBackup)" 'FAIL'
    }

    Restore-CargoToml
    Write-Log "TEST BAŞARISIZ: $Reason" 'FAIL'
    Write-Log "Log: $($Script:LogPath)" 'FAIL'
}

# ------------------------------------------------------------------ ana akış
New-Item -ItemType Directory -Path $Script:LogDir -Force | Out-Null

$started = Get-Date
Write-Host ''
Write-Host '================================================================' -ForegroundColor Cyan
Write-Host '  ÖN MUHASEBE - MSI KURULUM / YÜKSELTME / KALDIRMA TESTİ' -ForegroundColor Cyan
Write-Host '  Aşama 14-5' -ForegroundColor Cyan
Write-Host '================================================================' -ForegroundColor Cyan
Write-Host "  Proje      : $($Script:Root)"
Write-Host "  Log        : $($Script:LogPath)"
Write-Host "  Zaman      : $(Get-Date -Format 'dd.MM.yyyy HH:mm:ss')"
Write-Host ''

Write-Log "Test başladı. Proje kökü: $($Script:Root)" 'STEP'

try {
    Assert-Preconditions

    Invoke-TestStep 'ADIM 0 - Kullanıcı verisi yedeği' { Backup-UserData }
    Invoke-TestStep 'ADIM 1 - 1.0.0 kurulumu' { Install-Msi100 }
    Invoke-TestStep 'ADIM 2 - 1.0.0 doğrulama' { Assert-InstallState -ExpectedVersion '1.0.0' -Label '1.0.0' }
    Invoke-TestStep 'ADIM 3 - İlk kullanıcı verisi' { Initialize-TestData }
    Invoke-TestStep 'ADIM 4 - 1.0.1 MSI üretimi' { Build-Msi101 }
    Invoke-TestStep 'ADIM 5 - 1.0.1 yükseltmesi' { Install-Msi101 }
    Invoke-TestStep 'ADIM 6 - Yükseltme doğrulama' { Assert-InstallState -ExpectedVersion '1.0.1' -Label '1.0.1' }
    Invoke-TestStep 'ADIM 7 - Veri korundu doğrulama' { Assert-DataPreserved }
    Invoke-TestStep 'ADIM 8 - Kaldırma' { Uninstall-TestProduct }
    Invoke-TestStep 'ADIM 9 - Kaldırma doğrulama' { Assert-Removed }
    Invoke-TestStep 'ADIM 10 - Cargo.toml geri alma' { Restore-CargoToml }
    Invoke-TestStep 'ADIM 11 - Ortam temizliği' { Clear-TestEnvironment }
}
catch {
    Invoke-Rollback $_.Exception.Message
    Show-Summary
    Write-Host ''
    Write-Host "  TEST BAŞARISIZ. Log: $($Script:LogPath)" -ForegroundColor Red
    exit 1
}

$elapsed = (Get-Date) - $started
Write-Log ("Toplam süre: {0:N1} saniye" -f $elapsed.TotalSeconds) 'INFO'
Show-Summary

Write-Host ''
Write-Host '  Test tamamlandı.' -ForegroundColor Green
Write-Host "  Log dosyası: $($Script:LogPath)" -ForegroundColor Green
Write-Host '  Bu log dosyasını geliştiriciye iletin.' -ForegroundColor Green
Write-Host ''
exit 0
