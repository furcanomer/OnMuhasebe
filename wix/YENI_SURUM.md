# Yeni Sürüm Çıkarma Rehberi

Sürüm numarasının **tek kaynağı** `Cargo.toml` dosyasındaki `[package] version`
alanıdır. MSI sürümü `$(var.CargoPackageVersion)` ile buradan okunur; başka
hiçbir yerde sürüm numarası yazılmaz.

## 5 adım

1. `Cargo.toml` dosyasında `version = "1.0.1"` yap. **Başka yerde sürüm yazma.**
2. Proje kökünde `build-msi.bat` dosyasını çalıştır.
3. `dist\OnMuhasebe-1.0.1.msi` dosyası oluşur.
4. MSI dosyasını GitHub Release'e yükle (18. aşamada).
5. Kullanıcı MSI'yi indirip çift tıklar; eski sürüm otomatik kaldırılır,
   `%LOCALAPPDATA%\OnMuhasebe` altındaki veriler korunur.

## Sürüm biçimi

Windows Installer sürümü `Major.Minor.Build` biçimindedir:

- `Major` ve `Minor` en fazla 255 olabilir.
- `Build` en fazla 65535 olabilir.
- Örnek: `1.0.1`, `1.2.34`.

## "2 tane olma" sorunu neden çözülür?

- `UpgradeCode` sabittir: `B5E3F1A2-9C4D-4E8B-8F6A-1D2E3C4B5A69`.
- `Product Id="*"` sayesinde her derlemede yeni `ProductCode` üretilir.
- `MajorUpgrade AllowSameVersionUpgrades="yes" Schedule="afterInstallInitialize"`
  eski sürümü yeni sürüm kurulmadan önce kaldırır.
- `InstallScope="perMachine"` ve `ProgramFiles64Folder` tek makine geneli
  kurulum sağlar.

Sonuç: Programlar listesinde **tek giriş** kalır ve kullanıcı verisi korunur.
