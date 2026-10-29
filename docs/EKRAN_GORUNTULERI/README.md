# Ekran Görüntüleri Rehberi

Bu klasör, kullanım kılavuzu ve kurulum belgelerinde kullanılacak **ekran
görüntülerini** barındırır. Program ekran görüntülerini otomatik olarak üretmez;
görüntüler **elle** alınır ve bu klasöre kaydedilir.

> Bu belge bir geliştirici rehberi değildir. Yalnızca hangi ekran görüntüsünün
> gerektiğini, nasıl alınacağını ve nelere dikkat edileceğini açıklar.

## Mevcut Görseller (17 adet)

Görseller proje kökündeki `screenshots/` klasöründen bu klasöre kopyalanmıştır
(dosya adları korunmuştur). Boyutlar yaklaşıktır.

| Dosya | Boyut | Açıklama (dosya adından üretildi) |
|-------|-------|-----------------------------------|
| `Alis faturalari ekrani.png` | 310 KB | Alış Faturaları Ekranı |
| `Ayarlar ekrani.png` | 348 KB | Ayarlar Ekranı |
| `Cari hesaplar ekrani.png` | 334 KB | Cari Hesaplar Ekranı |
| `Gelir ve gider ekrani.png` | 397 KB | Gelir ve Gider Ekranı |
| `Genel bakis ekrani.png` | 341 KB | Genel Bakış Ekranı |
| `Kasa ve banka ekrani.png` | 426 KB | Kasa ve Banka Ekranı |
| `kurulum ekrani.png` | 340 KB | Kurulum Ekranı |
| `kurulum ekrani-1.png` | 294 KB | Kurulum Ekranı 1 |
| `kurulum ekrani-2.png` | 324 KB | Kurulum Ekranı 2 |
| `kurulum ekrani-3.png` | 328 KB | Kurulum Ekranı 3 |
| `kurulum ekrani-4.png` | 327 KB | Kurulum Ekranı 4 |
| `kurulum ekrani-5.png` | 388 KB | Kurulum Ekranı 5 |
| `kurulum ekrani-6.png` | 347 KB | Kurulum Ekranı 6 |
| `kurulum ekrani-7.png` | 330 KB | Kurulum Ekranı 7 |
| `Raporlar ekrani.png` | 393 KB | Raporlar Ekranı |
| `Satis faturalari ekrani.png` | 360 KB | Satış Faturaları Ekranı |
| `Urun ve stok ekrani.png` | 333 KB | Ürün ve Stok Ekranı |

Toplam: 17 görsel, yaklaşık 5,7 MB.

## Durum

- **Otomatik üretim:** Yok.
- **Mevcut görüntüler:** 17 adet (yukarıdaki tablo).
- **Hedef:** 27 görüntü (aşağıdaki planlanan liste); 17'si mevcut.


## 1. Görüntü alma kuralları

### 1.1. Pencere boyutu

- Tercih edilen pencere boyutu: **1180 × 760** piksel.
- Dar ekran testi için alternatif: **900 × 600** piksel.
- Pencereyi ekranın ortasına alın; masaüstü arka planı görünmesin.

### 1.2. Alım yöntemi (Windows)

1. İlgili ekranı programda açın.
2. **Win + Shift + S** tuşlarına basın (Ekran Alıntısı aracı).
3. **Pencere** veya **Dikdörtgen** modunu seçin; yalnızca uygulama penceresini
   alın.
4. Alıntı panoya kopyalanır; **Paint** veya **Ekran Alıntısı Aracı**'na yapıştırın.
5. **PNG** olarak bu klasöre, aşağıdaki tablodaki adla kaydedin.

Alternatif: **Win + PrtScn** tüm ekranı `Resimler\Ekran Görüntüleri` klasörüne
kaydeder; sonra kırpabilirsiniz.

### 1.3. İçerik kuralları

- **Örnek veriler kullanın.** Gerçek müşteri adları, vergi numaraları, telefon
  numaraları, IBAN'lar, e-postalar ve gerçek tutarlar **görünmemelidir**.
