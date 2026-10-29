# Yol Haritası (ROADMAP)

Bu belge, v1.0.1 sonrası planları, fork'lara bırakılan özellikleri ve bilinçli
olarak yapılmayacakları açıklar. Tarih taahhüdü içermez.

## 1. Mevcut Durum (v1.0.1)

| Kalem | Değer |
|-------|-------|
| Sürüm | 1.0.1 (`Cargo.toml`) — erken sürüm |
| Şema | v7 (migration v1..v7) |
| Yayın | v1.0.1 (önerilen), v1.0.0 (tarihsel) |
| Platform | Windows 10+ x64, native (webview yok) |
| Test | 453 (432 hızlı + 21 ağır) |
| Uygulama boyutu | 20.065.792 bayt (release exe, statik runtime) |
| MSI boyutu | 8.478.720 bayt |
| Lisans | BSD 3-Clause, telif (c) 2026, @furcanomer |
| Ekran | 9 görünüm, 6 rapor, 4 ayar sekmesi, 17 kısayol |
| Kullanıcı | 2 rol (yönetici, operatör), Argon2id şifre |

v1.0.1 günlük kullanıma uygundur: cari, ürün/stok, alış-satış faturası,
tahsilat/ödeme, kasa/banka, gelir/gider, raporlar, CSV, yedekleme, geri yükleme
ve MSI kurulum paketi çalışır durumdadır. Ancak geniş ölçekli gerçek kullanıcı
beta dönemi tamamlanmadığı için "erken sürüm" olarak işaretlenmiştir.

Kapsam dışı olduğu için 1.0'da bulunmayan başlıklar: e-Fatura, PDF/XLSX çıktısı,
çoklu şirket, döviz, virman, çek/senet, ağ paylaşımı, otomatik güncelleme ve
bulut senkron. Ayrıntı ve gerekçeler 3. bölümdedir.

## 2. Sürdürücü Tarafından Planlananlar (Kısa Vadeli)

Bu bölüm proje sahibinin (@furcanomer) kendi yapmayı planladığı işleri listeler.

| Alan | İş | Durum |
|------|----|-------|
| Bakım | Bağımlılık güncellemeleri (yaklaşık 3 ayda bir), `cargo audit` taraması | Planlı |
| Hata | Kullanıcı bildirimleri; veri bütünlüğü hataları öncelikli | Sürekli |
| Belge | Ekran görüntüleri (27 zorunlu + 7 isteğe bağlı), kısa video | Planlı |
| 1.1 (olası) | Klavye erişilebilirliği, küçük UX düzeltmeleri, hata mesajı iyileştirmeleri | Değerlendirmede |
| 2.0 (olası) | Aşağıdaki "fork'lara bırakılanlar"dan bazılarının entegrasyonu | Belirsiz |

Not: 1.1 ve 2.0 için tarih yoktur. Öncelik, mevcut sürümün doğruluğu ve
kullanıcı verisinin güvenliğidir.

### 2.1. 1.1 için aday işler

| Aday | Gerekçe | Risk |
|------|---------|------|
| Tablo başlıklarından sıralama | Kullanıcı listeleri tarayarak arıyor | Düşük |
| Tarih aralığı hatırlama | Her raporda aynı aralığı girmek yorucu | Düşük |
| Klavye ile form gezinme | Veri girişi hızlıdır | Orta |
| Hata mesajlarına çözüm ipucu | Destek yükünü azaltır | Düşük |
| CSV içe aktarmada sütun eşleme | Farklı başlıklı dosyalar | Orta |

Bu liste bağlayıcı değildir; kullanıcı geri bildirimiyle değişebilir.

## 3. Bilinçli Olarak Fork'lara Bırakılanlar

Aşağıdaki özellikler **1.0'da YOKTUR ve @furcanomer tarafından eklenmesi
planlanmıyor**. Fork edip ticari ürün yapanlar için fırsat alanlarıdır; her biri
kendi tasarımını, testini ve bakımını gerektirir.

