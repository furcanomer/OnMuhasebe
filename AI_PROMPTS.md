# AI Promptları — Vibe Coding Başlangıç Kütüphanesi

> Bu dosya, projeyi fork edip yapay zekâ ile geliştirmek isteyenler için hazır
> **kopyala-yapıştır** promptları içerir. Promptlar dört boşluk girintili düz
> metin blokları hâlindedir; bloğun tamamını seçip yapay zekâya yapıştırın.
> Projenin tanıtımı için `AI_DEVRALMA.md` dosyası birincil kaynaktır.

## Kullanım Kılavuzu

1. **Her yeni sohbette Prompt 1'i kullanın.** Yapay zekâ `AI_DEVRALMA.md`
   dosyasını okuyup özetlemeden kod yazmamalıdır.
2. **Sonra tek bir iş seçin.** Prompt 2–9 arasından ihtiyacınıza uyan şablonu
   kopyalayın; köşeli parantezli alanları doldurun.
3. **Önce plan, sonra kod.** Her prompt "önce plan öner, onay al" kuralıyla
   biter. Planı okumadan onay vermeyin.
4. **Her adımdan sonra test edin.** En az `cargo test --all-targets`; kod
   gönderilmeden önce `cargo fmt --check` ve `cargo clippy --all-targets -- -D
   warnings` de çalıştırılmalıdır.
5. **Küçük parçalar hâlinde ilerleyin.** Tek sohbette tek özellik; yarım kalan
   refactor bırakılmaz.
6. **Test silinmez.** Kırılan test yeni davranışa uyarlanır ve gerekçesi yazılır.
7. **Değişiklikleri sürüm kontrolünde tutun.** Proje Git deposu olarak
   dağıtılmaz; fork ettikten sonra `git init` ile kendi deponuzu kurun ve her
   onaylı adımdan sonra commit edin. Böylece yapay zekânın beklenmeyen bir
   değişikliği geri alınabilir.

### İlk kurulum (tek seferlik)

- Depoyu klonlayın veya klasörü kopyalayın; `Cargo.toml` ve `Cargo.lock`
  birlikte bulunmalıdır.
- `cargo build` çalıştırın. İlk derleme uzundur (SQLite kaynaktan derlenir);
  sonraki derlemeler hızlıdır.
- `cargo test --all-targets` ile 432 hızlı testin geçtiğini doğrulayın.
- MSI üretimi için WiX 3.14 gerekir; `build-msi.bat` bu araç zincirini kullanır.
- Yapay zekâya vermeden önce `AI_DEVRALMA.md` dosyasının güncel sürümde
  olduğundan emin olun (sürüm 1.0.0, şema v7).

### Hangi dosya ne işe yarar?

| Dosya | Rol |
|-------|-----|
| `AI_DEVRALMA.md` | Yapay zekâya verilecek proje tanıtımı (mimari, şema, kurallar) |
| `AI_PROMPTS.md` | Bu dosya: hazır başlangıç promptları |
| `README.md` | İnsan için teknik özet |
| `docs/` | Son kullanıcı belgeleri |

### Prompt seçim tablosu

| İhtiyaç | Prompt |
|---------|--------|
| Yeni sohbet / projeyi tanıtma | Prompt 1 |
| Yeni ekran | Prompt 2 |
| Yeni tablo veya kolon | Prompt 3 |
| Yeni rapor | Prompt 4 |
| e-Fatura tasarımı | Prompt 5 |
| Test yazma | Prompt 6 |
| Refactor / temizlik | Prompt 7 |
| Hata ayıklama | Prompt 8 |
| GitHub / MSI release | Prompt 9 |
| Kısayol araması | Prompt 10 |

## Prompt 1 — İlk Tanıtım (ZORUNLU)

Her yeni sohbetin ilk mesajı. Kod yazdırmaz; yalnızca projeyi kavratır.

    Bu projeyi devralacaksın. Proje kök klasöründe AI_DEVRALMA.md dosyasını oku ve özetle. Özet şunları içermeli:
    - Proje ne yapar, hangi teknoloji yığınını kullanır (Rust + egui/eframe + SQLite, native Windows)
    - Kritik kurallar: para Money (kuruş), miktar Quantity (mili-birim), f32/f64 yasak; migration yalnızca listenin sonuna eklenir; kayıt silinmez (iptal/pasif); çoklu tablo yazan işlemler tek transaction; test silme veya gevşetme yok
    - Katman sınırları: main → app → ui / db → error; UI katmanında SQL yok; iş modeli GUI ve SQL bilmez
    - Veri modeli: hangi tablolar var, hangi korelasyonlar korunuyor (stok toplamı, fatura ödenen tutar, kasa bakiyesi, cari bakiye)

    Okuduktan sonra "Hazırım, ne eklemek istiyorsun?" diye sor. HENÜZ KOD YAZMA.

