# AI Devralma Belgesi — Ön Muhasebe

> **Bu dosya, projeyi devralacak bir yapay zekâya yöneliktir.** İnsan geliştirici
> rehberi için `GELISTIRICI_REHBERI.md` (17. aşamada eklenecek) dosyasına bakın.
> Belgeyi baştan sona okuyun; kod yazmadan önce 6. ve 8. bölümleri tekrar
> okuyun. Kopyala-yapıştır başlangıç promptları için `AI_PROMPTS.md`.

## 1. Özet

**Ön Muhasebe**, Windows üzerinde yerel (native) masaüstü uygulaması olarak çalışan
bir ön muhasebe programıdır. Cari hesapları, ürün ve stoğu, alış/satış faturalarını,
kasa/banka hareketlerini, tahsilat/ödemeyi, gelir/gider kayıtlarını ve altı raporu
tek bir çevrimdışı programda toplar.

Teknoloji yığını **Rust + egui/eframe 0.37 + SQLite (rusqlite, bundled)** şeklindedir.
Web view, Tauri, Electron, tarayıcı veya sunucu yoktur; çizim doğrudan Win32/WGPU
üzerinden yapılır. Veriler kullanıcının bilgisayarında, yerel SQLite dosyasında
tutulur ve hiçbir veri internete gönderilmez.

Sürüm **1.0.0**, veritabanı şeması **v7**'dir. Lisans **BSD 3-Clause**, telif hakkı
**(c) 2026, @furcanomer**. Proje Git deposu değildir; sürüm takibi `Cargo.toml`
içindeki `version` alanıyla yapılır.

Uygulama günlük kullanıma hazırdır: 9 ekran, 6 rapor, 4 ayar sekmesi, 2 kullanıcı
rolü, otomatik/manuel yedekleme, CSV içe/dışa aktarma ve MSI kurulum paketi vardır.
453 testin tamamı (432 hızlı + 21 ağır) bu sürümde geçmektedir.

## 2. Teknoloji ve Sürümler

| Kalem | Değer |
|-------|-------|
| Dil | Rust, **edition 2024** |
| Doğrulanan araç zinciri | `rustc 1.99.1`, `cargo 1.99.1` (Windows x64) |
| Paket adı | `on_muhasebe` (kütüphane) / `OnMuhasebe` (ikili) |
| Sürüm | `1.0.0` (`Cargo.toml`) |
| Arayüz | `eframe` 0.37.0, `egui` 0.37.0 |
| Veritabanı | `rusqlite` 0.40.2, **`bundled`** (SQLite kaynaktan derlenir) |
| Tarih/saat | `chrono` 0.4.45 (`clock`, `std`) |
| Hata | `thiserror` 2.0.21 |
| Veri dizini | `directories` 6.0.0 |
| Şifre | `argon2` 0.5 (Argon2id, PHC) + `rand_core` 0.6 (`getrandom`) |
| Günlük | `tracing` 0.1, `tracing-subscriber` 0.3 (`env-filter`), `tracing-appender` 0.2 |
| Dosya diyalogları | `rfd` 0.17.2 |
| Build bağımlılığı | `winres` 0.1 (yalnızca exe dosya özellikleri) |
| Test bağımlılıkları | `tempfile` 3, `sysinfo` 0.37 |

**Hedef platform:** Windows 10 veya üzeri, **x64**, native. Minimum pencere
**900×600**, varsayılan **1180×760**. Release profili: `opt-level = 3`,
`lto = "thin"`, `codegen-units = 1`, `strip = "symbols"`.

SQLite programın içine gömülüdür; kullanıcı makinesinde ayrı DLL veya veritabanı
kurulumu gerekmez. `eframe` varsayılan olarak `wgpu` çizim arka ucunu kullanır.

## 3. Proje Klasör Yapısı ve Dosya Görevleri

### 3.1. Kök