- Örnek cari: `Örnek Ticaret Ltd. Şti.`, `Demo Müşteri`.
- Örnek ürün: `Örnek Ürün 01`, barkod `8690000000001`.
- Örnek vergi no: `1234567890`; IBAN: `TR00 0000 0000 0000 0000 0000 00`.
- Kullanıcı adı olarak `demo` veya `ornek` kullanın.
- Bildirim, hata veya uyarı kutusu varsa görüntüyü engellemeyin; kapatın.
- Fare imleci görüntüde olmasın (mümkünse pencere dışına alın).
- Türkçe karakterlerin doğru göründüğünden emin olun.

### 1.4. Dosya adlandırma

- Küçük harf, tire ayırıcı, `.png` uzantısı.
- Numara sırası korunmalıdır (01, 02, …).
- Örnek: `04-genel-bakis.png`

## 2. Planlanan Ekranlar

| # | Dosya adı | Ekran / durum | Nasıl açılır? |
|---|-----------|---------------|----------------|
| 01 | `01-ilk-yonetici.png` | İlk Yönetici Oluştur | Temiz kurulumda programı açın (kullanıcı yokken) |
| 02 | `02-sihirbaz-1.png` | Kurulum sihirbazı, adım 1 (Şirket bilgileri) | İlk yöneticiyi oluşturduktan sonra |
| 03 | `03-giris.png` | Giriş ekranı | Oturumu kapatın |
| 04 | `04-genel-bakis.png` | Genel Bakış | Giriş sonrası / `Ctrl+1` |
| 05 | `05-cari-liste.png` | Cari Hesaplar listesi | `Ctrl+2` |
| 06 | `06-cari-form.png` | Yeni cari formu | Cari listesinde **Yeni cari** |
| 07 | `07-cari-ayrinti.png` | Cari ayrıntısı (bakiye + faturalar) | Listede **Ayrıntı** |
| 08 | `08-urun-liste.png` | Ürün ve Stok listesi | `Ctrl+3` |
| 09 | `09-urun-form.png` | Yeni ürün formu | **Yeni ürün** |
| 10 | `10-stok-hareket.png` | Stok hareketi formu | Ürün ayrıntısı > **Yeni hareket** |
| 11 | `11-fatura-liste.png` | Satış Faturaları listesi | `Ctrl+5` |
| 12 | `12-fatura-form.png` | Yeni fatura formu (satırlar ve toplamlar) | **Yeni fatura** |
| 13 | `13-fatura-ayrinti.png` | Fatura ayrıntısı (ödeme rozeti + dağıtımlar) | Listede **Ayrıntı** |
| 14 | `14-kasa-hesaplar.png` | Kasa ve Banka > Hesaplar | `Ctrl+6` |
| 15 | `15-odeme-form.png` | Yeni tahsilat formu (fatura dağıtımı) | **Yeni Tahsilat** |
| 16 | `16-odeme-ayrinti.png` | Tahsilat/ödeme ayrıntısı | Hareketler > **Ayrıntı** |
| 17 | `17-gelir-gider-liste.png` | Gelir ve Gider > Hareketler | `Ctrl+7` |
| 18 | `18-gelir-gider-form.png` | Yeni gider formu (kasa etkisi önizlemesi) | **Yeni Gider** |
| 19 | `19-ozet.png` | Gelir ve Gider > Özet | Gelir/Gider > **Özet** sekmesi |
| 20 | `20-rapor-cari-ekstre.png` | Raporlar > Cari Ekstre | `Ctrl+8` |
| 21 | `21-rapor-stok.png` | Raporlar > Stok Durumu | Raporlar > **Stok Durumu** |
| 22 | `22-ayarlar-genel.png` | Ayarlar > Genel | `Ctrl+9` |
| 23 | `23-ayarlar-yedekleme.png` | Ayarlar > Yedekleme | Ayarlar > **Yedekleme** |
| 24 | `24-kullanicilar.png` | Ayarlar > Kullanıcılar | Ayarlar > **Kullanıcılar** (yönetici) |
| 25 | `25-arama-paleti.png` | Genel arama sonuçları | `Ctrl+K` |
| 26 | `26-klavye-kisayollari.png` | Klavye kısayolları penceresi | **Yardım > Klavye kısayolları** |
| 27 | `27-hakkinda.png` | Hakkında penceresi | **Yardım > Hakkında** |

