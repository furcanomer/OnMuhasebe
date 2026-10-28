# Geliştirici Rehberi — Ön Muhasebe

> **Bu rehber insan geliştirici içindir.** "Ne yapılacak" listesi için
> `AI_DEVRALMA.md`, kopyala-yapıştır promptlar için `AI_PROMPTS.md` dosyasına
> bakın. Buradaki asıl soru **"neden böyle"**: kararların gerekçesi, bedeli ve
> tuzakları. Yapay zekâ ile çalışıyorsanız önce `AI_DEVRALMA.md`'yi okuyun;
> oradaki kurallar burada gerekçeleriyle açıklanır, tekrar edilmez.

## 1. Bu Belge Kimin İçin

Bu rehber şu kişiler için yazıldı:

- Projeyi devralıp bakımını yapacak geliştirici,
- Yeni özellik ekleyecek ekip üyesi,
- Kodu inceleyip kendi ürününe temel yapacak fork sahibi,
- Hatayı ayıklaması gereken kişi (gece 02:00'de, canlı veriyle).

**Ön koşullar**

| Konu | Beklenen düzey |
|------|----------------|
| Rust | 1.99 veya üzeri; `edition 2024` sözdizimi, `Result`/`?`, trait'ler |
| SQLite | Temel SQL, `PRAGMA`, transaction, foreign key kavramları |
| egui/eframe | Opsiyonel; `ui.label`, `ScrollArea`, `Window` düzeyinde bilgi yeter |
| Muhasebe | Temel ön muhasebe: cari, fatura, tahsilat, kasa farkı |

Kurulum: Rust 1.99+ (rustup önerilir), Windows'ta MSVC toolchain ve Windows
SDK. MSI üretimi için ayrıca WiX Toolset 3.14 ve `cargo-wix` gerekir.

```text
cargo build            # hızlı geliştirme derlemesi
cargo test --all-targets
cargo run              # pencereyi açar
```

İlk derleme uzundur (SQLite kaynaktan derlenir); sonrası artımlıdır.

## 2. Neden Bu Mimari

Aşağıdaki tablo, projedeki büyük kararların özetidir. Alt bölümler her kararın
bedelini ve bugün hâlâ geçerli olup olmadığını anlatır.

| Karar | Alternatifler | Sonuç |
|-------|---------------|-------|
| Rust + egui/eframe | Tauri, Electron, .NET WinForms, Qt | Tek exe, harici runtime yok, bellek güvenli |
| SQLite (bundled) | PostgreSQL, MySQL, dosya tabanlı özel format | Tek dosya, sunucu yok, format 20+ yıldır stabil |
| Money (i64 kuruş) | f64, ondalık kütüphanesi, `rust_decimal` | Yuvarlama hatası yok, SQLite'a doğrudan yazılır |
| Quantity (i64 mili-birim) | f64, tam sayı + birim dönüşümü | 12,5 kg gibi ondalıklar tam sayı ile temsil edilir |
| Tek yönlü katman | Katmansız "her şey app.rs'te" | Test edilebilirlik, SQL'in UI'a sızmaması |
| İptal / pasife alma | Fiziksel `DELETE` | Denetim izi, geri alınabilirlik, yasal saklama |
| Migration: eskiye dokunma | Şemayı "son hâliyle" yeniden yazmak | Kurulu veritabanları bozulmaz |
| Tek transaction | Adım adım `execute` çağrıları | Yarım kalan fatura/stok/bakiye olmaz |
| Sanal liste | Tüm satırları çizmek | 1M kayıtta arayüz donmaz |
| Argon2id | SHA-256, düz metin, bcrypt | Yavaş hash; GPU kırma pahalı |

### 2.1. Neden Rust + egui, Tauri değil?

Tauri, arayüzü sistemin webview'ine devreder. Bu, kurulum boyutunu küçültür ama
üç bedel getirir: (1) Windows sürümleriyle birlikte gelen WebView2 çalışma
zamanına bağımlılık, (2) arayüz ile Rust arasında JSON/IPC katmanı ve onun
sürüm kırılganlığı, (3) hata ayıklarken iki ayrı dünya (JS + Rust).

egui ise doğrudan çizim yapar: tek `OnMuhasebe.exe`, harici DLL yok, IPC yok.
Bedeli: HTML/CSS ekosisteminin hazır bileşenleri yok; tablo, form ve yerleşim
elle yazılır. Bu proje için bu bedel kabul edilebilir, çünkü ekran sayısı sınırlı
(9 görünüm) ve arayüz yoğun veri tablosu ağırlıklı.

Sonuç: 20 yıl sonra da çalışacak bir kurulum hedefi korunur. Webview sürümü
değişse bile uygulama etkilenmez.

### 2.2. Neden SQLite, sunucu veritabanı değil?

Hedef kullanıcı tek bilgisayarda, tek oturumda çalışan küçük işletme. Sunucu
veritabanı; kurulum, yedekleme, port ve yetki yönetimi demektir — destek yükü
kullanıcıya devredilemez.

SQLite `bundled` özelliğiyle kaynaktan derlenir: kullanıcı makinesinde ayrı
`sqlite3.dll` sürüm çatışması olmaz. WAL modu tek yazıcı + çok okur sağlar;
kasa bakiyesi gibi sık okunan veriler yazma sırasında kilitlenmez.

Bedeli: ağ üzerinden paylaşım yoktur. Çok kullanıcı istenirse PostgreSQL'e geçiş
büyük bir refactor'dır (bkz. `ROADMAP.md`); bu yüzden bilinçli olarak kapsam
dışında tutulur.

### 2.3. Neden `Money` (i64 kuruş) ve `Quantity` (i64 mili-birim)?

Kayan noktalı sayı ikili tabanda çoğu ondalık kesri tam temsil edemez:

```text
0.1 + 0.2 = 0.30000000000000004
```

Bu, tek bir faturada önemsiz görünür; 10.000 satırlık bir dönemde kuruş kuruş
birikir ve kasa/cari mutabakatı tutmaz. Muhasebede "yaklaşık" yoktur: iki yol
birbirini kuruşuna kadar tutmalıdır.

Çözüm: her tutar **kuruş** cinsinden `i64`. 12,34 TL bellekte `1234`'tür.
Toplama/çıkarma tam sayı işlemidir; çarpma ve yüzde işlemlerinde ara sonuç
`i128`'e genişletilir ve sonuç kuruşa yuvarlanır. Miktar için aynı mantık
**mili-birim** (1/1000) ile uygulanır: 12,5 kg = `12500`.

Bedeli: her giriş/çıktı noktasında parse/format gerekir; yüzde ve KDV
hesaplarında yuvarlama kararı verilmelidir (bkz. 5. bölüm).

### 2.4. Neden tek yönlü katman?

`main → app → ui / db → error` yönü korunur. Ters yön (UI'dan `db`'ye doğrudan
erişim) yasaktır.

Neden: UI kodu SQL içerdiğinde (1) arayüz testleri veritabanı gerektirir,
(2) aynı sorgu iki ekranda farklı yazılır ve biri bozulur, (3) şema değişince
derleyici sizi uyarmaz — hata ancak çalışma anında çıkar. Katmanlı yapıda şema
değişikliği `db` katmanında tek noktada yapılır; üst katmanlar aynı imzayı
kullanmaya devam eder.

Bedeli: basit bir alan eklemek için 3–4 dosyaya dokunulur (model → db → app →
ui). Bu tekrar, kasıtlıdır: her katmanın kendi testi olur.

### 2.5. Neden kayıt silinmez?

Fatura, tahsilat/ödeme ve gelir/gider kaydı **iptal** edilir (`status='iptal'`);
cari, ürün, kasa hesabı ve kategori **pasife** alınır (`active=0`).

Gerekçe: muhasebe bir denetim izidir. "Bu fatura neden iptal edildi, ne zaman,
kim tarafından?" sorusunun cevabı veritabanında kalmalıdır. Ayrıca yanlışlıkla
silinen bir kayıt, ona bağlı stok hareketini ve kasa bakiyesini de tutarsız
bırakır. İptal, etkiyi ters kayıtla geri alır — muhasebe geleneğine de uygundur.

Tek istisna `users` tablosudur: yönetici pasif bir kullanıcıyı silebilir; son
yönetici silinemez (`LastAdminProtected`), kişi kendini silemez
(`SelfOperationProtected`).

### 2.6. Neden migration'lar değiştirilmez?

`MIGRATIONS` dizisindeki bir kayıt, dağıtılmış bir sözleşmedir: binlerce
kullanıcının veritabanında o sürüm çoktan uygulanmıştır. Eski migration'ı
düzenlerseniz yeni kurulum ile mevcut kurulum farklı şemalara sahip olur.

Somut tehlike: v5'e sonradan `ALTER TABLE invoices ADD COLUMN x` eklerseniz,
v5'i zaten uygulamış kullanıcıda bu kolon **yok** olur; uygulama "no such column"
ile açılır. Aynı migration'ı tekrar çalıştırmak ise "duplicate column" hatası
verir. Bu yüzden kural nettir: **eski migration'a dokunma, sona yeni ekle.**

Sürüm atlama da yakalanır: `schema_migrations` içinde boşluk varsa uygulama
`MigrationChainBroken { missing }` döner ve veriye dokunmadan durur. Veritabanı
uygulamadan yeni ise `SchemaTooNew { found, supported }` döner.

### 2.7. Neden tek transaction?

Bir fatura kesildiğinde en az dört tablo değişir: `invoices`, `invoice_lines`,
`product_stock_movements`, `products.current_stock` (ve ödeme varsa
`payment_allocations`, `payments`, `cash_accounts`). Bu adımların arasında güç
kesilirse stok düşmüş ama fatura yazılmamış olabilir; korelasyonlar bozulur.

Transaction, "ya hepsi ya hiçbiri" garantisidir. Bu projede her yazma işlemi tek
transaction içinde yapılır; hata `?` ile yukarı çıkar ve `Drop` sırasında
otomatik geri alınır.

### 2.8. Neden sanal liste?

egui her karede yalnızca görünen alanı çizer, ama `for` döngüsüyle 1.000.000
satır oluşturmak yine de satır başına iş yapar. `ScrollArea::show_rows` yalnızca
görünür aralığı ister; `ui/virtual_list.rs::visible_range` bu aralığı hesaplar.
Sonuç: kayıt sayısı arttıkça çizim maliyeti sabit kalır.

## 3. Katmanlar ve Sınırlar (Detaylı)

```text
main.rs
  └── app.rs  (OnMuhasebeApp: tüm durum, View, komutlar)
        ├── app/*_screen.rs   (ekran durumu + komut metotları)
        │     ├── db/*.rs     (SQL, transaction, korelasyon bakımı)
        │     └── <model>.rs  (customer, product, invoice, payment, ...)
        └── ui/*.rs           (çizim; SQL YOK, iş kuralı YOK)
              └── widgets.rs, theme.rs, format.rs, virtual_list.rs
```

| Katman | Ne yapar | Ne yapmaz | Kim çağırır |
|--------|----------|-----------|-------------|
| `main.rs` | Yapılandırma, log, pencere | İş mantığı, SQL | İşletim sistemi |
| `app.rs` | Durum sahibi, komut yönlendirme, yetki | SQL metni, çizim ayrıntısı | `ui`, `app/*_screen` |
| `app/*_screen.rs` | Form durumu, doğrulama çağrısı, db çağrısı | Piksel/yerleşim, ham SQL | `ui/*` olayları |
| `db/*.rs` | SQL, transaction, korelasyon | Türkçe metin biçimleme, GUI | `app/*_screen` |
| model (`customer.rs` vb.) | Doğrulama, hesap, tip güvenliği | SQL, GUI, dosya | `app`, `db`, `ui` (okuma) |
| `ui/*.rs` | Çizim, yerleşim, girdi | SQL, iş kuralı, kalıcı durum | `ui/layout.rs` |
| `error.rs` | Tek hata tipi, Türkçe çeviri | İş kuralı | Herkes |

### 3.1. Sınır ihlali örnekleri

Yanlış (UI'da SQL):

```rust
// src/ui/customers.rs içinde — YAPMAYIN
let mut statement = connection.prepare("SELECT * FROM customers")?;
```

Doğru (veri `app` üzerinden gelir):

```rust
// src/ui/customers.rs
for customer in &app.customer.list {
    ui.label(&customer.name);
}
```

Yanlış (iş modelinde SQL):

```rust
// src/invoice.rs içinde — YAPMAYIN
connection.execute("UPDATE invoices SET paid_amount = ?1", [total])?;
```

Doğru (model saf hesap yapar, `db` yazar):

```rust
// src/invoice.rs — saf fonksiyon
let amounts = calculate_line(quantity, unit_price, discount_rate, vat_rate)?;
```

### 3.2. Bir yazma işleminin yolculuğu (fatura kesme)

1. `ui/invoices.rs` düğmeye basıldığını görür, `app.invoice.save()` çağırır.
2. `app/invoice_screen.rs` taslağı doğrular (`InvoiceDraft::validate`), alan
   hatalarını forma yazar.
3. `db/invoices.rs::create` tek transaction açar: fatura + satırlar + stok
   hareketleri + `current_stock` + (varsa) `paid_amount`.
4. Hata olursa transaction geri alınır, `AppError` yukarı çıkar, arayüz Türkçe
   mesajı gösterir.
5. Başarıda `app` listeyi yeniler; `ui` yalnızca yeni durumu çizer.

Bu yol boyunca UI hiçbir SQL görmez, model hiçbir bağlantı görmez.

## 4. Veri Modelinin Ardındaki Mantık

Şema v7, 14 tablo. Aşağıdaki tablo "bu tablo neden var, kısıt neyi korur"
sorusunu yanıtlar; kolon listesi için `src/db/migrations.rs` ve
`AI_DEVRALMA.md` 5. bölüme bakın.

| Tablo | Ne için var | Kısıtın gerekçesi |
|-------|-------------|-------------------|
| `schema_migrations` | Hangi sürümün uygulandığını tutar | Boşluk tespiti ve `SchemaTooNew` kontrolü |
| `app_metadata` | Uygulama düzeyinde meta bilgi | Anahtar-değer tablosu; iş verisi tutmaz |
| `customers` | Cari kimliği | `code` UNIQUE `NOCASE`: "ali" ve "Ali" aynı cari |
| `products` | Ürün kartı + güncel stok | `barcode` kısmi UNIQUE: boş barkod çoğul olabilir |
| `product_stock_movements` | Stok değişiminin tek kaynağı | `quantity_delta <> 0`: anlamsız sıfır hareket yok |
| `invoices` | Fatura başlığı ve toplamları | `invoice_number` UNIQUE: aynı numara iki kez kesilemez |
| `invoice_lines` | Fatura satırları + ürün kopyası | `UNIQUE(invoice_id, line_no)`: satır sırası tekildir |
| `cash_accounts` | Kasa/banka hesabı | `opening_balance >= 0`: açılış negatif olamaz |
| `payments` | Tahsilat/ödeme başlığı | `amount > 0`: sıfır tutarlı ödeme yok |
| `payment_allocations` | Ödemenin faturaya dağıtımı | `UNIQUE(payment_id, invoice_id)`: çift dağıtım yok |
| `expense_categories` | Gelir/gider kategorisi | `UNIQUE(name, category_type)`: aynı ad iki türde olabilir |
| `income_expense_entries` | Kira, maaş, elektrik hareketi | `amount > 0`; cariye bağlı değildir |
| `users` | Giriş ve rol | `username` UNIQUE `NOCASE`; şifre Argon2id PHC |
| `app_settings` | Kullanıcı tercihleri | Anahtar-değer; migration gerektirmeden ayar |

### 4.1. Cari bakiyesi neden tabloda tutulmaz?

`Customer::balance()` her okumada şu formülü uygular:

```text
bakiye = Σ kesin satış faturaları
       − Σ kesin alış faturaları
       − Σ kesin tahsilatlar
       + Σ kesin ödemeler
```

Bakiye ayrı bir kolonda tutulsaydı, her fatura/tahsilat/iptal işleminde
güncellenmesi gerekirdi. Bir yerde unutulduğunda bakiye sessizce yanlış olur ve
hiçbir test bunu yakalamaz. Türetilmiş değer her zaman doğrudur; bedeli, büyük
cari listelerinde ek toplama sorgusudur (indeksler bunu karşılar).

### 4.2. `products.current_stock` neden denormalize?

Stok, ürün listesinde her satırda görünür. Her satır için
`SUM(quantity_delta)` çalıştırmak 100.000 üründe pahalıdır. Bu yüzden
`current_stock` kolonda tutulur — ama **tek kaynak hareketlerdir**:
`current_stock == SUM(quantity_delta)` eşitliği `assert_correlations` ile
denetlenir. Denormalizasyon burada bilinçli bir hız/doğruluk takasıdır ve
test ile çitlenmiştir.

### 4.3. Neden satırda ürün kopyası var?

`invoice_lines` içinde `product_code`, `product_name`, `unit` alanları
**snapshot**'tır. Ürünün adı sonradan değişse veya ürün pasife alınsa bile geçmiş
fatura, kesildiği gündeki bilgiyi gösterir. Aksi hâlde 2024 faturası, 2026'da
yapılan bir isim değişikliğiyle farklı görünürdü — denetimde kabul edilemez.

Aynı nedenle `unit_price`, `discount_rate`, `vat_rate` de satırda saklanır;
ürün kartındaki güncel fiyat geçmişi değiştirmez.

### 4.4. Ödeme, dağıtım ve avans

Bir tahsilat birden çok faturaya bölünebilir (`payment_allocations`). Dağıtım
toplamı ödeme tutarından küçükse kalan kısım **avanstır**; ayrı tablo yoktur,
farktan anlaşılır. Kurallar:

- Tahsilat yalnızca **satış**, ödeme yalnızca **alış** faturasına dağıtılır
  (`AllocationTypeMismatch`).
- Fatura, ödemenin carisine ait olmalıdır (`AllocationCustomerMismatch`).
- İptal faturaya dağıtım yapılamaz (`AllocationToCancelledInvoice`).
- Kalan tutardan fazlası dağıtılamaz (`AllocationExceedsRemaining`).

`invoices.paid_amount`, kesin ödemelerin dağıtım toplamıdır; elle yazılmaz.
İptalde sıfırlanır ve korelasyon testi bunu denetler.

### 4.5. Gelir/gider neden cariye bağlı değil?

Kira, elektrik, maaş gibi giderlerin bir müşteriyle ilişkisi yoktur. Bu kayıtlar
yalnızca kasa/banka bakiyesini etkiler. Cariye bağlanmaya çalışılsaydı, her
elektrik faturası için sahte bir "tedarikçi" kartı açmak gerekirdi.

## 5. Para ve Miktar Tipleri (Derinlemesine)

### 5.1. `Money` API'si

| Metot | Davranış |
|-------|----------|
| `from_kurus(i64)` | Ham kuruş; taşma yok |
| `from_lira(i64)` | `lira × 100`, taşmada `AmountOverflow` |
| `kurus()` | i64 kuruş |
| `parse(&str)` | "1.234,56", "1234.56", "1234" kabul eder |
| `format_turkish()` | "1.234,56" |
| `format_with_symbol()` | "1.234,56 ₺" |
| `checked_add/sub` | Taşma kontrollü |
| `checked_mul(i64)` | Tam sayı çarpanı |
| `checked_mul_quantity(Quantity)` | `fiyat × miktar`, 1000'e bölünür |
| `checked_mul_percent(i64)` | Yüzde 0–100, kuruşa yuvarlanır |
| `percentage_of(base)` | Bir tutarın yüzdesi (`Option<i64>`) |
| `checked_sum(iter)` | Dizi toplamı, taşma kontrollü |
| `checked_neg()` | İşaret ters çevirme (borç/alacak) |

Örnek:

```rust
let price = Money::parse("12,34")?;          // 1234 kuruş
let qty = Quantity::parse("2,5")?;           // 2500 mili-birim
let gross = price.checked_mul_quantity(qty)?; // 3085 kuruş = 30,85 TL
```

### 5.2. `Quantity` API'si

| Metot | Davranış |
|-------|----------|
| `from_milli(i64)` | Ham mili-birim |
| `from_units(i64)` | `birim × 1000`, taşmada `QuantityOverflow` |
| `milli()` | i64 mili-birim |
| `parse(&str)` | En fazla 3 ondalık (`FRACTION_MAX_LEN`) |
| `format_turkish()` | "12,5" (gereksiz sıfırlar atılır) |
| `checked_add/sub` | Taşma kontrollü |

`MILLI_PER_UNIT = 1000`. Neden 1000? Üç ondalık, stok dünyasında yeterlidir
(gram, mililitre). Daha fazlası hassasiyet değil, yanlış güven verir.

### 5.3. Yuvarlama kuralı

Yuvarlama tek bir özel fonksiyonda toplanır (`money.rs` içindeki
`divide_round_half_up`): yarım kuruş **sıfırdan uzağa** yuvarlanır.

```rust
fn divide_round_half_up(numerator: i128, denominator: i128) -> i128 {
    debug_assert!(denominator > 0);
    if numerator >= 0 {
        (numerator + denominator / 2) / denominator
    } else {
        -((-numerator + denominator / 2) / denominator)
    }
}
```

Negatif tutarlar (alış faturası iptali, ters kayıt) simetrik davranır: −0,005
kuruş → −1 kuruş. Simetri olmasaydı, aynı işlemin tersi aynı tutarı vermezdi.

### 5.4. Fatura satırı formülü

```text
gross    = unit_price × quantity          (checked_mul_quantity)
discount = gross × discount_rate / 100    (checked_mul_percent)
net      = gross − discount
vat      = net × vat_rate / 100           (checked_mul_percent)
total    = net + vat
```

Fatura toplamları satırlardan türetilir: `subtotal` (brüt toplam),
`discount_total`, `net_total`, `vat_total`, `grand_total`. Hiçbir toplam elle
yazılmaz; satır eklenince/silinince yeniden hesaplanır.

### 5.5. Yasaklar

| Yasak | Neden |
|-------|-------|
| `(a as f64 * b as f64) as i64` | Kuruş kaybı; 0.1+0.2 problemi |
| `money.kurus() as f64 / 100.0` ile hesap | Aynı sorun, gizli hâli |
| `unwrap()` yerine `expect()` ile taşmayı gizlemek | Panik = veri kaybı riski |
| `i64` toplamada taşma kontrolü atlamak | Sessiz sarma (wrap), yanlış bakiye |
| UI ölçüsü dışında f32/f64 | egui ölçüleri f32'dir; muhasebe hesabı değildir |

Taşma durumunda `AppError::AmountOverflow` / `QuantityOverflow` döner; arayüz
"Tutar çok büyük" benzeri Türkçe mesaj gösterir.

## 6. Test Felsefesi

### 6.1. Piramit

| Katman | Yer | Sayı | Süre |
|--------|-----|------|------|
| Birim | `src/*.rs` içinde `#[cfg(test)] mod tests` | 299 | saniyeler |
| Entegrasyon | `tests/*.rs` (geçici DB) | 110 | ~11 sn |
| Ağır (stress) | `tests/stress/*.rs`, `#[ignore]` | 23 geçen + 21 atlanan | dakikalar |

Toplam hızlı koşu: **432 test**. Ağır paket release profilinde, tek iş
parçacığıyla çalıştırılır.

### 6.2. Korelasyon testi neden bu kadar önemli?

Tek bir mutasyon (fatura kesme, iptal, ödeme) dört ayrı tabloyu etkiler. Kodu
okuyarak "hepsini güncelledim" demek yeterli değildir; kanıt gerekir.
`tests/stress/mod.rs::assert_correlations` şunları denetler:

```text
PRAGMA integrity_check            -> "ok"
PRAGMA foreign_key_check          -> 0 ihlal
journal_mode                      -> wal
products.current_stock            == SUM(quantity_delta)
invoices.paid_amount              == SUM(kesin dağıtımlar)
cash_accounts.current_balance     == açılış + tahsilat + gelir − ödeme − gider
iptal faturada paid_amount        == 0
```

Yeni bir yazma yolu eklediğinizde bu fonksiyonu çağıran bir test yazın. Test
geçmiyorsa ya kod ya da test yanlıştır; ikisini de körü körüne "düzeltmeyin".

### 6.3. Yerleşim testi nasıl yapılır?

Gerçek kalıp `tests/stress/ui_health.rs` içindedir: 900×600 sanal ekran,
`Context::run_ui`, `ui::layout::draw`.

```rust
let input = egui::RawInput {
    screen_rect: Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(900.0, 600.0),
    )),
    ..Default::default()
};
let mut output = context.run_ui(input, |ui| {
    on_muhasebe::ui::layout::draw(ui, app);
});
output.textures_delta.clear();
```

Bu test panik, taşma veya sonsuz döngü arar; piksel doğruluğu aramaz. DPI
ölçekleri (1.0, 1.25, 1.5, 2.0) ayrıca denenir — düşük çözünürlükte taşan
düğmeler burada yakalanır.

### 6.4. Neden bazı testler `#[ignore]`?

`scale_up` 10.000.000 cari ve 100.000.000 stok hareketi kurar; `soak` saatler
süren tekrarlar yapar; `real_world` süreci öldürüp WAL kurtarmayı dener. Bunlar
her `cargo test` koşusunda çalışırsa geliştirme döngüsü kilitlenir. Bu yüzden
ayrı işaretlenir ve release öncesi elle çalıştırılır:

```text
cargo test --release -- --ignored --test-threads=1
```

### 6.5. Determinizm

Rastgele veri üreten testler sabit tohum kullanır. Tohum değişirse hata tekrar
üretilemez; "bende geçiyor" durumu doğar. Aynı şekilde tarih/saat testlerde
enjekte edilir (`now` parametresi), `Local::now()` doğrudan çağrılmaz.

### 6.6. Test silme yasağı

Kırılan test, davranışın değiştiğinin kanıtıdır. İki meşru seçenek vardır:
(1) kod hatalıysa kodu düzelt, (2) davranış bilinçli değiştiyse testi yeni
davranışa göre güncelle ve **gerekçeyi commit mesajına yaz**. Üçüncü seçenek —
testi silmek veya `assert` gevşetmek — yasaktır.

## 7. Yeni Özellik Ekleme (İnsan İçin Ayrıntılı)

### A) Yeni ekran

Başlangıç noktası `src/app.rs`. Sıra:

1. `View` enum'a varyant ekle; `title()`, `description()`, `is_ready()`
   eşlemelerini yaz. `is_ready()` yalnızca menüde "henüz geliştirilmedi"
   ipucunu denetler; bugün dokuz görünümün hepsi hazırdır.
2. `View::ALL` dizisine menü sırasına uygun ekle — sol menü bu diziden üretilir,
   ayrıca menü girişi yazmayın.
3. `src/app/<yeni>_screen.rs`: durum struct'ı (`*State`), `Default`, komut
   metotları (`save`, `cancel`, `refresh`).
4. `src/ui/<yeni>.rs`: yalnızca çizim. Ortak bileşenler `ui/widgets.rs`
   (kart, başlık, arama kutusu), ölçüler `ui/theme.rs`.
5. `src/ui/mod.rs` + `src/ui/layout.rs`: modül kaydı ve `View` dağıtımı.
6. Gerekiyorsa `src/<yeni>.rs` (saf model) ve `src/db/<yeni>.rs` (SQL).
7. Test: 900×600 yerleşim + davranış testi.

Tuzak: ekran durumu `app.rs` içinde tutulur, `ui` içinde `static mut` veya global
yoktur. Her karede yeniden oluşturulan durum, kullanıcının yazdığını siler.

### B) Yeni migration

`src/db/migrations.rs` sonuna ekleyin; `version` bir artırın. Basit kolon
ekleme:

```sql
ALTER TABLE invoices ADD COLUMN due_date TEXT;
```

SQLite'ta `ADD COLUMN` ve `DROP COLUMN` (3.35+) desteklenir; ancak kısıt
ekleme/çıkarma, kolon tipini değiştirme ve FK tanımını düzenleme desteklenmez.
Bu durumlarda tablo yeniden oluşturulur. Projede bu kalıp v4'te kullanıldı
(stok hareketlerine `invoice_id` eklemek için):

```sql
CREATE TABLE product_stock_movements_new ( ... invoice_id INTEGER REFERENCES invoices (id) ... );
INSERT INTO product_stock_movements_new (id, product_id, ..., invoice_id)
SELECT id, product_id, ..., NULL FROM product_stock_movements;
DROP TABLE product_stock_movements;
ALTER TABLE product_stock_movements_new RENAME TO product_stock_movements;
CREATE INDEX idx_stock_movements_product ON product_stock_movements (product_id, movement_date);
CREATE INDEX idx_stock_movements_date ON product_stock_movements (movement_date);
```

Dikkat: `DROP TABLE` indeksleri de siler; indeksleri yeniden kurun. Yeni tablo
adı (`_new`) sonradan `RENAME` ile düzeltilir; `PRAGMA foreign_keys` açıkken bu
kalıbın sırası önemlidir.

Test: her `v(N-1) → vN` geçişi ayrı ayrı denenir; mevcut veriler korunur
(`current_stock` toplamı, `paid_amount`, kasa bakiyesi). `migration_chain`
testleri bu zinciri denetler.

### C) Yeni rapor

1. `src/report.rs`: `ReportKind` varyantı, filtre struct'ı, satır tipleri (saf).
2. `src/db/reports.rs`: salt okunur sorgu. Rapor **hiçbir tabloya yazmaz**.
3. `src/ui/reports.rs`: sekme ve filtre paneli.
4. `src/app/report_screen.rs`: filtre durumu, yenileme, hata gösterimi.
5. CSV: `src/app/export_screen.rs` içindeki rapor satırı üretimine ekle.
6. Test: rapor toplamı = DB toplamı (korelasyon), 900×600, CSV başlık/satır.

### D) Yeni para/miktar işlemi

- Her zaman `checked_*` metotlarını kullanın; `+`, `-`, `*` doğrudan `i64` ile
  yazmayın.
- Yüzde hesabında `checked_mul_percent` kullanın; kendi yuvarlama formülünüzü
  yazmayın.
- Sonucu kullanıcıya gösterirken `format_turkish` / `format_with_symbol`.
- Girdi okurken `Money::parse` / `Quantity::parse`; hata mesajını alan bazında
  gösterin.

### E) Yetki kontrolü

Yazma metotları `Database::require_admin(operation)` veya
`require_self_or_admin(user_id, operation)` çağırır. Operatör yalnızca kendi
şifresini değiştirebilir ve Güvenlik sekmesini görür. Yeni bir yazma yolu
eklerken bu çağrıyı atlarsanız, yetki kontrolü sessizce kaybolur — testte
operatör rolüyle denemek tek savunmadır.

## 8. Sık Karşılaşılan Tuzaklar (İnsan)

| Tuzak | Neden cazip | Gerçek bedel | Doğrusu |
|-------|-------------|--------------|---------|
| "Migration'ı düzelteyim" | Tek dosyada çözüm | Kurulu DB'ler bozulur | Yeni migration ekle |
| "Bakiyeyi kolonda tutayım" | Hızlı okuma | Korelasyon sessizce bozulur | Türet; gerekirse indeksle |
| "Faturayı sileyim" | Temiz görünür | Denetim izi ve stok tutarsız | İptal + ters kayıt |
| "UI'dan SQL çalıştırayım" | Kısa yol | Test edilemez, şema sızıntısı | `db` katmanına taşı |
| "`unwrap()` koyayım" | Hızlı | Panik = çökme, veri kaybı | `?` ile hata döndür |
| "Testi sileyim" | Yeşil koşu | Regresyon görünmez olur | Testi düzelt |
| "egui iç API'si" | Esnek çözüm | Sürüm yükseltmesinde kırılır | Public API |
| "PRAGMA'ları atlayayım" | Daha hızlı | FK/WAL yok, bozulma riski | `db/mod.rs` açılışında |
| "`synchronous=FULL` en güvenli" | Sezgisel | Her yazmada disk senkronu | `NORMAL` + WAL |
| "`current_stock`'u elle set edeyim" | Tek satır | Hareketlerle çelişir | Hareket ekle |
| "İki ayrı `execute` yeter" | Basit görünür | Yarım kalan işlem | Tek transaction |
| "Türkçe mesajı sonra eklerim" | Zaman | Ham SQLite hatası kullanıcıya | `sqlite_error_to_turkish` |

### 8.1. "Bakiye kolonda" tuzağının ayrıntısı

Cari bakiyesi kolonda tutulsaydı şu yolların hepsinde güncellenmesi gerekirdi:
fatura kesme, fatura iptali, tahsilat, tahsilat iptali, ödeme, ödeme iptali,
fatura düzenleme. Sekiz yolun birinde unutulan tek satır, aylar sonra "bakiye
tutmuyor" şikâyeti olarak döner. Türetilmiş bakiye bu sınıfı tamamen ortadan
kaldırır.

### 8.2. "PRAGMA'ları atlayayım" tuzağının ayrıntısı

Her bağlantı açılışında şu dört satır çalışır (`src/db/mod.rs`):

```sql
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;
PRAGMA synchronous = NORMAL;
PRAGMA journal_mode = WAL;
```

- `foreign_keys = ON` olmadan FK kısıtları **sessizce yok sayılır** (SQLite
  varsayılanı kapalıdır).
- `busy_timeout` olmadan eşzamanlı yazmada anında "database is locked" alınır.
- `synchronous = NORMAL` + WAL, güç kesintisinde veritabanını bozmaz; `FULL`
  gereksiz yavaştır, `OFF` kabul edilemez.

## 9. Test Yazma Rehberi (İnsan)

### 9.1. Birim testi

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yuzde_elli_iskonto_brut_tutari_yarilar() {
        let gross = Money::from_kurus(10_000);
        let discount = gross.checked_mul_percent(50).unwrap();
        assert_eq!(discount.kurus(), 5_000);
    }
}
```

Kurallar: test adı Türkçe ve davranışı anlatır; `unwrap()` testte serbesttir
(panik = başarısızlık); her test tek bir davranışı doğrular.

### 9.2. Entegrasyon testi

```rust
let directory = tempfile::tempdir().unwrap();
let database = Database::open(directory.path().join("test.db")).unwrap();
```

Geçici dizin kullanın; `%LOCALAPPDATA%` altındaki gerçek veriye asla
dokunmayın. Test bitince dizin otomatik silinir.

### 9.3. Ağır test

```rust
#[test]
#[ignore = "ağır test: release ve --ignored ile çalışır"]
fn milyon_cari_ile_arama_siniri_asilmaz() {
    // ...
}
```

`#[ignore]` gerekçesi metin olarak yazılır; böylece `cargo test -- --ignored`
çıktısında neyin neden atlandığı görünür.

### 9.4. Test isimleri ve kapsam

- `..._basarili` / `..._reddedilir` / `..._korunur` gibi son ekler davranışı
  özetler.
- Sınır değerleri ayrı test edilir: sıfır, negatif, `i64::MAX`, boş metin.
- Bir hata bulduğunuzda önce **kırılan testi** yazın, sonra düzeltin.

## 10. Kod Stili

| Konu | Kural |
|------|-------|
| Biçim | `cargo fmt` (varsayılan rustfmt) |
| Lint | `cargo clippy --all-targets -- -D warnings` |
| Görünürlük | Varsayılan özel; modül içi `pub(crate)` tercih |
| Hata | `thiserror` + `Result<T, AppError>`; `?` operatörü |
| Metin | UI ve hata mesajları Türkçe |
| Sayı biçimi | `format_turkish` / `format_with_symbol` |
| Tarih | `YYYY-MM-DD` (görüntü) / `YYYY-MM-DD HH:MM:SS` (zaman damgası) |
| Yorum | "Neden" yazın; "ne" koddan zaten okunur |
| Dosya adı | `snake_case`; ekranlar `*_screen.rs`, çizim `ui/*.rs` |

Türkçe karakter kuralı: kaynak dosyalar UTF-8'dir; konsolda bozuk görünse de
dosya bozuk değildir. Bir dosyada U+FFFD (değiştirme karakteri) görürseniz
kodlama bozulmuştur, düzeltin.

## 11. Hata Ayıklama

### 11.1. Günlük dosyası

```text
%LOCALAPPDATA%\OnMuhasebe\OnMuhasebe\data\logs\onmuhasebe-YYYY-MM-DD.log
```

10 MB'ta `.1.log` / `.2.log` olarak döner; 30 günden eski dosyalar silinir.
Panikler de buraya yazılır (panik hook'u).

### 11.2. İlk bakılacaklar

| Belirti | Olası neden | Kontrol |
|---------|-------------|---------|
| "Veritabanı açılamadı" | Dizin izni, disk dolu | `AppConfig::ensure_data_dir`, log |
| "Şema çok yeni" | Eski exe + yeni DB | `SchemaTooNew { found, supported }` |
| "Migration zinciri bozuk" | `schema_migrations` satırı silinmiş | `MigrationChainBroken { missing }` |
| "Veritabanı kilitli" | Başka süreç yazıyor | `busy_timeout`, süreç listesi |
| "Tutar çok büyük" | Taşma | `AmountOverflow`; girdiyi küçült |
| "Çok fazla hatalı giriş" | Kilit | `LoginThrottled(kalan_sn)` |
| Arayüz boş | DB açılmadı, karşılama ekranı | `startup_error` alanı |

### 11.3. Backtrace

```text
set RUST_BACKTRACE=1
cargo run
```

Release derlemesinde `strip = "symbols"` olduğu için backtrace sınırlıdır;
panik ayıklaması debug derlemesinde yapılır.

### 11.4. Veritabanına bakmak

DB Browser for SQLite veya `sqlite3.exe` ile **kopya** üzerinde çalışın; canlı
dosyayı açıkken yazmayın (WAL). İlk komutlar:

```sql
PRAGMA integrity_check;
PRAGMA foreign_key_check;
SELECT version, name, applied_at FROM schema_migrations ORDER BY version;
SELECT COUNT(*) FROM products WHERE current_stock <> (
  SELECT COALESCE(SUM(quantity_delta), 0)
  FROM product_stock_movements m WHERE m.product_id = products.id
);
```

### 11.5. Yeniden üretilemeyen hata

Önce ortamı sabitleyin: sürüm (`Cargo.toml`), işletim sistemi, veri dizini,
ekran boyutu/DPI, kullanıcı rolü. Sonra aynı veriyi `ON_MUHASEBE_DATA_DIR` ile
ayrı bir klasöre kopyalayıp deneyin. Gerçek veriyle deney yapmayın.

## 12. Performans (Ölçüm Noktaları)

Projede performans iddiası ölçüme dayanır; "hızlı olmalı" yeterli değildir.

| Alan | Mekanizma | Nerede |
|------|-----------|--------|
| Liste çizimi | `ScrollArea::show_rows` + `visible_range` | `src/ui/virtual_list.rs` |
| Arama | SQL `LIKE 'önek%'` + grup başına 5, toplam 20 | `src/db/search.rs` |
| Yazma | WAL + `synchronous=NORMAL` + `busy_timeout=5000` | `src/db/mod.rs` |
| Sorgular | `(customer_id, invoice_date)`, `(product_id, movement_date)` gibi bileşik indeksler | `src/db/migrations.rs` |
| Bellek | Sayfalama yok; liste `Vec` olarak tutulur, çizim sanal | `app/*_screen.rs` |
| Ölçüm testleri | `scale_up`, `soak`, `ui_health`, `bulk_data` | `tests/stress/` |

Yaklaşım: yeni özellik eklerken önce mevcut ölçüm testini çalıştırın, sonra
değişikliği yapın, sonra tekrar ölçün. Aynı makinede %30'dan fazla yavaşlama
varsa nedenini bulmadan birleştirmeyin. Ölçüm testleri `#[ignore]` olduğu için
release profilinde çalıştırılır.

## 13. Sürümleme ve Release

### 13.1. Tek doğruluk kaynağı

Sürüm yalnızca `Cargo.toml` içindeki `version` alanıdır. `build-msi.bat` bu
satırı okur ve MSI adını üretir:

```text
cargo wix ... --output dist\OnMuhasebe-<sürüm>.msi
```

Uygulama içindeki sürüm `env!("CARGO_PKG_VERSION")` ile aynı değeri okur; iki
yerde ayrı sürüm yazmayın.

### 13.2. MSI kimlikleri

| Alan | Değer / kural |
|------|----------------|
| `UpgradeCode` | Sabit: `B5E3F1A2-9C4D-4E8B-8F6A-1D2E3C4B5A69` |
| `Product Id` | `*` — her derlemede yeni ProductCode |
| `AllowSameVersionUpgrades` | `yes` — aynı sürüm yeniden kurulabilir |
| `InstallScope` | `perMachine` (Program Files, yönetici onayı) |
| `Platform` | `x64` |
| `Language` | `1055` (Türkçe) |

`UpgradeCode` değişirse eski sürüm kaldırılmaz, iki ürün yan yana kurulur. Fork
yapanlar kendi ürünleri için **yeni** bir GUID üretmelidir.

### 13.3. Release kontrol listesi

1. `cargo fmt --check`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test --all-targets` (432 geçmeli)
4. `cargo test --release -- --ignored --test-threads=1` (en az bir kez)
5. `build-msi.bat` → `dist\OnMuhasebe-<sürüm>.msi`
6. MSI'yı temiz bir makinede kur, ilk açılışı doğrula
7. `tests/msi_install_test.ps1` (yönetici) ile kur/yükselt/kaldır zinciri
8. Sürüm notları + GitHub Release

Kod imzalama yoktur; SmartScreen "bilinmeyen yayıncı" uyarısı beklenen
davranıştır ve sürüm notlarında belirtilir.

## 14. Katkı ve İletişim

- PR/issue süreci: `CONTRIBUTING.md`
- Fork ve ticari kullanım: `FORK_REHBERI.md`
- Yol haritası: `ROADMAP.md`
- Yapay zekâ ile geliştirme: `AI_DEVRALMA.md`, `AI_PROMPTS.md`
- Kullanıcı belgeleri: `docs/`
- Lisans: BSD 3-Clause, telif (c) 2026, @furcanomer

Katkı gönderirken yapay zekâ kullandıysanız bunu PR açıklamasında belirtin;
sorumluluk sizde kalır. Kod incelemesinde "neden" sorusuna yanıt aranır; yalnızca
"çalışıyor" yeterli değildir.

Son güncelleme: 2026-10-27