| Yol | Görev |
|-----|-------|
| `Cargo.toml`, `Cargo.lock` | Paket tanımı ve kilitli bağımlılıklar |
| `build.rs` | Windows exe meta verisi (winres) |
| `README.md` | Teknik özet ve belge bağlantıları |
| `LICENSE` | BSD 3-Clause metni |
| `AI_DEVRALMA.md` | Bu belge |
| `AI_PROMPTS.md` | Kopyala-yapıştır başlangıç promptları |
| `build-msi.bat` | Release derleme + MSI üretimi |
| `dist/` | Üretilen `OnMuhasebe-<sürüm>.msi` |
| `.gitignore` | `target/`, `dist/`, `*.db`, `*.csv`, `*.msi`, log ve yedek artıkları |
| `assets/icon.ico` | Pencere/uygulama simgesi |
| `docs/` | Son kullanıcı belgeleri |
| `tests/` | Entegrasyon ve ağır testler |
| `wix/` | WiX kaynağı ve lisans RTF'i |
| `SONUC_RAPORU.txt` | Aşama sonuç raporu |

### 3.2. `src/` — çekirdek

| Dosya | Görev |
|-------|-------|
| `main.rs` | Giriş noktası; yalnızca yapılandırma + log + pencere kurar |
| `lib.rs` | Kütüphane kökü, modül kayıtları, `APP_NAME`, `APP_VERSION`, `GIT_HASH`, `DATABASE_FILE_NAME` |
| `error.rs` | `AppError` enum, `Result` alias, `sqlite_error_to_turkish` |
| `config.rs` | Veri dizini çözümlemesi (`ON_MUHASEBE_DATA_DIR` veya `%LOCALAPPDATA%`) |
| `money.rs` | `Money` (i64 kuruş): parse/format, checked aritmetik, yüzde |
| `quantity.rs` | `Quantity` (i64 mili-birim): parse/format, checked aritmetik |
| `customer.rs` | Cari iş modeli, `CustomerType`, `Customer::balance()` |
| `product.rs` | Ürün/stok iş modeli, `Unit`, `MovementType` |
| `invoice.rs` | Fatura iş modeli, `calculate_line`, `InvoiceTotals`, `PaymentState` |
| `payment.rs` | Tahsilat/ödeme iş modeli, dağıtım kuralları, `allocate_all` |
| `cash.rs` | Kasa/banka iş modeli, `IBAN_LEN = 26`, IBAN doğrulama |
| `income_expense.rs` | Gelir/gider iş modeli, kategori doğrulama, hazır kategoriler |
| `report.rs` | Rapor alan modeli (saf hesaplama, SQL yok) |
| `auth.rs` | Kullanıcı/şifre (Argon2id), `Actor`, `Role`, giriş kilidi |
| `backup.rs` | Yedek adlandırma, oluşturma, listeleme, rotasyon |
| `csv.rs` | CSV yazıcı/okuyucu (`;`, UTF-8 BOM, virgül ondalık) |
| `restore_journal.rs` | Geri yükleme günlüğü (`restore.journal`) |
| `settings.rs` | `app_settings` anahtar-değer yönetimi |
| `onboarding.rs` | İlk çalıştırma sihirbazının saf veri modeli |
| `logging.rs` | Günlük dosyası, 10 MB rollover, 30 gün temizliği, panik kaydı |
| `icon.rs` | Gömülü pencere simgesi |

### 3.3. `src/db/` — SQL katmanı

| Dosya | Görev |
|-------|-------|
| `mod.rs` | `Database::open`, PRAGMA'lar, migration uygulama, `require_admin` |
| `migrations.rs` | `MIGRATIONS` listesi (v1..v7) ve şema SQL'i |
| `customers.rs` | Cari CRUD, filtre, bakiye toplamları |
| `products.rs` | Ürün CRUD, stok hareketi + `current_stock` (tek transaction) |
| `invoices.rs` | Fatura kesme/iptal, satırlar, stok ve `paid_amount` etkisi |
| `payments.rs` | Tahsilat/ödeme, dağıtımlar, kasa bakiyesi, iptal |
| `cash.rs` | Kasa/banka hesap CRUD, bakiye |
| `income_expense.rs` | Kategori ve gelir/gider hareketleri, kasa etkisi |
| `users.rs` | Kullanıcı CRUD, şifre özeti, son giriş, son yönetici koruması |
| `settings.rs` | `app_settings` okuma/yazma |
| `reports.rs` | 6 raporun salt okunur sorguları |
| `search.rs` | Global arama (grup başına 5, toplam 20) |
| `transfer.rs` | CSV içe aktarma, yedek dışa aktarma/geri yükleme |
| `onboarding.rs` | Sihirbazın tek transaction'lık yazımı |

