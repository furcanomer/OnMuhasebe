# 2.0'a Geçiş Rehberi

Bu belge, Ön Muhasebe 1.x sürümlerinden 2.0'a geçişi planlayan geliştirici ve
kullanıcılar içindir. 1.0.1 (erken sürüm) ile birlikte yayınlanmıştır.

## 1. Ne Zaman 2.0

Sürüm numarası `MAJOR.MINOR.PATCH` biçimindedir (semver):

| Bileşen | Ne zaman artar | Örnek |
|---------|----------------|-------|
| MAJOR | Geriye **uyumsuz** değişiklik | 2.0.0 |
| MINOR | Geriye uyumlu yeni özellik | 1.1.0 |
| PATCH | Hata düzeltmesi | 1.0.2 |

1.x hattı boyunca hedef: hata düzeltme ve küçük iyileştirmeler. Veritabanı
şeması yalnızca **eklemeli** migration ile büyür; mevcut migration dosyaları
değiştirilmez.

2.0 şu durumlarda gündeme gelir:

- Veritabanının SQLite'tan PostgreSQL'e taşınması (çok kullanıcılı ağ senaryosu).
- Arayüz katmanının tamamen yeniden yazılması (ör. egui'dan farklı bir
  teknolojiye geçiş).
- Veri modelinin geriye uyumsuz biçimde yeniden düzenlenmesi (para birimi,
  çoklu şirket anahtarları vb.).
- Eski `.onm-backup` biçiminin artık okunamaması.

Tek bir özellik eklemek 2.0 gerektirmez. "Yeni özellik = MINOR" kuralı geçerlidir.

## 2. 1.0 Verisini 2.0'a Taşıma

### 2.1. Veri yolu