### 2.1. Ek (isteğe bağlı) görüntüler

Belgelerde gerektiğinde kullanılabilecek ekler:

| Dosya adı | Ekran |
|-----------|-------|
| `28-rapor-cari-bakiye.png` | Raporlar > Cari Bakiye Özeti |
| `29-rapor-satis-alis.png` | Raporlar > Satış/Alış Özeti |
| `30-rapor-kasa.png` | Raporlar > Kasa/Banka Hareket Özeti |
| `31-rapor-gelir-gider.png` | Raporlar > Gelir/Gider Özeti |
| `32-yedek-geri-yukle.png` | Yedeği geri yükle penceresi |
| `33-sifre-degistir.png` | Ayarlar > Güvenlik |
| `34-kategori-listesi.png` | Gelir/Gider > Kategoriler |

## 3. Görüntüleri hazırlama ipuçları

1. **Temiz bir veritabanı** kullanın. Gerçek veriyle çalışmayın; yeni bir Windows
   kullanıcısı veya `ON_MUHASEBE_DATA_DIR` ortam değişkeni ile ayrı bir veri
   klasörü kullanabilirsiniz.
2. **Tutarlı örnek veri** oluşturun: 3–5 cari, 5–8 ürün, 2 alış ve 2 satış
   faturası, 1 tahsilat, 1 ödeme, 3 gelir/gider kaydı.
3. Aynı gün/tarih aralığını kullanın; ekran görüntüleri arasında tutarlılık olsun.
4. Listelerde arama/filtre kutusunu boş bırakın; tüm örnek kayıtlar görünsün.
5. Form ekranlarında bazı alanları doldurun, bazılarını boş bırakın; gerçek
   kullanımı yansıtsın.
6. Hata/uyarı pencerelerini kapatın; yalnızca bilinçli olarak belgelemek
   istediğiniz uyarıları görüntüleyin.
7. Görüntüyü kaydettikten sonra dosya adını ve sırasını kontrol edin.

## 4. Belgelerde kullanım

Görüntüler eklendiğinde Markdown'da **resim sözdizimi** kullanılır: ünlem işareti,
köşeli parantez içinde alternatif metin ve parantez içinde göreli dosya yolu.
Örnek dosya yolu: `EKRAN_GORUNTULERI/04-genel-bakis.png`.

- Yol, `docs/` klasörüne göre görecelidir.
- Alternatif metin, ekranın ne gösterdiğini açıklamalıdır.
- Görüntü eklenmeden önce bağlantıyı kullanmayın; kırık bağlantı oluşur.

## 5. Gizlilik ve güvenlik

- Ekran görüntülerinde **kişisel veri** (gerçek müşteri/tedarikçi bilgileri, TC
  kimlik numaraları, IBAN, telefon, e-posta) bulunmamalıdır.
- Görüntüleri paylaşmadan önce tek tek kontrol edin.
- Gerçek veritabanınızla alınmış görüntüleri bu klasöre **koymayın**.
- Ekran görüntüleri programla birlikte dağıtılacaksa lisans (BSD 3-Clause) ve
  telif bilgisiyle uyumlu olmalıdır.

## 6. Kontrol listesi

Görüntüleri teslim etmeden önce:

- [ ] 27 zorunlu görüntü eksiksiz mi?
- [ ] Dosya adları tablodakiyle birebir aynı mı?
- [ ] Tümü PNG mi?
- [ ] Pencere boyutu tutarlı mı (1180 × 760 veya 900 × 600)?
- [ ] Gerçek/hassas veri yok mu?
- [ ] Türkçe karakterler doğru mu?
- [ ] Fare imleci ve istenmeyen pencereler görünmüyor mu?
- [ ] Belgelerdeki görüntü yolları doğru mu?

---

Son güncelleme: 2026-10-27