### 3.4. `src/app/` — uygulama durumu ve komutlar

| Dosya | Görev |
|-------|-------|
| `../app.rs` | `OnMuhasebeApp` (`eframe::App`), `View`, tüm ekran durumları |
| `auth_screen.rs` | Giriş, ilk yönetici, kilit ekranı, oturum |
| `customer_screen.rs` | Cari listesi/formu/ayrıntısı komutları |
| `product_screen.rs` | Ürün, stok hareketi, Genel Bakış metrikleri |
| `invoice_screen.rs` | Fatura listesi/formu/ayrıntısı/iptali |
| `cash_screen.rs` | Kasa hesapları, tahsilat/ödeme, dağıtım |
| `income_expense_screen.rs` | Gelir/gider hareketleri, kategoriler, özet |
| `report_screen.rs` | 6 raporun filtre ve yenileme durumu |
| `settings_screen.rs` | Ayarlar sekmeleri, kullanıcı işlemleri, şifre |
| `export_screen.rs` | Liste ve rapor CSV dışa aktarma |
| `transfer_screen.rs` | CSV içe aktarma önizlemesi, yedek geri yükleme |
| `search_screen.rs` | Global arama paleti |
| `navigation_links.rs` | Ekranlar arası çapraz gezinme |
| `onboarding_screen.rs` | 4 adımlı kurulum sihirbazı durumu |
| `shortcuts.rs` | 17 kısayol tanımı ve yönlendirme |
| `stage9_tests.rs` | Uygulama katmanı testleri (`#[cfg(test)]`) |

### 3.5. `src/ui/` — çizim katmanı

| Dosya | Görev |
|-------|-------|
| `mod.rs` | Arayüz kökü ve modül listesi |
| `layout.rs` | Ana yerleşim, üst menü, Hakkında, hata ayrıntıları |
| `navigation.rs` | Sol menü |
| `theme.rs` | Renk paleti (`Palette`), ölçüler (`metrics`), stil |
| `widgets.rs` | Kart, başlık, satır etiketi, arama kutusu, hata kartı |
| `format.rs` | Tarih ve bayt biçimlendirme |
| `overview.rs` | Genel Bakış ekranı |
| `customers.rs` | Cari liste/form/ayrıntı |
| `products.rs` | Ürün liste/form/ayrıntı/stok hareketi |
| `invoices.rs` | Fatura liste/form/ayrıntı |
| `cash.rs` | Kasa/banka ve hareketler |
| `income_expense.rs` | Gelir/gider hareketleri, kategoriler, özet |
| `reports.rs` | 6 rapor sekmesi |
| `settings.rs` | 4 ayar sekmesi |
| `transfer.rs` | İçe aktarma ve geri yükleme pencereleri |
| `overlays.rs` | Kısayollar, arama, silme/iptal onayları, sihirbaz |
| `auth.rs` | Giriş, ilk yönetici, kilit ekranı |
| `virtual_list.rs` | Sanallaştırılmış liste (yalnızca görünür satır çizilir) |
| `placeholder.rs` | Geliştirilmemiş bölüm bilgi ekranı (şu an kullanılmıyor) |

### 3.6. `tests/` — testler

| Yol | İçerik |
|-----|--------|
| `database.rs`, `customers.rs`, `products.rs`, `invoices.rs` | Entegrasyon testleri |
| `payments.rs`, `cash.rs`, `income_expense.rs`, `reports.rs` | Entegrasyon testleri |
| `stage11.rs`, `stage12.rs` | Aşama testleri |
| `stage14_license.rs`, `stage14_restore_journal.rs` | Lisans ve geri yükleme günlüğü |
| `stress.rs` → `stress/mod.rs` | Ağır test paketi (21 `#[ignore]` test) |
| `stress/*.rs` | `integrity`, `migration_chain`, `recovery`, `fuzz`, `soak`, `scale_up`, `bulk_data`, `failure_injection`, `real_world`, `ui_health` |
| `msi_install_test.ps1` | Gerçek MSI kur/yükselt/kaldır testi (yönetici) |
| `msi-test-logs/` | 14-5 aşamasının kanıt günlükleri |

### 3.7. `wix/` ve `docs/`