| # | Özellik | Neden kapsam dışı | Fork için not |
|---|---------|-------------------|---------------|
| 1 | e-Fatura / e-Arşiv gönderimi | GİB onaylı özel entegratör ve API sözleşmesi gerekir | Entegratör seçimi size ait; sabit ad koda gömülmemeli |
| 2 | PDF fatura ve yazdırma | A4 tasarım, Türkçe fatura biçimi, yazıcı desteği | Fatura verisi hazır (`invoice_lines` snapshot) |
| 3 | XLSX dışa aktarma | Ek bağımlılık ve şablon bakımı | CSV altyapısı (`src/csv.rs`) başlangıç noktası |
| 4 | Çoklu şirket (multi-tenant) | Tüm tablolara şirket ayrımı = büyük migration | Tek veritabanı, çok şirket en zor senaryo |
| 5 | Döviz ve kur takibi | Kur kaynağı, yuvarlama ve raporlama karmaşası | `Money` yalnızca TL kuruş varsayar |
| 6 | Virman (hesaplar arası transfer) | Kasa/banka modeli tek yönlü hareket varsayar | Çift kayıt (çıkış + giriş) tasarımı gerekir |
| 7 | Çek / senet | Portföy, vade, tahsil durumu, hatırlatma | Ayrı modül; cari ve kasa ile ilişkilendirme |
| 8 | Ağ paylaşımı / çok kullanıcı | SQLite WAL ağ üzerinde riskli; kilit ve bozulma | Daha güvenlisi PostgreSQL'e geçiş |
| 9 | Otomatik güncelleme | İndirme, imza doğrulama, geri alma altyapısı | Şu an güncelleme, yeni MSI'ın elle kurulmasıyla yapılır |
| 10 | Bulut senkron | Sunucu, kimlik, KVKK yükümlülüğü | Yerel çalışma temel felsefedir |
| 11 | Mobil uygulama | Ayrı teknoloji yığını ve ayrı ürün | Masaüstü öncelikli |
| 12 | Kod imzalama | Ücretli sertifika ve dağıtım süreci | İmzasız kurulumda SmartScreen uyarısı normaldir |
| 13 | Çoklu dil (i18n) | Tüm metinlerin anahtarlanması = büyük refactor | Şu an yalnızca Türkçe |

Ortak kural: bu başlıklardan biri istenirse **önce tasarım** yapılmalıdır
(veri modeli, migration, yetki, test). Doğrudan kodlamaya başlamak, mevcut
korelasyonları bozar.

### 3.1. Fork için önerilen sıra

| Sıra | Özellik | Neden bu sırada |
|------|---------|-----------------|
| 1 | PDF fatura | Mevcut veri yeterli; şema değişikliği gerekmez |
| 2 | XLSX dışa aktarma | CSV altyapısı hazır; bağımlılık kararı verilir |
| 3 | Virman | Kasa modeline iki hareket; migration gerekir |
| 4 | Çek/senet | Yeni tablolar ve vade mantığı |
| 5 | Döviz | Tüm para alanlarını etkiler; en riskli |
| 6 | Çoklu şirket | Tüm tablolara şirket ayrımı; en büyük refactor |
| 7 | e-Fatura | Dış servis, sözleşme ve operasyon gerektirir |

Sıra öneridir; her adımda mevcut 453 testin korunması ve yeni testlerin
eklenmesi beklenir.

## 4. Yapılmayacaklar (Bilinçli Reddedilenler)

Bu başlıklar ürün felsefesine aykırı olduğu için yapılmayacaktır:

| Başlık | Gerekçe |
|--------|---------|
| Genel muhasebe (mizan, yevmiye, defteri kebir) | Bu program "ön muhasebe"dir; kapsam ve sorumluluk farklıdır |
| Vergi beyannamesi hazırlama/gönderme | GİB entegrasyonu ve mali müşavir alanı; ayrı ürün |
| Bordro ve İK | Mevzuat yoğun, ayrı ürün |
| Üretim / ERP modülleri | Kapsam dışı |
| Yalnızca bulut (cloud-only) | Yerel ve çevrimdışı çalışma temel felsefe |
| Reklam, telemetri, kullanım izleme | Kullanıcı verisi toplanmaz |
| Zorunlu hesap açma / aktivasyon | Çevrimdışı kurulum ve kullanım korunur |