Ek not: Yapay zekâ özeti eksik ya da yanlış verdiyse ("f32 kullanılabilir",
"tablo silinebilir" gibi) düzeltmesini isteyin; özet doğru olmadan ilerlemeyin.

## Prompt 2 — Yeni Ekran Ekleme

    [Ekran adı] ekranı eklemek istiyorum. Şunları yap:
    1. AI_DEVRALMA.md dosyasının "Yeni Özellik Ekleme Rehberi" bölümündeki "A) Yeni ekran ekle" adımlarını uygula.
    2. src/app.rs içindeki View enum'a varyant ekle; title() ve description() eşlemelerini yaz; View::ALL listesine menü sırasına uygun ekle.
    3. src/app/[yeni]_screen.rs dosyasını oluştur (ekran durumu + komut metotları).
    4. src/ui/[yeni].rs dosyasını oluştur (yalnızca çizim; veri OnMuhasebeApp üzerinden okunur, SQL yazma).
    5. src/ui/mod.rs ve src/ui/layout.rs içine modül kaydı ve görünüm dağıtımı ekle.
    6. Gerekliyse src/db/[yeni].rs (SQL katmanı) ve src/[yeni].rs (saf iş modeli) ekle.
    7. 900x600 yerleşim testi ve davranış/entegrasyon testi yaz (tests/ altına).
    8. cargo fmt --check && cargo check --all-targets && cargo clippy --all-targets -- -D warnings && cargo test --all-targets çalıştır; kırılan test varsa DÜZELT, SİLME.
    Önce plan öner, onay al, sonra kodla.

Beklenen çıktı: `View` varyantı, iki yeni dosya, menüde görünen ekran ve iki test.
Ekran metinleri Türkçe olmalı; para/miktar alanları `Money` ve `Quantity`
tipiyle tutulmalıdır.

## Prompt 3 — Yeni Migration Ekleme

    Veritabanına [yeni tablo / yeni kolon] eklemek istiyorum. Şunları yap:
    1. AI_DEVRALMA.md dosyasının "Kritik Kurallar" bölümündeki migration kuralına uy.
    2. src/db/migrations.rs içindeki MIGRATIONS listesinin SONUNA yeni kayıt ekle. Mevcut v1..v7 kayıtlarına DOKUNMA.
    3. SQL'i "IF NOT EXISTS" KULLANMADAN yaz; hata gizlenmemeli (mevcut v1'deki app_metadata kalıbı istisnadır, o kayda dokunma). Birden çok ifade aynı migration içinde olabilir; tek transaction'da çalışır.
    4. Migration testi yaz: v1→vN, v2→vN, ..., v(N-1)→vN zincirlerinin hepsi geçmeli; mevcut veriler korunmalı (current_stock toplamı, invoices.paid_amount, kasa bakiyesi, cari bakiye).
    5. Yeni kolon eklersen mevcut kayıtlar için varsayılan değeri SQL içinde belirt; NULL'a düşen alan bırakma.
    6. cargo test --all-targets çalıştır.
    Önce plan öner, onay al, sonra kodla.

Uyarı: Şema sürümü uygulamadan yeni ise açılış `SchemaTooNew` hatası verir.
Migration numarasını asla geriye dönük değiştirmeyin.

## Prompt 4 — Yeni Rapor Ekleme

    [Rapor adı] raporu eklemek istiyorum. Şunları yap:
    1. AI_DEVRALMA.md dosyasının "Yeni Özellik Ekleme Rehberi" bölümündeki "C) Yeni rapor ekle" adımlarını uygula.
    2. src/report.rs içine rapor modelini ve filtre tiplerini ekle (saf hesaplama; SQL yok).
    3. src/db/reports.rs içine salt okunur sorguyu ekle.
    4. src/ui/reports.rs içine sekme çizimini, src/app/report_screen.rs içine filtre durumunu ekle.
    5. CSV dışa aktarma noktasını src/app/export_screen.rs içine ekle (src/csv.rs biçimi: noktalı virgül ayırıcı, UTF-8 BOM, virgül ondalık).
    6. Test yaz: korelasyon testi + 900x600 yerleşim testi + CSV başlık/satır sayısı.
    7. cargo test --all-targets çalıştır.
    Önce plan öner, onay al, sonra kodla.