| Yol | Görev |
|-----|-------|
| `wix/main.wxs` | WiX 3.14 kaynağı: perMachine, x64, ProgramFiles64Folder |
| `wix/LICENSE.rtf` | Kurulum sihirbazındaki lisans metni |
| `wix/YENI_SURUM.md` | Sürüm yükseltme adımları |
| `docs/KURULUM.md` | Kurulum, ilk açılış, güncelleme, kaldırma |
| `docs/KULLANIM_KILAVUZU.md` | Tüm ekranlar ve iş akışları |
| `docs/SSS.md` | 24 soru-cevap |
| `docs/SORUN_GIDERME.md` | Belirti / neden / çözüm senaryoları |
| `docs/EKRAN_GORUNTULERI/README.md` | Ekran görüntüsü rehberi |

## 4. Mimari ve Katman Sınırları

Bağımlılık yönü **tek yönlüdür**:

```text
main → app → ui
          → db → error
          → config / money / quantity
          → customer → money / error
          → product  → money / quantity / customer / error
          → invoice  → money / quantity / product / customer / error
          → cash     → money / error
          → payment  → money / customer / invoice / error
          → income_expense → money / customer / error
          → report   → money / quantity / customer / product / invoice
                       / payment / cash / income_expense / error
```

| Kural | Açıklama |
|-------|----------|
| UI'da SQL yok | `src/ui/*` hiçbir SQL çalıştırmaz; veriyi `OnMuhasebeApp` metotlarından okur |
| İş modeli saf | `customer.rs`, `product.rs`, `invoice.rs` vb. SQL ve GUI bilmez |
| Ekran durumu köprü | `src/app/*_screen.rs` UI ile `db` arasında durum ve komut sağlar |
| Tek veri kaynağı | `OnMuhasebeApp` arayüzün tek durum kaynağıdır |
| Hata tek tip | Tüm katmanlar `AppError` döner; `?` her yerde çalışır |
| Yetki denetimi | Yazma işlemleri `Database::require_admin` / `require_self_or_admin` kullanır |

Yeni ekran eklerken tipik akış: `View` varyantı → `ui/mod.rs` + `ui/layout.rs`
dağıtımı → `app/<yeni>_screen.rs` durumu → `ui/<yeni>.rs` çizimi → gerekirse
`db/<yeni>.rs` ve `src/<yeni>.rs`.

## 5. Veri Modeli (şema v7)

### 5.1. Tablolar

| Tablo | Sürüm | Kritik alanlar / kısıtlar |
|-------|-------|---------------------------|
| `schema_migrations` | altyapı | `version` PK, `name`, `applied_at` |
| `app_metadata` | 1 | `key` PK, `value`, `updated_at` |
| `customers` | 2 | `code` UNIQUE `NOCASE`, `kind` ∈ (`musteri`,`tedarikci`,`ikisi`), `active` 0/1 |
| `products` | 3 | `code` UNIQUE `NOCASE`, kısmi UNIQUE `barcode`, `unit` ∈ 11 değer, fiyatlar kuruş, `current_stock` mili-birim |
| `product_stock_movements` | 3 → 4 | `movement_type` ∈ (`ilk_stok`,`giris`,`cikis`,`duzeltme`,`alis_faturasi`,`satis_faturasi`), `quantity_delta ≠ 0`, `invoice_id` nullable FK |
| `invoices` | 4 → 5 | `invoice_number` UNIQUE `NOCASE`, `invoice_type` ∈ (`alis`,`satis`), `status` ∈ (`kesin`,`iptal`), `paid_amount` (v5) |
| `invoice_lines` | 4 | `UNIQUE(invoice_id, line_no)`, ürün kodu/adı/birim **kopya**, `quantity ≠ 0`, `discount_rate`/`vat_rate` 0–100 |
| `cash_accounts` | 5 | `name` UNIQUE `NOCASE`, `account_type` ∈ (`kasa`,`banka`), `opening_balance ≥ 0`, `current_balance` işaretli |
| `payments` | 5 | `payment_type` ∈ (`tahsilat`,`odeme`), `amount > 0`, `status` ∈ (`kesin`,`iptal`) |
| `payment_allocations` | 5 | `UNIQUE(payment_id, invoice_id)`, `amount > 0`, `ON DELETE CASCADE` |
| `expense_categories` | 6 | `UNIQUE(name, category_type)`, `category_type` ∈ (`gelir`,`gider`) |
| `income_expense_entries` | 6 | `entry_type` ∈ (`gelir`,`gider`), `amount > 0`, `status` ∈ (`kesin`,`iptal`) |
| `users` | 7 | `username` UNIQUE `NOCASE`, `password_hash` Argon2id PHC, `role` ∈ (`yonetici`,`operator`) |
| `app_settings` | 7 | `key` PK, `value`, `updated_at` |