| İçerik | Yol |
|--------|-----|
| Veritabanı | `%LOCALAPPDATA%\OnMuhasebe\OnMuhasebe\data\on_muhasebe.db` |
| Yedekler | `%LOCALAPPDATA%\OnMuhasebe\OnMuhasebe\data\backups\` |
| Günlükler | `%LOCALAPPDATA%\OnMuhasebe\OnMuhasebe\data\logs\` |

Taşınabilir kullanım için `ON_MUHASEBE_DATA_DIR` ortam değişkeniyle veri
dizini değiştirilebilir.

### 2.2. Yedek

Taşımadan önce **mutlaka** yedek alın:

1. **Ayarlar → Yedekleme → Yedek al** (`.onm-backup`).
2. Dosyayı bilgisayar dışında bir yere kopyalayın (USB disk, bulut).
3. Yedeğin açılabildiğini doğrulayın (geri yükleme ekranında listeleyin).

`.onm-backup` dosyası SQLite veritabanının bayt kopyasıdır; herhangi bir SQLite
aracıyla açılabilir. Bu, 20 yıl sonra bile veriye erişmenin garantisidir.

### 2.3. Yöntem

Önerilen sıra:

1. 1.x'te `.onm-backup` alın ve dosyayı saklayın.
2. 2.0'ı kurun (yeni MSI; eski sürüm kaldırılır, veri korunur).
3. 2.0'da **Ayarlar → Yedekleme → Geri yükle** ile 1.0 yedeğini içe aktarın.
4. 2.0 açılışta v7 → v8 (veya güncel) migration'ı otomatik uygular.
5. Ekranlarda veri sayılarını kontrol edin (cari, ürün, fatura, kasa).

2.0 geliştiricisi için: v7 → v8 migration'ı **ayrı bir test** ile doğrulanmalı.
Gerçek 1.0 veritabanı kopyasıyla açılış testi yapılmadan sürüm yayınlanmamalı.

### 2.4. Fallback: CSV

Migration başarısız olursa:

1. 1.x'i yeniden kurun (arşivdeki `OnMuhasebe-1.0.1.msi`).
2. Yedeği geri yükleyin.
3. Her ekrandan CSV dışa aktarın (cari, ürün, faturalar, kasa, gelir/gider).
4. 2.0'da CSV içe aktarma ile veriyi yeniden oluşturun.

CSV yolu daha yavaş ve hata yapmaya açıktır; yalnızca son çare olarak
kullanılmalıdır. İçe aktarma sonrası bakiyeler mutlaka karşılaştırılmalıdır.

## 3. 1.0'ı Yıllar Sonra Kullanma

2.0'a geçmek istemeyen veya eski veriyi açması gereken kullanıcı için:

**Saklanması gerekenler**

| Dosya | Neden |
|-------|-------|
| `OnMuhasebe-1.0.1.msi` | Kurulum (statik runtime, VC++ gerekmez) |
| `OnMuhasebe-1.0.1-kaynak.zip` | Yeniden derleme |
| `Cargo.lock` | Bağımlılık sürümlerinin pinlenmesi |
| `rust-toolchain.toml` | Derleyici sürümü (1.99.1) |
| `archive/` zip'i | Belgeler + görseller + notlar |
| `.onm-backup` dosyaları | Kullanıcı verisi |

**Saklama ortamı**

- Harici disk (en az iki kopya),
- Bulut (OneDrive/Drive/S3 vb.),
- Fiziksel ortam (yazdırılmış SHA-256 listesi + optik medya isteğe bağlı).

**Derleme tekrarlanabilirliği**

`Cargo.lock` + `rust-toolchain.toml` birlikte, aynı kaynaktan aynı bağımlılık
sürümleriyle derlemeyi mümkün kılar. `cargo build --release` yeterlidir.

**crates.io kapanırsa**

`cargo vendor` çıktısı arşive eklenirse bağımlılıklar yerelden sağlanabilir:

```text
cargo vendor vendor
```

Ardından `.cargo/config.toml` içine `[source.crates-io] replace-with = ...`
bölümü eklenir. Bu, 1.0.2 veya 1.1 için planlanan açık uçtur.

**Windows tarafı**

Win32 API'leri geriye dönük uyumludur; Windows 10+ için yazılmış bir exe,
gelecekteki Windows sürümlerinde çalışmaya devam eder (30+ yıllık uyumluluk
geçmişi). 32 bit desteği hiç eklenmedi; x64 varsayılan.

## 4. 2.0 İçin Öneriler

**Veritabanı.** SQLite bu uygulamanın ölçeği için yeterlidir. PostgreSQL
yalnızca gerçek çok kullanıcılı ağ senaryosunda gerekir; bu, 2.0'ın en büyük
gerekçesi olabilir. Geçiş yapılırsa migration katmanı soyutlanmalı ve SQLite
yolu korunmalıdır.

**Arayüz.** egui büyük bir sürüm atlarsa derleme kırılabilir. Arayüz kodu
mümkün olduğunca egui'ya bağımlı olmayan bir "durum" katmanı üzerinden
yazılmalıdır. Şu an bu ayrım kısmen vardır; 2.0'da tamamlanabilir.

**e-Fatura.** GİB onaylı özel entegratör gerekir. Entegrasyon, bir trait
(`InvoiceProvider`) arkasında soyutlanmalı; sağlayıcı değişince iş mantığı
değişmemelidir.

**Çoklu şirket.** Tüm tablolara `company_id` eklenmesi ve her sorguya filtre
konması gerekir. Büyük bir refactor'dür; migration ile birlikte planlanmalıdır.

**Döviz.** `currency` alanı + kur tablosu + kur tarihi gerekir. Orta ölçekli
bir refactor'dür; rapor ve fatura toplamları etkilenir.

**Kod imzalama.** 2.0 ile birlikte ücretli sertifika alınırsa SmartScreen
uyarısı azalır. Ayrıntılar: `KOD_IMZALAMA.md`.

## 5. 1.0'dan 1.1'e (Ufak Sürüm)

MINOR sürüm geriye uyumludur:

- Yeni ekran, yeni rapor, yeni dışa aktarma biçimi eklenebilir.
- Veritabanına yeni tablo/kolon eklenebilir; **eski migration değiştirilmez**.
- Açılışta otomatik migration çalışır; kullanıcıdan işlem istenmez.
- Kullanıcı verisi korunur; `.onm-backup` biçimi değişmez.

Kural: aynı `UpgradeCode`, yeni `ProductCode` ve artan `version`. MSI Major
Upgrade ile eski sürümün üzerine kurulur.

## 6. Kullanıcıya Bilgi

Her sürümde:

1. `RELEASE_NOTES.md` güncellenir: ne değişti, neden değişti, veri taşıma
   gerekip gerekmediği.
2. GitHub Release açıklaması aynı metinle yayınlanır.
3. Veri taşıma gerekiyorsa adım adım anlatılır ve yedek uyarısı verilir.
4. Geri dönüş yolu (eski MSI) belirtilir.

2.0 duyurusunda ayrıca:

- 1.x'in desteklenmeye devam edip etmeyeceği,
- veri taşımanın tek yönlü olup olmadığı (2.0 → 1.x geri dönüş yok),
- tahmini geçiş süresi ve riskleri

açıkça yazılmalıdır.

Son güncelleme: 2026-10-29