Beklenen çıktı: yeni sekme, CSV çıktısı ve en az üç test. Rapor sorguları salt
okunur olmalı; hiçbir rapor tabloya yazmamalıdır.

## Prompt 5 — e-Fatura Entegrasyonu (İLERİ DÜZEY)

Bu projede e-Fatura yoktur. Aşağıdaki prompt yalnızca tasarım ister; kod yazdırmaz.

    Bu projeye e-Fatura gönderimi eklemek istiyorum. ÖNEMLİ UYARILAR:
    1. AI_DEVRALMA.md dosyasının "Bilinen Sınırlamalar" bölümünü oku. e-Fatura bu projede YOKTUR; sıfırdan tasarım gerekir.
    2. Türkiye'de e-Fatura gönderimi GİB onaylı özel entegratör ile API entegrasyonu gerektirir. Kendi entegratörünü seçmelisin (örnek: Uyumsoft, İzibiz, Foriba, Digital Planet). Sabit entegratör adı koda GÖMÜLMEZ.
    3. Tasarım öner:
       - Yeni tablo: e_invoice_outbox (yeni migration; invoice_id FK, status, entegrator_ref, hata_mesaji, retry_sayisi, created_at)
       - Yeni modül: src/e_invoice/ (trait EntegratorAdapter + her entegratör için ayrı impl)
       - Yeni ekran: Ayarlar > e-Fatura (entegratör seçimi, API anahtarı, ortam test/canlı)
       - Worker thread: kuyruktan iş alır, API'ye gönderir, sonucu ve hatayı yazar; yeniden deneme sınırı
       - Giden belge için invoice_lines snapshot alanları yeterli mi, değilse hangi alanlar eksik
    4. Riskleri yaz: API anahtarı saklama, kişisel veri, GİB teknik kılavuz uyumu, iptal/itiraz akışı.
    5. Uygulama sırası öner (migration → adapter trait → sahte adapter ile test → gerçek adapter → arayüz).
    Önce TASARIM öner, onay al. HENÜZ KOD YAZMA.

## Prompt 6 — Test Yazma

    [Modül veya davranış] için test yazmak istiyorum. AI_DEVRALMA.md dosyasının "Test Stratejisi" bölümüne uy:
    1. Testleri tests/ altına ayrı dosya olarak yaz; kütüphane içi birim testleri ilgili modülün sonundaki #[cfg(test)] bloğuna gider.
    2. Mevcut testleri SİLME ve GEVŞETME.
    3. Her test bağımsız çalışabilmeli: tempfile ile geçici veritabanı kullan, gerçek kullanıcı verisine dokunma.
    4. Mutlu yol, sınır değer ve hata yolu senaryolarını ayrı ayrı yaz (sıfır, negatif, taşma, iptal edilmiş kayıt, yabancı anahtar ihlali).
    5. Para ve miktar karşılaştırmalarında Money ve Quantity kullan; f64 ile karşılaştırma yazma.
    6. cargo test --all-targets çalıştır ve sonucu raporla.
    Önce hangi senaryoları yazacağını listele, onay al, sonra kodla.

Beklenen çıktı: yeni test dosyası ve geçen test listesi. Test sayısı artabilir;
mevcut 453 testin hiçbiri kaybolmamalıdır.

## Prompt 7 — Refactor / Temizlik

    [Modül] için refactor istiyorum. AI_DEVRALMA.md dosyasının "Kritik Kurallar" bölümüne uy:
    1. Mevcut testleri KORUYARAK refactor yap; test dosyalarını değiştirme.
    2. Katman sınırlarını İHLAL ETME: UI katmanında SQL yok, iş modeli GUI ve SQL bilmez, tüm veri erişimi db katmanında.
    3. Para ve miktar Money/Quantity olarak KALMALI; ara hesaplarda i128 kullanımı korunmalı, yuvarlama kuralı (yarım kuruş yukarı) değişmemeli.
    4. Davranış değişikliği YAPMA; yalnızca okunabilirlik, tekrar ve isimlendirme.
    5. cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --all-targets çalıştır.
    6. Değişiklikleri küçük parçalara ayır; her parça derlenip test geçmeli.
    Önce plan öner, onay al, sonra kodla.

Beklenen çıktı: davranışı aynı kalan, testleri değişmemiş kod. Refactor sonrası
test sayısı ve test adları aynı kalmalıdır.