Tüm kimlikler `INTEGER PRIMARY KEY AUTOINCREMENT`'tır; silinen kimlik yeniden
kullanılmaz. Para alanları **kuruş (i64)**, miktar alanları **mili-birim (i64)**
olarak saklanır. Tarihler `TEXT` (`YYYY-MM-DD`), zaman damgaları
`YYYY-MM-DD HH:MM:SS` biçimindedir.

### 5.2. Korunması zorunlu korelasyonlar

```text
products.current_stock
  == SUM(product_stock_movements.quantity_delta)          (her ürün için)

invoices.paid_amount
  == SUM(payment_allocations.amount)                       (yalnızca kesin ödemeler)

cash_accounts.current_balance
  == opening_balance
   + Σ kesin tahsilat + Σ kesin gelir
   − Σ kesin ödeme    − Σ kesin gider

Customer::balance()
  == Σ kesin satış − Σ kesin alış
   − Σ kesin tahsilat + Σ kesin ödeme
```

Ayrıca: iptal edilmiş faturanın `paid_amount` değeri **0** olmalıdır. Bu dört
korelasyon `tests/stress/mod.rs` içindeki `assert_correlations()` ile denetlenir
(`PRAGMA integrity_check` ve `foreign_key_check` dâhil).

### 5.3. Fatura satırı formülü

```text
gross    = quantity × unit_price
discount = gross × discount_rate / 100
net      = gross − discount
vat      = net × vat_rate / 100
total    = net + vat
```

Ara sonuçlar `i128` ile hesaplanır, yarım kuruş **yukarı** yuvarlanır, taşmada
`AppError` döner. Fatura toplamları satırlardan türetilir: `subtotal` (brüt),
`discount_total`, `net_total`, `vat_total`, `grand_total`.

### 5.4. PRAGMA ve dosya

```text
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;
PRAGMA synchronous = NORMAL;
PRAGMA journal_mode = WAL;
```

Veritabanı yolu: `%LOCALAPPDATA%\OnMuhasebe\OnMuhasebe\data\on_muhasebe.db`.
`ON_MUHASEBE_DATA_DIR` ortam değişkeni yolu geçici olarak değiştirir (test ve
taşınabilir kullanım). `-wal` ve `-shm` dosyaları SQLite'ın normal çalışma
dosyalarıdır.

## 6. Kritik Kurallar (BU KURALLARA UYULMAZSA PROJE BOZULUR)

1. **Para ve miktar asla `f32`/`f64` ile temsil edilmez.** `Money` (i64 kuruş) ve
   `Quantity` (i64 mili-birim) zorunludur. Kayan noktalı sayı yalnızca çizim/ölçü
   kodunda kullanılabilir; muhasebe hesabında kullanılamaz.
2. **Migration eklerken listenin SONUNA ekle.** `MIGRATIONS` dizisindeki mevcut
   v1..v7 kayıtları **değiştirilemez**. Versiyon artırılır, her migration kendi
   transaction'ında uygulanır.
3. **Kayıt silme yok.** Fatura, tahsilat/ödeme ve gelir/gider `status = 'iptal'`
   ile iptal edilir; cari, ürün, kasa/banka hesabı ve kategori `active = 0` ile
   pasife alınır. Tek istisna `users` tablosudur (yönetici kullanıcı silebilir).
4. **Çoklu tablo yazan işlemler tek transaction içinde olmalı.** Fatura + satırlar
   + stok hareketleri + `current_stock` + `paid_amount`; ödeme + dağıtımlar + kasa
   bakiyesi; gelir/gider + kasa bakiyesi. Hata → tamamı geri alınır.
5. **Testleri silme veya gevşetme.** Kırılan test yeni davranışa göre güncellenir
   ve gerekçesi rapora yazılır.
6. **UI katmanına SQL yazma.** Sorgular `src/db/` altında, çağrılar
   `OnMuhasebeApp` metotlarıyla yapılır.