## 5. 2.0 Vizyonu (Uzun Vadeli, Belirsiz)

- **Tarih taahhüdü yoktur.** 2.0 çıkmayabilir; çıkarsa kapsamı bu belgede
  güncellenir.
- Veri taşınabilirliği hazırdır: `.onm-backup` yedekleri ve CSV dışa aktarma
  mevcuttur. Kullanıcı 1.0 verisini kaybetmeden yeni sürüme geçebilmelidir.
- Potansiyel başlıklar: isteğe bağlı PostgreSQL arka ucu, çoklu şirket,
  e-Fatura için adaptör çatısı, gelişmiş raporlar.
- Kural: 2.0'da da para/miktar tipleri, iptal modeli ve migration disiplini
  korunur. Bunlar pazarlık konusu değildir.

## 6. Fork'lar Nasıl Katkı Sağlayabilir?

| Katkı türü | Yol |
|------------|-----|
| Hata düzeltmesi | Upstream'e PR (`CONTRIBUTING.md`) |
| Belge düzeltmesi | Upstream'e PR |
| Yeni özellik (e-Fatura, çoklu şirket vb.) | Fork'ta kalabilir; upstream'e göndermek zorunlu değil |
| Fikir / kapsam önerisi | Issue açın; uygunsa tartışılır |
| Çeviri | Upstream'de i18n olmadığı için fork'ta yapılır |

BSD 3-Clause, değişikliklerinizi paylaşma zorunluluğu getirmez. Yine de hata
düzeltmelerini upstream'e göndermek, aynı hatanın her fork'ta tekrar
düzeltilmesini önler.

## 7. Sürüm Politikası

Semver: `MAJOR.MINOR.PATCH`.

| Bileşen | Ne zaman artar | Örnek |
|---------|----------------|-------|
| MAJOR | Geriye uyumsuz değişiklik, zorunlu veri dönüşümü | Şema v8 + dönüştürme aracı |
| MINOR | Geriye uyumlu yeni özellik | Yeni rapor türü |
| PATCH | Hata düzeltmesi, belge | Bakiye yuvarlama hatası |

Ek kurallar:

- Sürümün tek kaynağı `Cargo.toml` içindeki `version` alanıdır; MSI adı buradan
  üretilir.
- Yeni migration eklemek MINOR sayılır (veri korunur, otomatik uygulanır).
- Veri dönüşümü gerektiren migration MAJOR sayılır ve sürüm notlarında açıkça
  belirtilir.
- Yedek uyumluluğu korunur: yeni sürüm eski `.onm-backup` dosyalarını
  okuyabilmelidir.

## 8. Sürüm ve Yayın Durumu

| Sürüm | Durum | Not |
|-------|-------|-----|
| v1.0.0 | Tarihsel | İlk yayın; statik runtime öncesi MSI |
| v1.0.1 | **Önerilen** | crt-static, güncel belgeler, 17 ekran görüntüsü, erken sürüm notu |

- Depo: kaynak + `LICENSE` + belgeler; `dist/`, `target/` ve `archive/` hariç.
- CI: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all-targets` (Windows runner).
- Ağır testler CI'da çalıştırılmaz; release öncesi yerelde koşulur.
- Release: `dist\OnMuhasebe-<sürüm>.msi` + kaynak zip + `SHA256SUMS` + sürüm
  notları.
- Kod imzalama kapsam dışıdır; SmartScreen uyarısı sürüm notlarında belirtilir.
- Bakım ve sürüm çıkarma adımları: `SURDURUCU_NOTLARI.md`.
- Sürüm atlama (2.0) planı: `SURUM_2_GECIS.md`.

## 9. İletişim

- GitHub: <https://github.com/furcanomer/OnMuhasebe>
- Hata ve öneri: Issues
- Katkı: `CONTRIBUTING.md`
- Fork ve ticari kullanım: `FORK_REHBERI.md`
- Lisans: BSD 3-Clause, telif (c) 2026, @furcanomer

Son güncelleme: 2026-10-29
