# Sürdürücü Notları

Bu belge, projeyi yıllar sonra devralacak veya bakımını yapacak kişi içindir.
Kişisel bir projedir; destek "en iyi çaba" esasıyla verilir.

## 1. Yıllık Bakım

**Her 3 ayda**

- `cargo audit` çalıştır; güvenlik uyarısı varsa değerlendir.
- `cargo update` **dikkatli** kullan: `Cargo.lock` değişir, testler yeniden
  koşulmalı. Yayınlanmış bir sürümün lock dosyasını geriye dönük değiştirme.
- GitHub Actions CI'nin yeşil olduğunu doğrula (`gh run list --limit 5`).
- Açık issue'lara göz at.

**Her yıl**

- `rust-toolchain.toml` sürümünü güncelle (yeni kararlı Rust). Güncelleme
  sonrası tüm testler + MSI kur/kaldır testi tekrarlanmalı.
- Ekran görüntülerini gözden geçir; arayüz değiştiyse yenile.
- `archive/` paketini yeniden oluştur ve harici ortama kopyala.
- README ve belgelerdeki tarihleri güncelle.
- Bağımlılık lisanslarını (`THIRD_PARTY_LICENSES.md`) yeniden üret.

## 2. Kullanıcı Geri Bildirimi

- GitHub Issues haftalık taranır.
- Hata bildiriminde şu bilgiler istenir:
  - Sürüm (v1.0.1 gibi),
  - Windows sürümü,
  - `%LOCALAPPDATA%\OnMuhasebe\OnMuhasebe\data\logs\` altındaki günlük,
  - Yeniden üretme adımları,
  - Mümkünse ekran görüntüsü.
- Özellik istekleri `ROADMAP.md` ile karşılaştırılır; kapsam dışıysa nazikçe
  reddedilir.
- Yanıt süresi garantisi yoktur; "en iyi çaba" açıkça belirtilir.

## 3. Sürüm Çıkarma Kontrol Listesi

1. `Cargo.toml` içindeki `version` alanını artır (semver).
2. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
   `cargo test --all-targets` çalıştır. İsteğe bağlı: `-- --ignored` ile ağır
   testler.
3. `build-msi.bat` ile MSI üret (`dist\OnMuhasebe-<sürüm>.msi`).
4. `RELEASE_NOTES.md` içine yeni sürüm bölümünü ekle (en üste).
5. Değişiklikleri commit et; `v<sürüm>` etiketi oluştur.
6. `git push origin main --tags`, ardından `gh release create`.
7. README ve belgelerdeki sürüm/tarih bilgilerini güncelle.
8. `archive/` paketini yeniden oluştur (MSI + kaynak zip + belgeler +
   `SHA256SUMS.txt`).

Ek kontrol: MSI'ı yönetici izni olmadan `msiexec /a` ile çıkarıp içindeki
exe'nin statik olduğunu doğrula (`target\release` ile aynı boyut).

## 4. Acil Durumlar

**Güvenlik açığı**

1. Etkilenen sürümleri belirle.
2. Yamayı hazırla, regresyon testi ekle.
3. Yeni PATCH sürümü yayınla.
4. GitHub Security Advisory aç; kullanıcıya yükseltme çağrısı yap.

**Veri bozulması**

1. Etkilenen sürümü yayından çek (Release'i "pre-release" veya sil).
2. Hatayı yeniden üret; düzelt.
3. Regresyon testi ekle (bozuk veri senaryosu).
4. Kullanıcıya "yedekten dön" talimatı ver.

**Migration hatası**

1. Kullanıcıya panik yapmadan yedekten dönmesini söyle.
2. Migration'ı **düzeltme**; yeni bir migration ekle.
3. v7 → v8 geçişini gerçek veri kopyasıyla test et.

**Erken sürüm geri bildirimi**

Kullanıcı geri bildirimi hata bildirimi içeriyorsa öncelik verilir. Erken sürüm
etiketinin amacı tam olarak budur: gerçek kullanımdan gelen hataları hızla
kapatmak.

## 5. Uzun Vadeli (Yıl 5, 10, 20)

- Rust veya egui büyük bir sürüm atlarsa derleme kırılabilir. `Cargo.lock` +
  `rust-toolchain.toml` eski sürümü sabitler; yeni sürüme geçiş ayrı bir iş
  olarak planlanmalı.
- crates.io kapanırsa: `cargo vendor` çıktısı arşive eklenir.
- Windows API'leri geriye dönük uyumludur; Win32 tabanlı bir uygulama uzun
  yıllar çalışır. Yine de sanal makine veya uyumluluk modu gerekebilir.
- Proje terk edilirse BSD 3-Clause sayesinde fork yaşar; kod ve belgeler
  arşivde olduğu sürece topluluk devam ettirebilir.
- Veritabanı SQLite olduğu için veri, uygulamadan bağımsız olarak okunabilir.
  Bu, en güçlü uzun vadeli güvencedir.

## 6. Katkı Yönetimi

- PR küçük ve tek konulu olmalı; her PR test içermeli.
- `cargo fmt` ve `clippy -D warnings` temiz olmalı.
- AI destekli katkılarda `CONTRIBUTING.md` içindeki beyan kuralı uygulanır.
- Reddetme gerekçeleri: kapsam dışı, lisans uyumsuz (GPL/AGPL bağımlılık),
  testleri geçmiyor, geriye uyumsuzluk.
- Commit mesajları Türkçe veya İngilizce olabilir; tek dilde tutarlılık tercih
  edilir.

## 7. Yedekleme ve Arşiv

- `archive/*.zip` en az üç yerde tutulmalı: harici disk, bulut, fiziksel ortam.
- `Cargo.lock` + `rust-toolchain.toml` derleme tekrarlanabilirliğini sağlar;
  bunlar olmadan "20 yıl sonra derle" hedefi zayıflar.
- `SHA256SUMS.txt` ile bütünlük doğrulanır; arşiv her yenilendiğinde yeniden
  üretilir.
- `.onm-backup` dosyaları kullanıcıya aittir; sürdürücü kopyası tutulmaz.

Son güncelleme: 2026-10-29