7. **`current_stock` elle güncellenmez.** Stok hareketi eklenir; `current_stock`
   aynı transaction içinde hareketten türetilir.
8. **`invoices.paid_amount` elle set edilmez.** `payment_allocations` üzerinden
   hesaplanır; iptalde sıfırlanır.
9. **Yeni tablo/kolon migration'sız eklenmez.** Şema değişikliği her zaman yeni
   migration demektir.
10. **Hata mesajları Türkçedir.** SQLite hataları `sqlite_error_to_turkish`
    üzerinden çevrilir; ham İngilizce metin kullanıcıya gösterilmez.
11. **Yeni bağımlılık gerekçelendirilir.** Lisans BSD 3-Clause ile uyumlu olmalı
    (izin veren lisanslar uygun, copyleft değil). Gereksiz bağımlılık eklenmez.
12. **Türkçe UI metinleri ve `COLLATE NOCASE` benzersizlikleri korunur.**

## 7. Yeni Özellik Ekleme Rehberi

### A) Yeni ekran ekle

1. `src/app.rs` içindeki `View` enum'a varyant ekle; `title()` ve `description()`
   eşlemelerini yaz.
2. `View::ALL` dizisine menü sırasına uygun olarak ekle.
3. `src/app/<yeni>_screen.rs` oluştur: ekran durumu (`*State`) ve komut metotları.
4. `src/ui/<yeni>.rs` oluştur: yalnızca çizim; veri `app` üzerinden okunur.
5. `src/ui/mod.rs` ve `src/ui/layout.rs` içine modül kaydı ve dağıtım ekle.
6. Gerekiyorsa `src/<yeni>.rs` (saf iş modeli) ve `src/db/<yeni>.rs` (SQL) ekle.
7. Test: 900×600 yerleşim testi + davranış/entegrasyon testi.
8. `cargo fmt --check && cargo check --all-targets && cargo clippy --all-targets
   -- -D warnings && cargo test --all-targets`.

### B) Yeni migration ekle (örnek: v8)

```rust
// src/db/migrations.rs — listenin SONUNA eklenir, mevcutlar değişmez
Migration {
    version: 8,
    name: "ornek_yeni_tablo",
    sql: r#"
CREATE TABLE ornek (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    invoice_id INTEGER NOT NULL REFERENCES invoices (id),
    durum      TEXT    NOT NULL CHECK (durum IN ('bekliyor', 'tamam')),
    created_at TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_ornek_invoice ON ornek (invoice_id);
"#,
},
```

