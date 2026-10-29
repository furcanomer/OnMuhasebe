# Ön Muhasebe 1.0.0 — 20 Yıl Arşiv Notları

Bu arşiv, Ön Muhasebe 1.0.0 sürümünü uzun vadede yeniden üretebilmek veya
çalıştırabilmek için hazırlanmıştır.

## Neler Var

- `OnMuhasebe-1.0.0.msi` — Kurulabilir Windows paketi (Windows 10+ x64)
- `OnMuhasebe-1.0.0-kaynak.zip` — Tam kaynak kodu (`git archive HEAD`)
- `Cargo.lock` — Tüm bağımlılıkların pinlenmiş sürümleri
- `rust-toolchain.toml` — Derleme için gereken tam Rust sürümü (1.99.1)
- `.cargo/config.toml` — MSVC çalışma zamanını statik bağlayan derleme ayarı
- Tüm belgeler (README, LICENSE, `docs/`, AI/dev rehberleri, ekran görüntüleri)
- `SHA256SUMS.txt` — Arşivdeki her dosyanın SHA-256 özeti

## Sadece .exe'yi Çalıştırmak İçin

MSI'ı kurun; başka bir şey gerekmez. MSI içindeki `OnMuhasebe.exe` **statik
bağlantılıdır** (`-C target-feature=+crt-static`); VC++ Redistributable
GEREKMEZ. Windows 10 veya üzeri yeterlidir.

Doğrulama: `target\release` klasöründe hiç `.dll` yoktur ve exe'nin içe
aktardığı DLL listesinde `VCRUNTIME140.dll` / `MSVCP140.dll` bulunmaz; yalnızca
Windows sistem DLL'leri (KERNEL32, USER32, ADVAPI32, ole32, dwrite, dxgi,
d3d12 vb.) görünür.

## Sıfırdan Derlemek İçin (20 yıl sonra)

1. rustup kurun (<https://rustup.rs/>).
2. Bu arşivdeki `rust-toolchain.toml` dosyasını proje köküne koyun (rustup
   otomatik olarak doğru sürümü indirir).
3. Kaynak zip'ini açın.
4. `.cargo/config.toml` dosyasını proje kökündeki `.cargo/` klasörüne koyun
   (statik bağlantı için).
5. `cargo build --release` çalıştırın.
6. `target\release\OnMuhasebe.exe` oluşur.

MSI'yı yeniden üretmek isterseniz: `build-msi.bat` (WiX Toolset v3 gerekir;
`cargo install cargo-wix` ile kurulur).

## Bağımlılıklar

`Cargo.lock` tam pinlidir; `cargo build` bu sürümleri kullanır.

crates.io 20 yıl sonra hâlâ ayakta olmayabilir; o durumda:

- crates.io dizin arşivi (GitHub'da `rust-lang/crates.io-index`) üzerinden eski
  sürümler erişilebilir.
- Alternatif: bu arşive bağımlılıkların vendor hâlini eklemek (`cargo vendor`
  komutu). İhtiyaç duyulursa 1.0.1'de eklenebilir.

## Veriler ve Yedekleme

- Tüm veriler yerel SQLite dosyasındadır (`on_muhasebe.db`); sunucu veya bulut
  bağımlılığı yoktur.
- Yedekler `.onm-backup` uzantılıdır ve SQLite veritabanının bayt kopyasıdır;
  herhangi bir SQLite aracıyla 20 yıl sonra da açılabilir.
- Varsayılan veri dizini: `%LOCALAPPDATA%\OnMuhasebe\`.

## Lisans

BSD 3-Clause. Telif hakkı (c) 2026, @furcanomer. Üçüncü taraf bağımlılıkları
için `THIRD_PARTY_LICENSES.md` dosyasına bakın.

## Yapay Zeka Beyanı

Kaynak kodu ve belgeler, @furcanomer'in yönlendirmesi ve denetimi altında yapay
zeka (LLM) desteğiyle geliştirilmiştir.

## Kod İmzalama

1.0.0 imzasızdır; Windows SmartScreen "bilinmeyen yayıncı" uyarısı gösterebilir.
Bu normaldir. İleride imzalamak isteyenler için ayrıntılı rehber:
`KOD_IMZALAMA.md`.

## Derleme Bilgileri

| Kalem | Değer |
|-------|-------|
| Sürüm | 1.0.0 |
| Rust | 1.99.1 (2026-10-13) |
| Hedef | x86_64-pc-windows-msvc |
| Profil | release (optimize) |
| Statik CRT | Açık (`-C target-feature=+crt-static`) |
| `OnMuhasebe.exe` boyutu | 20.065.792 bayt |
| MSI içindeki exe | Aynı boyut ve statik bağlantı (DLL importlarında `VCRUNTIME140`/`MSVCP140` yok) |

MSI, bu statik exe ile 29.10.2026 tarihinde yeniden üretilmiştir (WiX Toolset
3.14.1). MSI'dan yönetici izni olmadan çıkarılan (`msiexec /a`) `OnMuhasebe.exe`
20.065.792 bayttır ve statik bağlantılıdır.

Not: Rust derlemeleri bayt düzeyinde her zaman birebir aynı çıkmayabilir (PE
zaman damgası ve derleme sırası). Bu yüzden aynı kaynaktan yapılan yeni bir
derlemenin SHA-256 değeri farklı olabilir; işlev ve bağımlılıklar aynıdır.
Doğrulama için `SHA256SUMS.txt` her zaman arşivdeki **fiili dosyaların**
özetlerini taşır.

## Doğrulama

Tüm dosyaların SHA-256 özetleri `SHA256SUMS.txt` içindedir.

PowerShell ile doğrulama:

```powershell
Get-FileHash <dosya> -Algorithm SHA256
```

Çıktıyı `SHA256SUMS.txt` içindeki satırla karşılaştırın.

Son güncelleme: 2026-10-29