## Prompt 8 — Hata Ayıklama

    Şu hatayı alıyorum: [hata metni veya ekran görüntüsü özeti]
    AI_DEVRALMA.md dosyasının "Bilinen Sınırlamalar" ve "Sık Yapılan Hatalar" bölümlerini oku. Sonra:
    1. Önce HATAYI yeniden üret (hangi komut, hangi ekran, hangi veri).
    2. Kök nedeni bul: panik mi, SQL hatası mı, migration uyumsuzluğu mu, iş mantığı mı, transaction eksikliği mi.
    3. Aynı hatanın veri bütünlüğüne etkisini değerlendir (stok, kasa bakiyesi, fatura ödenen tutar, cari bakiye).
    4. Minimal düzeltme öner; ilgisiz refactor yapma.
    5. Regresyon testi ekle (tests/ altına); test hatayı düzeltmeden ÖNCE kırılmalı.
    6. cargo test --all-targets çalıştır.
    HENÜZ KOD YAZMA; önce neden ve çözüm planı öner.

Log yolu: `%LOCALAPPDATA%\OnMuhasebe\OnMuhasebe\data\logs`
(`onmuhasebe-YYYY-MM-DD.log`). Hata metnini buradan tam olarak kopyalayın;
özetlemek yerine ham kaydı verin.

## Prompt 9 — GitHub'a Yayınlama ve Release

    Projeyi kendi GitHub repoma yüklemek ve MSI release yayınlamak istiyorum. AI_DEVRALMA.md dosyasının "Fork ve Ticari Kullanım" bölümünü oku. Sonra:
    1. Bu projeyi fork'ladığımı kabul et; orijinal yazar @furcanomer.
    2. LICENSE (BSD 3-Clause) telif bildirimini KORU; 3. madde gereği orijinal yazarın adını onay imasında kullanma.
    3. README içinde kendi repo adresini yaz; orijinal projeyi "ilham kaynağı" olarak an.
    4. Marka: "Ön Muhasebe" adını ürün adı olarak kullanmak istiyorsan orijinal yazarla iletişime geç.
    5. gh auth login ile GitHub'a giriş yap.
    6. git init && git add . && git commit -m "Initial fork"
    7. gh repo create <repo-adi> --private --source=. --push
    8. MSI üret: build-msi.bat çalıştır; çıktı dist\OnMuhasebe-1.0.0.msi olmalı.
    9. gh release create v1.0.0 dist\OnMuhasebe-1.0.0.msi --title "v1.0.0" --notes "İlk fork sürümü"
    10. Repoyu hazır olunca public yap.
    Önce plan öner, her adımı onaylat.

Not: `dist/` ve `target/` sürüm kontrolüne eklenmemelidir. Kod imzalama
yoktur; SmartScreen "bilinmeyen yayıncı" uyarısı beklenen davranıştır.

## Prompt 10 — Kısayol Tablosu

Hangi durumda hangi promptu kullanacağınızı hızlıca seçmek için.

| Durum | Prompt | Not |
|-------|--------|-----|
| Yeni sohbet açtım | Prompt 1 | Her zaman ilk |
| Yeni ekran istiyorum | Prompt 2 | View + ui + app + db |
| Tablo/kolon ekleyeceğim | Prompt 3 | Yalnızca listenin sonuna |
| Rapor ekleyeceğim | Prompt 4 | CSV çıktısını unutma |
| e-Fatura düşünüyorum | Prompt 5 | Önce tasarım |
| Test kapsamı artacak | Prompt 6 | Mevcut testler korunur |
| Kodu toparlayacağım | Prompt 7 | Davranış değişmez |
| Hata alıyorum | Prompt 8 | Önce yeniden üret |
| Yayınlayacağım | Prompt 9 | Lisans korunur |
| Ne seçeceğimi bilmiyorum | Prompt 10 | Bu tablo |

### Son kontrol listesi

- [ ] Yapay zekâ `AI_DEVRALMA.md` özetini doğru verdi.
- [ ] Plan onaylandı, kod sonra geldi.
- [ ] `cargo fmt --check` temiz.
- [ ] `cargo clippy --all-targets -- -D warnings` temiz.
- [ ] `cargo test --all-targets` geçti (432 hızlı test).
- [ ] Ağır testler gerekiyorsa `cargo test --release -- --ignored --test-threads=1`.
- [ ] Hiçbir test silinmedi veya gevşetilmedi.
- [ ] Para/miktar `Money` ve `Quantity`; `f32`/`f64` yok.
- [ ] Yeni şema değişikliği varsa migration listenin sonuna eklendi.
- [ ] UI katmanına SQL yazılmadı.

Son güncelleme: 2026-10-27