- `IF NOT EXISTS` kullanma; yeni migration hatayı gizlememeli. (Yalnızca mevcut
  v1'deki `app_metadata` tablosu bu kalıpla yazılmıştır; o kayıt değiştirilmez.)
- Aynı migration içinde birden çok ifade olabilir; tek transaction'da çalışır.
- Test: v1→v8, v2→v8, …, v7→v8 zincirleri geçmeli; mevcut veriler korunmalı
  (`tests/stress/migration_chain.rs` bu zinciri denetler).
- Şema sürümü uygulamadan yeni ise açılış `SchemaTooNew` hatası verir; sessiz
  bozulma olmaz.

### C) Yeni rapor ekle

1. `src/report.rs`: rapor modeli + filtre + satır tipleri (saf hesaplama).
2. `src/db/reports.rs`: salt okunur SQL sorgusu.
3. `src/ui/reports.rs`: sekme çizimi ve filtre paneli.
4. `src/app/report_screen.rs`: filtre durumu ve yenileme.
5. CSV: `src/app/export_screen.rs` içindeki rapor satırı üretimine ekle.
6. Test: korelasyon + 900×600 yerleşim + CSV başlık/satır sayısı.

### D) e-Fatura (kapsam dışı, ileri düzey)

Bu projede **e-Fatura yoktur**. Eklemek isteyen fork, GİB onaylı bir özel
entegratörle API entegrasyonu yapmak zorundadır. Önerilen tasarım:

- `e_invoice_outbox` tablosu (yeni migration): `invoice_id` FK, `status`,
  `entegrator_ref`, `hata_mesaji`, `retry_sayisi`.
- `src/e_invoice/` modülü: `trait EntegratorAdapter` + her entegratör için `impl`.
- Ayarlar > e-Fatura ekranı (API anahtarı, entegratör seçimi).
- Worker: kuyruktan iş alır, API'ye gönderir, sonucu yazar.

**Sabit entegratör adı koda gömülmez**; trait soyutlaması kullanılır. Bu özellik
için önce tasarım onayı alınmalı, doğrudan kodlamaya başlanmamalıdır.

### E) Yeni kullanıcı rolü / yetki

`src/auth.rs` içindeki `Role` enum'u ve `is_manager()` kararları tek noktadır.
Yeni rol eklenirse `users.role` CHECK kısıtı için migration gerekir; arayüzde
`src/ui/settings.rs` ve yazma metotlarındaki `require_admin` çağrıları gözden
geçirilir.

## 8. Sık Yapılan Hatalar (vibe coding tuzakları)

| Hatalı eğilim | Doğrusu |
|---------------|---------|
| "Hızlıca `f32` kullanayım" | `Money` / `Quantity` kullan; f32/f64 muhasebede yasak |
| "Migration'ı düzenleyeyim" | Yeni migration ekle; v1..v7 değişmez |
| "Faturayı sileyim" | İptal et (`status='iptal'`), stok ve bakiye geri alınır |
| "UI'dan direkt SQL çalıştırayım" | Sorguyu `src/db/` katmanına taşı |
| "Testi sileyim, geçsin" | Testi düzelt veya yeni davranışa uyarla; gerekçe yaz |
| "`current_stock`'u elle güncelleyeyim" | Stok hareketi ekle; aynı transaction'da türet |
| "`paid_amount`'u elle set edeyim" | `payment_allocations` üzerinden hesapla |
| "Yeni tablo ekleyeyim, migration sonra" | Migration zorunlu; aksi hâlde şema bozulur |
| "Türkçe hata mesajını atlayayım" | `sqlite_error_to_turkish` kullan |
| "İşlemi iki ayrı `execute` ile yazayım" | Tek `transaction` kullan |
| "Kullanıcıyı sileyim" | Pasife al (`active=0`); yalnızca `users` silinebilir |
| "Ağ paylaşımı ekleyeyim" | Tek bilgisayar/tek oturum tasarımı korunur |

## 9. Test Stratejisi

| Grup | Komut | Kapsam |
|------|-------|--------|
| Hızlı | `cargo test --all-targets` | 432 test; saniyeler |
| Ağır | `cargo test --release -- --ignored --test-threads=1` | 21 test; onlarca dakika |
| Biçim | `cargo fmt --check` | Zorunlu |
| Derleme | `cargo check --all-targets` | Zorunlu |
| Lint | `cargo clippy --all-targets -- -D warnings` | Uyarı = hata |
| Release | `cargo build --release` | MSI öncesi |

- Ağır testler CI'da çalıştırılmaz; yerel makinede release profiliyle koşulur.
- Ağır paket: bütünlük, migration zinciri, kurtarma, fuzz, soak, ölçek, toplu
  veri, hata enjeksiyonu, gerçekçi senaryolar ve arayüz sağlığı.
- 900×600 yerleşim testleri `egui::Context::run` + `DropOutput` ile yapılır;
  panik/taşma olmadan çizim tamamlanmalıdır.
- Testler geçici dizin (`tempfile`) kullanır; gerçek kullanıcı verisine
  dokunulmaz. Ağır testler `ON_MUHASEBE_DATA_DIR` ile ayrılmış dizinde çalışır.

## 10. Bilinen Sınırlamalar (bu sürümde YOKTUR)

- PDF/XLSX dışa aktarma ve fatura yazdırma.
- e-Fatura / e-Arşiv / e-Defter gönderimi.
- Virman (kasa/banka hesapları arası transfer).
- Çek/senet, kredi kartı ve döviz; tüm tutarlar TL'dir.
- Çoklu şirket (multi-tenant) ve şube.
- Otomatik güncelleme indirme/kurma; güncelleme yeni MSI'ın elle kurulmasıyla
  yapılır.
- Ağ üzerinde paylaşım; aynı anda tek oturum, tek bilgisayar.
- Kod imzalama; SmartScreen "bilinmeyen yayıncı" uyarısı normaldir.
- Bulut yedekleme ve senkronizasyon.

Bu özelliklerden biri istenirse **önce tasarım sorulmalı**; doğrudan kodlamaya
girişilmemelidir.

## 11. Fork ve Ticari Kullanım (BSD 3-Clause)

- Kod serbestçe fork edilebilir, değiştirilebilir, dağıtılabilir ve satılabilir.
- **3. madde:** Orijinal yazarın adı (@furcanomer) veya proje adı, tanıtım ya da
  onay imasında **kullanılamaz**.
- Telif bildirimi ve lisans metni (`LICENSE`) korunmalıdır; ikili dağıtımda
  lisans metni de verilmelidir.
- "Ön Muhasebe" adı ve @furcanomer markası, onay ima edecek şekilde
  kullanılmamalıdır. Ticari bir üründe farklı bir ad seçilmesi önerilir.
- Değiştirilmiş sürümlerde "orijinal projeden türetilmiştir" notu bırakmak iyi
  bir uygulamadır (zorunlu değildir).

## 12. Hızlı Referans

| Ne yapmak istiyorsun? | Hangi dosyaya bak? |
|-----------------------|--------------------|
| Yeni ekran | `src/app.rs` (`View`) + `src/ui/layout.rs` + `src/app/*_screen.rs` + `src/ui/*` |
| Yeni tablo/kolon | `src/db/migrations.rs` (listenin sonuna) |
| Para işlemi | `src/money.rs` (f32 yasak) |
| Miktar/stok | `src/quantity.rs` (mili-birim) |
| Fatura hesabı | `src/invoice.rs` (`calculate_line`) |
| Tahsilat dağıtımı | `src/payment.rs` + `src/db/payments.rs` |
| Kasa bakiyesi | `src/db/payments.rs`, `src/db/income_expense.rs` |
| Korelasyon denetimi | `tests/stress/mod.rs` → `assert_correlations` |
| CSV | `src/csv.rs` + `src/app/export_screen.rs` |
| Yedek/geri yükleme | `src/backup.rs` + `src/restore_journal.rs` + `src/db/transfer.rs` |
| Kullanıcı/şifre | `src/auth.rs` + `src/db/users.rs` |
| Ayarlar | `src/settings.rs` + `src/ui/settings.rs` |
| MSI | `wix/main.wxs` + `build-msi.bat` |
| Kullanıcı belgesi | `docs/` |

**Sık kullanılan komutlar:**

```text
cargo run                                  # geliştirme
cargo test --all-targets                   # hızlı testler
cargo clippy --all-targets -- -D warnings  # lint
build-msi.bat                              # release + MSI
```

## 13. Yapay Zekâ İçin Son Notlar

- **Kod yazmadan önce mevcut kodu oku.** Özellikle ilgili `db` ve `app`
  dosyalarını; tahminle dosya/alan adı uydurma.
- **Değişiklikten önce plan öner; kullanıcı onaylasın.** Küçük ve geri
  alınabilir adımlar hâlinde ilerle.
- **Her değişiklikten sonra `cargo test --all-targets` çalıştır.** Kırılan test
  varsa önce testi incele; testi silme.
- **Büyük değişikliği parçalara ayır.** Her parça derlenmeli ve testleri
  geçmeli; yarım kalan refactor bırakma.
- **Türkçe hata mesajları ve UI metinleri korunur.** İngilizce metin ekleme.
- **Şema değişikliğinde migration, veri değişikliğinde transaction.**
- **Kullanıcı verisine dokunma.** Testler `tempfile` veya ayrı
  `ON_MUHASEBE_DATA_DIR` kullanır.
- **Belirsizlikte sor.** Özellikle "silme mi, iptal mi?", "yeni migration mı,
  mevcut alan mı?" sorularında varsayım yapma.

## 14. İletişim ve Kaynaklar

| Kaynak | Adres |
|--------|-------|
| GitHub | <https://github.com/furcanomer/OnMuhasebe> |
| Kullanıcı belgeleri | `docs/KURULUM.md`, `docs/KULLANIM_KILAVUZU.md`, `docs/SSS.md`, `docs/SORUN_GIDERME.md` |
| Ekran görüntüleri | `docs/EKRAN_GORUNTULERI/README.md` |
| Lisans | BSD 3-Clause (`LICENSE`) |
| Telif | Copyright (c) 2026, @furcanomer |
| Sürüm | 1.0.0, şema v7 |
| MSI | `dist\OnMuhasebe-1.0.0.msi` (`build-msi.bat`) |

Son güncelleme: 2026-10-27
