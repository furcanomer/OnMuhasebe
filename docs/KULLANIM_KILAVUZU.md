# On Muhasebe — Kullanım Kılavuzu

Bu kılavuz, **On Muhasebe** programını günlük işlerinde kullanan kişiler için
hazırlanmıştır. Her bölüm bir ekranı veya iş akışını anlatır; baştan sona okumak
zorunda değilsiniz, ihtiyacınız olan bölüme atlayabilirsiniz.

> **Kısa tanıtım:** On Muhasebe; cari hesapları, ürün ve stoğu, alış/satış
> faturalarını, kasa/banka hareketlerini, gelir/gider kayıtlarını ve raporları tek
> bir yerel masaüstü programında toplar. Veriler bilgisayarınızda kalır.

## İçindekiler

1. [Genel Bakış](#1-genel-bakış)
2. [Giriş ve Oturum](#2-giriş-ve-oturum)
3. [Genel Bakış Ekranı](#3-genel-bakış-ekranı)
4. [Cari Hesaplar](#4-cari-hesaplar)
5. [Ürün ve Stok](#5-ürün-ve-stok)
6. [Alış ve Satış Faturaları](#6-alış-ve-satış-faturaları)
7. [Kasa ve Banka](#7-kasa-ve-banka)
8. [Gelir ve Gider](#8-gelir-ve-gider)
9. [Raporlar](#9-raporlar)
10. [Ayarlar](#10-ayarlar)
11. [Yedekleme ve Geri Yükleme](#11-yedekleme-ve-geri-yükleme)
12. [CSV İçe ve Dışa Aktarma](#12-csv-i̇çe-ve-dışa-aktarma)
13. [Klavye Kısayolları](#13-klavye-kısayolları)
14. [Genel Arama](#14-genel-arama)
15. [Kullanıcı Rolleri](#15-kullanıcı-rolleri)
16. [İpuçları ve Püf Noktaları](#16-i̇puçları-ve-püf-noktaları)
17. [Veri Konumları ve Güvenlik](#17-veri-konumları-ve-güvenlik)
18. [Lisans ve Katkı](#18-lisans-ve-katkı)

---

## 1. Genel Bakış

### 1.1. Program ne yapar?

On Muhasebe bir **ön muhasebe** programıdır. Günlük ticari işlemlerinizi kayıt altına
alır ve basit raporlar üretir:

| Modül | Ne işe yarar? |
|-------|---------------|
| Cari Hesaplar | Müşteri ve tedarikçi kartları, bakiye takibi |
| Ürün ve Stok | Ürün kartları, fiyatlar, stok giriş/çıkış/düzeltme |
| Alış Faturaları | Tedarikçiden alınan faturalar |
| Satış Faturaları | Müşteriye kesilen faturalar |
| Kasa ve Banka | Nakit kasa ve banka hesapları, tahsilat/ödeme |
| Gelir ve Gider | Fatura dışı gelir/giderler ve kategoriler |
| Raporlar | Cari ekstre, bakiye, stok, satış/alış, kasa, gelir/gider |
| Ayarlar | Kilit, şifre, yedekleme, kullanıcılar |

### 1.2. Program ne yapmaz?

Aşağıdaki konular bu sürümün kapsamı dışındadır. Belgelerde bunlar yalnızca
"bu sürümde yok" anlamında anılır:

- Genel muhasebe (mizan, yevmiye defteri, hesap planı) tutmaz.
- Vergi beyannamesi veya resmî beyan üretmez.
- e-Fatura, e-Arşiv, e-Defter göndermez veya oluşturmaz.
- PDF veya Excel (XLSX) dosyası üretmez; dışa aktarma CSV biçimindedir.
- Çek/senet, kredi kartı, döviz ve virman işlemleri yoktur. Tüm tutarlar TL'dir.
- Otomatik güncelleme, bulut yedekleme veya ağ üzerinden çok kullanıcılı çalışma
  yoktur.

### 1.3. Temel çalışma kuralları

- **Kayıtlar silinmez.** Cari, ürün, kasa/banka hesabı ve kategori kayıtları
  "pasife alınır"; fatura, tahsilat/ödeme ve gelir/gider kayıtları "iptal edilir".
- **Geçmiş değiştirilmez.** Kesinleşmiş fatura düzenlenemez; stok hareketi
  silinemez. Düzeltme, ters yönlü yeni kayıtla yapılır.
- **Bakiyeler otomatik hesaplanır.** Cari bakiye, kasa bakiyesi ve stok miktarı
  elle yazılmaz; kesinleşmiş kayıtlardan hesaplanır.
- **Her işlem tek parçadır ve veriler yereldir.** Bir kayıt ve onun yan etkileri
  (stok, bakiye) ya birlikte yazılır ya da hiç yazılmaz; veriler internete veya bir
  sunucuya gönderilmez.

---

## 2. Giriş ve Oturum

### 2.1. Giriş ekranı

Program her açıldığında giriş ekranı gelir. Kullanıcı adı ve şifrenizi girip
**Giriş yap** düğmesine tıklayın.

- **"Beni hatırla" yoktur.** Oturum yalnızca bellekte tutulur; program kapanınca
  biter.
- Kullanıcı adı büyük/küçük harf duyarsızdır; şifre duyarlıdır.
- Şifre alanına yazdıklarınız ekranda gizlenir.

### 2.2. Hatalı giriş denemeleri

Aynı kullanıcı adı için **5 hatalı denemeden** sonra giriş **30 saniye** kilitlenir.
Ekranda kalan süre gösterilir. Bu sayaç yalnızca bellekte tutulur ve program
kapatılıp açıldığında sıfırlanır.

### 2.3. Otomatik kilit

Hareketsizlik sonrası otomatik kilit **varsayılan olarak açıktır** ve süre
**15 dakikadır**.

- Süre: **Ayarlar > Genel > Kilit süresi** (5, 15, 30 veya 60 dakika).
- Kilitlendiğinde ekranda **"Oturum kilitlendi"** görünür. Şifrenizi girip
  **Kilidi aç** düğmesine tıklayın.
- Kilit ekranından **Oturumu kapat** ile giriş ekranına dönebilirsiniz.
- Otomatik kilidi tamamen kapatmak için **Ayarlar > Genel**'deki onay kutusunu
  kaldırın (yalnızca yönetici).

### 2.4. Oturumu kapatma ve çıkış

**Dosya > Oturumu kapat** (`Ctrl+Q`) oturumu kapatır, programı kapatmaz;
**Dosya > Çıkış** programdan çıkar. Açık formda **kaydedilmemiş değişiklik**
varsa onay istenir.

---

## 3. Genel Bakış Ekranı

Program açıldığında ilk gösterilen ekrandır. Amaç, işletmenin o anki durumunu tek
bakışta görmektir.

### 3.1. Özet kartı

| Gösterge | Anlamı |
|----------|--------|
| Aktif ürün | Pasif olmayan ürün sayısı |
| Düşük stoklu ürün | Mevcut stoğu minimum stoğun altında olan ürün sayısı |
| Cari hesap | Kayıtlı cari sayısı |
| Kesinleşmiş satış | İptal edilmemiş satış faturalarının KDV dâhil toplamı |
| Kesinleşmiş alış | İptal edilmemiş alış faturalarının KDV dâhil toplamı |
| Bu ay gelir / gider / net | İçinde bulunulan ayın gelir/gider toplamları (iptaller hariç) |
| Toplam kasa | Aktif kasa hesaplarının güncel bakiyeleri |
| Toplam banka | Aktif banka hesaplarının güncel bakiyeleri |
| Toplam nakit | Kasa + banka toplamı |

"Düşük stoklu ürün" satırındaki **Listele** düğmesi, Ürün ve Stok bölümünü
"yalnızca düşük stok" filtresiyle açar.

### 3.2. Diğer kartlar

- **Raporlar** — sık kullanılan raporlara kısayol.
- **Son faturalar** — en yeni 5 fatura (numara, cari, tutar, durum).
- **Son tahsilat/ödemeler** — en yeni 5 hareket.
- **Son stok hareketleri** — en yeni 5 stok hareketi.
- **Modüller** — tüm bölümlere kısayol düğmeleri.
- **Veritabanı bağlantı durumu** — bağlantı durumu, şema sürümü, uygulanan
  migration sayısı, tablo sayısı, dosya boyutu, dosyanın oluşturulma zamanı.
- **Program bilgisi** — sürüm, arayüz teknolojisi, veritabanı teknolojisi ve veri
  dizini.

### 3.3. Yenileme

Ekranın altındaki **Durumu yenile** düğmesi veya `F5`, göstergeleri yeniden
hesaplar. Başka bir bölümde kayıt ekledikten sonra Genel Bakış'a döndüğünüzde
değerler güncellenir.

---

## 4. Cari Hesaplar

Cari hesaplar; müşteri, tedarikçi veya her ikisi olarak tanımlanan kişi/firma
kartlarıdır. Bölüm üç ekrandan oluşur: **Liste**, **Form** ve **Ayrıntı**.

### 4.1. Liste ekranı

Sütunlar: cari kodu, ad/unvan, tür, telefon, vergi dairesi, vergi/TC kimlik
numarası, güncel bakiye ve durum.

- **Ara** kutusu kod, ad, telefon ve vergi numarasında arar.
- **Tür** filtresi: Tümü / Müşteri / Tedarikçi / Her İkisi.
- **Durum** filtresi: Tümü / Aktif / Pasif.
- **Filtreleri temizle** tüm filtreleri sıfırlar.
- Alt bilgide kaç kayıt listelendiği ve bakiyelerin toplamı gösterilir.
- Eylemler: **Yeni cari**, **Ayrıntı**, **Düzenle**, **Pasife al / Aktife al**.
- **CSV dışa aktar**, **CSV içe aktar** (yalnızca yönetici) ve **Şablon indir**
  düğmeleri vardır.

Arama **Türkçe karakter duyarlıdır**: `ı/i`, `ş/s`, `ğ/g`, `ü/u`, `ö/o`, `ç/c`
eşleşir. Örneğin "sirket" yazınca "Şirket" bulunur.

### 4.2. Yeni cari / düzenleme formu

| Alan | Zorunlu | Açıklama |
|------|---------|----------|
| Cari kodu | Evet | Otomatik önerilir (`CAR-0001`); **Yeni kod öner** ile yenilenir, değiştirilebilir |
| Cari türü | Evet | Müşteri / Tedarikçi / Her İkisi |
| Ad veya firma unvanı | Evet | |
| Yetkili kişi | Hayır | |
| Telefon | Hayır | Biçim denetlenir |
| E-posta | Hayır | Geçerli e-posta biçimi denetlenir |
| Adres | Hayır | Çok satırlı |
| Vergi dairesi | Hayır | |
| Vergi numarası | Hayır | 10 veya 11 hane |
| T.C. kimlik numarası | Hayır | 11 hane |
| Notlar | Hayır | Çok satırlı |
| Durum | Hayır | Aktif / Pasif onay kutusu |

- Cari kodu **benzersizdir** (büyük/küçük harf duyarsız). Aynı kod ikinci kez
  kullanılamaz.
- Zorunlu alan boşsa veya biçim hatalıysa alanın altında **kırmızı** açıklama
  görünür. Alanı düzeltmeye başladığınızda hata mesajı silinir.
- Kaydetme başarısız olursa girdikleriniz **kaybolmaz**; form açık kalır.
- **Kaydet** (`Ctrl+S`) veya **İptal** (`Esc`). Değişiklik varsa iptal onay ister.
- Kayıt **silinmez**; **Pasif** yapıldığında listede kalır ama yeni işlemlerde
  seçilemez.

### 4.3. Cari ayrıntısı

- Cari bilgileri ve oluşturulma/güncellenme tarihi.
- **Güncel bakiye** ve hesap özeti.
- O cariye ait **fatura listesi**: numara, tarih, tür, tutar, iptal durumu.
- Kompakt **tahsilat** ve **ödeme** listeleri.
- **Faturaları gör** ve **Ekstresini gör** düğmeleri.
- **Tahsilat Al** ve **Ödeme Yap** düğmeleri; ödeme formunu bu cariyle açar.

### 4.4. Bakiye nasıl hesaplanır?

Cari bakiye elle değiştirilemez. Kesinleşmiş kayıtlardan hesaplanır:

```
bakiye = kesinleşmiş satış faturaları
       − kesinleşmiş alış faturaları
       − kesinleşmiş tahsilatlar
       + kesinleşmiş ödemeler
```

- **Pozitif** bakiye: cari bize borçlu.
- **Negatif** bakiye: biz cariye borçluyuz.
- İptal edilen faturalar ve ödemeler hesaba **katılmaz**.

### 4.5. Pasife alma

Cari kaydı silinmez. **Pasife al** ile durum "Pasif" olur:

- Listede görünmeye devam eder.
- Yeni fatura ve ödeme formlarında seçilemez.
- Geçmiş kayıtları ve bakiyesi korunur.
- **Aktife al** ile yeniden kullanılabilir.

---

## 5. Ürün ve Stok

Bölüm dört ekrandan oluşur: **Liste**, **Form**, **Ayrıntı** ve **Stok hareketi
formu**.

### 5.1. Liste ekranı

Sütunlar: ürün kodu, barkod, ad, birim, alış fiyatı, satış fiyatı, KDV oranı,
güncel stok, minimum stok ve durum.

- **Ara** kutusu kod, barkod, ad ve kategoride arar (Türkçe karakter duyarlı).
- **Durum** filtresi: Tümü / Aktif / Pasif.
- **Yalnızca düşük stoklu ürünler** onay kutusu.
- Mevcut stoğu minimum stoğun altında olan satırlar görsel olarak işaretlenir.
- Eylemler: **Yeni ürün**, **Ayrıntı**, **Düzenle**, **Pasife al / Aktife al**.
- **CSV dışa aktar**, **CSV içe aktar** (yalnızca yönetici), **Şablon indir**.

### 5.2. Yeni ürün / düzenleme formu

| Alan | Zorunlu | Açıklama |
|------|---------|----------|
| Ürün kodu | Evet | Otomatik önerilir (`URN-0001`), değiştirilebilir |
| Barkod | Hayır | Doluysa benzersiz olmalı |
| Ürün adı | Evet | |
| Ölçü birimi | Evet | Adet, Kilogram (kg), Gram (g), Litre, Mililitre (ml), Metre, Metrekare (m²), Paket, Kutu, Koli, Ton |
| Kategori | Hayır | Serbest metin |
| Alış fiyatı (₺) | Hayır | Ondalık ayırıcı virgül (`1.234,56`) |
| Satış fiyatı (₺) | Hayır | |
| KDV oranı (%) | Evet | Hazır oranlardan seçilebilir (ör. %20) |
| Minimum stok | Hayır | Düşük stok uyarısı için eşik |
| Başlangıç stoğu | Hayır | Yalnızca yeni kayıtta; girilirse tek bir "İlk Stok" hareketi oluşur |
| Notlar | Hayır | |
| Durum | Hayır | Aktif / Pasif |

- Ürün kodu ve (doluysa) barkod **benzersizdir**; hem arayüzde hem veritabanında
  denetlenir.
- Negatif fiyat veya miktar kabul edilmez.
- Kayıt sonrası **başlangıç stoğu değiştirilemez**; stok yalnızca hareketlerle
  değişir.
- Ürün kayıtları **silinmez**, pasife alınır.

### 5.3. Ürün ayrıntısı

- Ürün bilgileri, oluşturulma/güncellenme tarihi ve durum.
- **Güncel stok**, minimum stok ve düşük stok uyarısı.
- **Stok hareketleri** listesi (en yeni önce): tarih, tür, miktar, açıklama.
- **Düzenle**, **Yeni hareket** ve **Geri dön** düğmeleri.

### 5.4. Stok hareketleri

Stok hareketi formunda üç tür vardır:

| Tür | Yön | Açıklama |
|-----|-----|----------|
| Giriş | Artı | Mal girişi; yön sistem tarafından belirlenir |
| Çıkış | Eksi | Mal çıkışı; yön sistem tarafından belirlenir |
| Düzeltme | Seçilir | Sayım farkı vb.; yönü kullanıcı seçer, **açıklama zorunludur** |

- Kaydetmeden önce **"Yeni stok"** önizlemesi gösterilir.
- Stok **eksiye düşecekse** onay istenir: "Stok eksiye düşecek … Devam edilsin mi?"
  Onaylarsanız kayıt yapılır, stok negatif görünür. Vazgeçerseniz hiçbir şey
  kaydedilmez.
- **Hareketler silinemez veya düzenlenemez.** Düzeltmek için ters yönlü yeni bir
  **Düzeltme** hareketi girin; böylece iz kaybolmaz.
- Tarih alanı için **Bugün** düğmesi vardır.

### 5.5. Faturaların stoğa etkisi

- **Alış faturası** kesinleşince her satır için **artı** yönlü stok hareketi
  (`alis_faturasi`) oluşur.
- **Satış faturası** kesinleşince her satır için **eksi** yönlü stok hareketi
  (`satis_faturasi`) oluşur.
- **Fatura iptali** her satır için ters yönlü `duzeltme` hareketi üretir.
- Bu hareketler Ürün ve Stok bölümündeki hareket listesinde görünür.

---

## 6. Alış ve Satış Faturaları

Alış ve satış faturaları aynı ekranı kullanır; sol menüde iki ayrı bölüm olarak
açılır. Fatura ekranı **Liste**, **Form (yalnızca yeni)** ve **Ayrıntı** olmak
üzere üç görünümden oluşur.

### 6.1. Fatura listesi

Sütunlar: numara, tarih, cari, matrah, KDV, genel toplam, kalan ve durum.

**Ödeme rozeti (Kalan sütunu):**

| Durum | Anlamı |
|-------|--------|
| Ödendi (yeşil) | Kalan tutar 0 |
| Kısmi (sarı) | Bir kısmı ödenmiş |
| Ödenmemiş (kırmızı) | Hiç ödeme yok |
| — | Fatura iptal edilmiş |

Filtreler:

- **Ara**: fatura numarası veya cari adı.
- **Cari**: belirli bir cariye ait faturalar.
- **Durum**: Tümü / Kesin / İptal.
- **Tarih**: başlangıç ve bitiş (`YYYY-AA-GG`).
- **Filtreleri temizle**.

Alt bilgide "X fatura, genel toplam: Y" yazar; **iptal edilen faturalar toplama
katılmaz**. Eylemler: **Yeni fatura**, **Ayrıntı**, **İptal Et** (yalnızca kesin
faturalarda).

### 6.2. Yeni fatura formu

| Bölüm | Alanlar |
|-------|---------|
| Fatura bilgileri | Fatura türü (Alış/Satış), fatura numarası, fatura tarihi, vade tarihi (isteğe bağlı), cari hesap, notlar |
| Fatura satırları | Ürün, miktar, birim fiyat, iskonto %, KDV %, satır net (salt okunur), satır açıklaması |
| Toplamlar | Brüt toplam, iskonto, matrah, KDV, genel toplam |

- **Fatura türü yalnızca oluştururken seçilir.** Kesinleştikten sonra değiştirilemez.
- Fatura numarası seri ve yıla göre önerilir: satışta `SF-YYYY-NNNN`, alışta
  `AF-YYYY-NNNN`. **Yeni numara öner** ile yenilenir; numara değiştirilebilir.
- Fatura tarihi varsayılan olarak bugündür. Vade tarihi **yalnızca bilgi amaçlıdır**;
  gecikme faizi veya otomatik hatırlatma yapmaz.
- Cari listesi aranabilir. Cari türü beklenen türle uyuşmazsa (ör. satış faturasında
  tedarikçi seçilirse) **uyarı** gösterilir ama kayıt **engellenmez**.
- Ürün seçildiğinde kodu, birimi ve varsayılan fiyatı otomatik gelir.
- **Satır ekle** ile yeni satır; **Satırı sil** ile satır kaldırılır. Aynı ürün
  birden fazla satırda kullanılabilir.
- Toplamlar yazdıkça **canlı** hesaplanır. Satır neti KDV hariçtir.

**Kaydetme onayı:** Kaydet düğmesine bastığınızda "Fatura kesinleşecek. Stok ve
cari bakiyesi güncellenecek. Onaylıyor musunuz?" sorusu gelir. Onaylarsanız fatura,
satırlar, stok hareketleri ve cari bakiye etkisi **tek işlemde** yazılır.

**Negatif stok onayı:** Satış faturası bir ürünün stoğunu eksiye düşürecekse ayrıca
"Bazı ürünlerin stoğu eksiye düşecek" listesi ve onay istenir. Onaylarsanız fatura
kaydedilir ve stok negatif görünür.

### 6.3. Fatura ayrıntısı

- Fatura üst bilgisi (numara, tarih, tür, cari, vade, notlar).
- Satırlar ve toplamlar.
- **Ödenen** ve **kalan** tutar ile ödeme durumu rozeti.
- Faturaya bağlı **ödeme kayıtları** listesi. Yoksa "Bu faturaya henüz ödeme kaydı
  yok." yazar.
- **Tahsilat Al** (satış faturası) veya **Ödeme Yap** (alış faturası) düğmesi.
  Bu düğme ödeme formunu, faturanın kalan tutarı kadar **ön-dağıtılmış** olarak
  açar; kayıttan sonra fatura ayrıntısına döner.

### 6.4. Fatura iptali

- Faturalar **silinmez**, **iptal edilir**.
- İptal için **neden zorunludur**.
- İptal, her satır için ters yönlü bir `duzeltme` stok hareketi oluşturur ve cari
  bakiyesini geri alır.
- İptal edilen fatura listede kalır ve "İptal" durumuyla görünür.

**Ödenmiş fatura iptal edilemez.** Faturaya bağlı tahsilat/ödeme varsa şu mesaj
döner: *"Bu faturaya bağlı tahsilat/ödeme bulunuyor. Önce ilgili ödemeleri iptal
edin."* Önce ilgili tahsilat/ödeme kayıtlarını iptal edin, sonra faturayı iptal
edin.

### 6.5. Düzeltme akışı

Kesinleşmiş bir faturada hata varsa faturayı **iptal** edin (neden yazın) ve doğru
bilgilerle **yeni fatura** oluşturun. Fatura düzenlenemediği için bu iki adım
gereklidir; böylece hangi kaydın neden değiştiği izlenebilir kalır.

---

## 7. Kasa ve Banka

Bölüm iki sekmeden oluşur: **Hesaplar** (varsayılan) ve **Hareketler**.

### 7.1. Hesaplar sekmesi

Sütunlar: hesap adı, tür (Kasa/Banka), banka adı, IBAN (kısaltılmış), açılış
bakiyesi, güncel bakiye ve durum.

- Üstte **Toplam kasa**, **Toplam banka** ve **Genel toplam** gösterilir. Toplamlar
  yalnızca **aktif** hesapları kapsar.
- **Ara** kutusu hesap adı, banka adı ve IBAN'da arar.
- **Tür** ve **Durum** filtreleri.
- Eylemler: **Yeni hesap**, **Düzenle**, **Pasife al / Aktife al**.
- **CSV dışa aktar** düğmesi vardır.

**Hesap formu:**

| Alan | Zorunlu | Açıklama |
|------|---------|----------|
| Hesap adı | Evet | Benzersiz (büyük/küçük harf duyarsız) |
| Hesap türü | Evet | Kasa / Banka |
| Banka adı | Hayır | Yalnızca banka hesabında gösterilir |
| IBAN | Hayır | Yalnızca banka hesabında; `TR` ile başlayan 26 karakter |
| Açılış bakiyesi (₺) | Hayır | Yalnızca yeni hesapta veya hiç hareketi olmayan hesapta değiştirilebilir |
| Notlar | Hayır | |
| Durum | Hayır | Aktif / Pasif |

- **Açılış bakiyesi**, hesapta hareket varsa **salt okunur** olur; nedeni ekranda
  açıklanır.
- Güncel bakiye = açılış bakiyesi + tahsilatlar + gelirler − ödemeler − giderler.
- **Kasa bakiyesi eksiye düşebilir**; program engellemez.

### 7.2. Hareketler sekmesi

Alt sekmeler: **Tümü**, **Tahsilatlar**, **Ödemeler**, **Gelirler**, **Giderler**.

Sütunlar: tarih, tür, cari (varsa), kasa/banka, tutar ve durum (Kesin/İptal).

Filtreler:

- **Ara**: açıklama veya cari adı.
- **Tarih** aralığı.
- **Cari**, **Hesap** ve **Tür**.
- **Durum**.

Alt bilgide (iptaller hariç) toplam tahsilat, toplam ödeme ve net gösterilir.
Eylemler: **Ayrıntı** ve **İptal et** (yalnızca kesin kayıtlarda). **Yeni Tahsilat**
ve **Yeni Ödeme** düğmeleri her zaman görünür. **Tahsilat/ödemeleri CSV** düğmesi
listeyi CSV olarak aktarır.

### 7.3. Yeni tahsilat / yeni ödeme

| Alan | Açıklama |
|------|----------|
| Tür | Tahsilat veya Ödeme (yalnızca yeni kayıtta değişir) |
| Cari | Aranabilir liste; tahsilatta müşteriler, ödemede tedarikçiler önce gelir |
| Kasa/banka | Yalnızca **aktif** hesaplar |
| Tarih | Varsayılan bugün |
| Açıklama | İsteğe bağlı |
| Fatura dağıtımı | Seçili carinin uygun faturaları |

**Fatura dağıtımı nasıl çalışır?**

- Tahsilatta yalnızca **satış** faturaları, ödemede yalnızca **alış** faturaları
  listelenir.
- Yalnızca **aynı cariye** ait, **kesin** ve **kalanı > 0** faturalar listelenir.
- Her satıra **dağıtılacak tutar** elle girilir; **Tümünü dağıt** kalanların
  tamamını doldurur.
- Özette **ödeme tutarı**, **dağıtılan toplam** ve **kalan (avans)** gösterilir.
- **Dağıtılan > tutar** ise kırmızı uyarı çıkar ve kayıt engellenir.
- **Dağıtılan < tutar** ise fark **avans** olarak kaydedilir.
- Fatura listesi boşsa tüm tutar avans olarak kaydedilebilir.
- Bir faturaya **birden çok** tahsilat/ödeme dağıtılabilir (kısmi ödeme).

Cari türü beklenen türle uyuşmazsa uyarı gösterilir ama kayıt engellenmez.

Kaydetme onayı: "Tahsilat/Ödeme kesinleşecek. Kasa bakiyesi ve fatura ödemeleri
güncellenecek." Onaylarsanız kayıt, dağıtımlar, faturaların ödenen tutarı ve kasa
bakiyesi **tek işlemde** yazılır.

### 7.4. Tahsilat/ödeme ayrıntısı ve iptal

Ayrıntı ekranında tür, tarih, cari, kasa/banka, tutar, dağıtılan, avans, açıklama
ve durum gösterilir. Fatura dağıtımları listelenir. İptal edilmişse iptal zamanı ve
nedeni görünür.

- **İptal et** için neden zorunludur.
- İptal, kasa bakiyesini ve faturaların ödenen tutarını **geri alır**; kayıt
  `iptal` durumuna geçer.
- Kayıtlar silinmez; düzeltme = **iptal + yeni kayıt**.

---

## 8. Gelir ve Gider

Fatura dışı gelir ve giderleri (kira, personel, elektrik, kargo vb.) kaydetmek için
kullanılır. Üç sekme vardır: **Hareketler**, **Kategoriler**, **Özet**.

### 8.1. Hareketler sekmesi

Sütunlar: tarih, tür (Gelir/Gider), kategori, kasa/banka, tutar, belge no, açıklama
ve durum.

- **Yeni Gelir** ve **Yeni Gider** düğmeleri.
- **Ara**: açıklama, kategori veya belge no.
- **Tür**, **Kategori**, **Hesap** ve **Tarih** filtreleri.
- Üstte **Özet (iptaller hariç)**: toplam gelir, toplam gider ve net.
- **CSV dışa aktar** düğmesi.

**Hareket formu:**

| Alan | Zorunlu | Açıklama |
|------|---------|----------|
| Tür | Evet | Gelir / Gider |
| Kategori | Evet | Yalnızca türle uyumlu **aktif** kategoriler |
| Kasa/banka hesabı | Evet | Yalnızca **aktif** hesaplar |
| Tarih | Evet | Varsayılan bugün |
| Tutar | Evet | Sıfırdan büyük |
| Belge/fiş no | Hayır | |
| Açıklama | Hayır | |

Formun altında **Kasa/banka etkisi** kartı **"Yeni kasa/banka bakiyesi: X"**
önizlemesini gösterir. Kaydedince kasa bakiyesi gelirde artar, giderde azalır.

### 8.2. Kategoriler sekmesi

Kategoriler gelir/gider hareketlerini gruplamak için kullanılır.

- Sütunlar: ad, tür, açıklama, durum.
- **Yeni kategori**, **Düzenle**, **Pasife al / Aktife al**.
- Kategori adı aynı türde **benzersizdir**.
- **Pasif kategoriye yeni hareket eklenemez**; geçmiş hareketler korunur.
- Kategori türü (Gelir/Gider) ile hareket türü uyuşmalıdır.

Kurulum sihirbazındaki hazır kategori paketleri: Satış geliri, Hizmet geliri,
Diğer gelirler, Kira, Personel, Elektrik, Su, Doğalgaz, İnternet / telefon, Kargo,
Vergi ve harçlar, Bakım ve onarım, Ofis malzemeleri, Diğer giderler.

### 8.3. Özet sekmesi

- **Dönem toplamları (iptaller hariç)**: toplam gelir, toplam gider, net.
- **Kategori dağılımı**: kategori bazında tutar ve yüzde.
- **Dönem** filtresi: başlangıç ve bitiş tarihi.
- Altı aylık gelir/gider **trend** tablosu.

### 8.4. İptal

- Gelir/gider kaydı **silinmez**, **neden zorunlu** olarak iptal edilir.
- İptal, kasa/banka bakiyesini geri alır; kayıt "İptal" durumuna geçer ve toplamlara
  ile raporlara **katılmaz**.

---

## 9. Raporlar

Raporlar bölümü **salt okunurdur**; hiçbir kaydı değiştirmez. Altı rapor sekmesi
vardır ve her raporda **CSV dışa aktar** düğmesi bulunur.

### 9.1. Cari Ekstre

- Bir cari seçin ve tarih aralığı girin.
- Tarih, tür, belge no, açıklama, borç, alacak ve **yürüyen bakiye** sütunları.
- Alt toplam: **dönem başı bakiye**, **dönem borç toplamı**, **dönem alacak
  toplamı**, **dönem sonu bakiye**.
- Cari seçilmezse "Ekstre için bir cari hesap seçin." bilgisi gösterilir.

### 9.2. Cari Bakiye Özeti

- Tüm carilerin satış, alış, tahsilat, ödeme ve bakiye toplamları.
- **Ara** (kod veya ad), **Tür**, **Durum** ve **Yalnızca sıfır olmayan bakiyeler**
  filtreleri.
- Sütun başlıklarına tıklayarak **sıralama**.
- Alt toplam: alacaklı cariler (bize borçlu), borçlu cariler (bizim borcumuz), net
  bakiye, cari sayısı.

### 9.3. Stok Durumu

- Ürün kodu, ad, birim, mevcut stok, minimum stok ve durum.
- Durum: **Yeterli**, **Düşük**, **Sıfır**, **Eksi**.
- **Ara**, **Durum** filtresi ve **Yalnızca aktif ürünler** onay kutusu.
- Alt toplam: toplam aktif ürün, düşük stoklu ürün sayısı.

### 9.4. Satış/Alış Özeti

- Dönem içindeki **kesinleşmiş** satış ve alış faturalarının özeti.
- **Gruplama**: **Cari** veya **Ürün**.
- Tarih aralığı filtresi.
- Alt toplam: satış adedi/tutarı, alış adedi/tutarı.

### 9.5. Kasa/Banka Hareket Özeti

- **Hesap** filtresi (Tüm hesaplar veya tek hesap) ve tarih aralığı.
- **Dönem başı bakiye**, hareketler, **dönem sonu bakiye**.
- Hesap dökümü: tahsilat, ödeme, gelir, gider toplamları.

### 9.6. Gelir/Gider Özeti

- Dönem toplamları (iptaller hariç).
- Kategori dağılımı (tutar ve yüzde).
- Son **altı aylık** gelir/gider trendi.

### 9.7. Rapor filtrelerini sıfırlama

**Sıfırla** düğmesi aktif raporu varsayılana döndürür:

- Cari ekstre/bakiye: boş tarih aralığı.
- Dönem raporları: içinde bulunulan ay.
- Stok: yalnızca aktif ürünler.

Tarih alanları `YYYY-AA-GG` biçimindedir (ör. `2026-10-27`). Boş bırakılan tarih
sınırı "sınırsız" anlamına gelir.

---

## 10. Ayarlar

Ayarlar bölümünde dört sekme vardır. **Operatör** rolündeki kullanıcı yalnızca
**Güvenlik** sekmesini görür.

### 10.1. Genel sekmesi

| Ayar | Seçenekler / varsayılan |
|------|--------------------------|
| Dil | Türkçe (sabit) |
| Hareketsizlik sonrası otomatik kilit | Açık / Kapalı (varsayılan: açık) |
| Kilit süresi | 5 / 15 / 30 / 60 dakika (varsayılan: 15) |
| Log seviyesi | Warn (varsayılan) / Info (ayrıntılı) |

- Değişiklikten sonra **Ayarları kaydet** düğmesine basın.
- **Şirket ve kurulum** kartında şirket unvanı, vergi dairesi ve vergi numarası
  görünür.
- **Kurulum sihirbazını yeniden aç** (yalnızca yönetici) sihirbazı yeniden başlatır.

### 10.2. Güvenlik sekmesi

- **Şifre değiştir**: mevcut şifre, yeni şifre, yeni şifre tekrarı.
- Yeni şifre kuralları ilk kurulumdakiyle aynıdır: en az 8 karakter, en az bir harf
  ve bir rakam.
- **Otomatik kilit** kartı durumu ve süreyi gösterir; süre Genel sekmesinden
  değiştirilir.
- Bu sekme tüm roller için açıktır; herkes **yalnızca kendi** şifresini değiştirir.

### 10.3. Yedekleme sekmesi

- **Son otomatik yedek** zamanı ve **toplam yedek boyutu**.
- **Şimdi yedek al** düğmesi.
- **Rotasyon (otomatik yedekler)**: günlük varsayılan **30**, aylık **12**, yıllık
  **5**; **Rotasyon ayarlarını kaydet** ile uygulanır.
- **Manuel yedekler rotasyona dâhil değildir.**
- **Son yedekler** listesi: dosya adı, tür (otomatik/manuel), tarih, boyut.
- **Bu yedeğe dön** düğmesi seçili yedeği geri yükler (yalnızca yönetici).
- **Yedek boyutu uyarısı** eşiği (varsayılan 1000 MB).

### 10.4. Kullanıcılar sekmesi (yalnızca yönetici)

- Kullanıcı listesi: kullanıcı adı, görünen ad, rol, durum, son giriş.
- **Yeni kullanıcı**: kullanıcı adı, görünen ad, rol (Yönetici/Operatör), şifre.
- Her satırdaki **İşlemler** menüsünde:
  - **Düzenle**: görünen ad, rol ve aktiflik durumu.
  - **Pasife al / Aktifleştir**.
  - **Şifre sıfırla**: yönetici, kullanıcıya yeni bir şifre belirler.
  - **Sil**: kullanıcı hesabını kalıcı olarak siler; onay ister.
- Kurallar:
  - **Son aktif yönetici** silinemez, pasife alınamaz veya rolü düşürülemez.
  - **Kendi hesabınız** için pasife alma, şifre sıfırlama ve silme seçenekleri
    gösterilmez; kendi rolünüzü de değiştiremezsiniz.
  - Kullanıcı adı benzersizdir (büyük/küçük harf duyarsız).

---

## 11. Yedekleme ve Geri Yükleme

### 11.1. Otomatik yedek

- Program her açılışta, **son otomatik yedekten 24 saat geçtiyse** yeni bir yedek
  alır.
- Dosya adı: `onmuhasebe-auto-YYYYAAGG-SSDDss.onm-backup`.
- Yedek alınırken ekranda "Otomatik yedek alınıyor..." bilgisi görünür.
- Yedek alınamazsa uyarı gösterilir: "Otomatik yedek alınamadı. Ayarlar >
  Yedekleme'den manuel deneyin."
- Yedek oluşturulduktan sonra **doğrulanır**; bozuksa dosya silinir ve hata
  bildirilir.
- Rotasyon yalnızca **otomatik** yedeklere uygulanır.

### 11.2. Manuel yedek

**Ayarlar > Yedekleme > Şimdi yedek al**. Dosya adı
`onmuhasebe-manual-YYYYAAGG-SSDDss.onm-backup` biçimindedir ve rotasyondan
etkilenmez.

### 11.3. Dışa aktarma

**Dosya > Tüm veriyi dışa aktar...** menüsü, veritabanının tam kopyasını
`.onm-backup` dosyası olarak kaydetmenizi sağlar. Dosya adı önerisi
`OnMuhasebe-yedek-<zaman>.onm-backup` şeklindedir.

Bu dosya:

- Başka bir bilgisayara taşınabilir.
- Harici disk, USB bellek veya ağ konumuna kopyalanabilir.
- Program içinden **geri yüklenebilir**.

### 11.4. Geri yükleme

**Dosya > Yedeği geri yükle...** (yalnızca yönetici):

1. `.onm-backup` dosyasını seçin.
2. Ekranda yedeğin şema sürümü ve tarihi gösterilir.
3. **Yedeği geri yükle** düğmesine tıklayın.
4. Onaylayın.
5. Mevcut veritabanı önce **güvenlik yedeğine** alınır
   (`on_muhasebe.db.pre-restore-<zaman>`).
6. Yeni dosya geçici kopyaya yazılır, doğrulanır ve ancak ondan sonra asıl dosyanın
   yerine geçer.
7. Program veritabanını yeniden açar.

**Güvenlik kuralları:**

- Yalnızca `.onm-backup` uzantılı dosyalar kabul edilir.
- Programın **daha yeni** sürümüyle oluşturulmuş yedek reddedilir: "Bu yedek,
  programın daha yeni bir sürümüyle oluşturulmuş. Önce programı güncelleyin."
- OnMuhasebe yedeği olmayan dosya reddedilir: "Bu dosya bir OnMuhasebe yedeği
  değil."
- Geri yükleme sırasında hata olursa güvenlik yedeği geri kopyalanır.

### 11.5. Yarıda kalan geri yükleme

Geri yükleme sırasında program kapanırsa (elektrik kesintisi, çökme), bir sonraki
açılışta **"Yarıda kalan geri yükleme"** ekranı gelir. Şu bilgiler gösterilir:
başlangıç zamanı, kalan adım, kaynak yedek, güvenlik yedeği ve hedef veritabanı.

Seçenekler:

| Seçenek | Sonuç |
|---------|-------|
| **Güvenlik yedeğine dön** | Geri yükleme öncesi duruma döner (önerilen) |
| **Mevcut veritabanıyla devam et** | O anki dosyayla devam eder |
| **Uygulamayı kapat** | Değişiklik yapmadan çıkar |

### 11.6. Yedekleri başka bilgisayara taşıma

Eski bilgisayarda **Dosya > Tüm veriyi dışa aktar...** ile `.onm-backup` dosyasını
alın; USB bellek veya ağ üzerinden yeni bilgisayara kopyalayın. Yeni bilgisayarda
programı kurup ilk yönetici hesabını oluşturun, ardından **Dosya > Yedeği geri
yükle...** ile dosyayı seçin.

> Yedeği **başka bir diske** de kopyalayın. Aynı diskteki iki kopya, disk arızasına
> karşı korumaz (3-2-1 kuralı).

---

## 12. CSV İçe ve Dışa Aktarma

### 12.1. Dışa aktarma

Aşağıdaki ekranlarda CSV aktarma düğmesi vardır:

- Cari hesaplar listesi — **CSV dışa aktar**
- Ürünler listesi — **CSV dışa aktar**
- Fatura listesi — **CSV dışa aktar**
- Tahsilat/ödemeler listesi — **Tahsilat/ödemeleri CSV**
- Kasa/banka hesapları listesi — **CSV dışa aktar**
- Gelir/gider hareketleri listesi — **CSV dışa aktar**
- Altı raporun her biri — **CSV dışa aktar**

**Biçim:**

- Ayırıcı: **noktalı virgül** (`;`)
- Kodlama: **UTF-8 BOM** (Excel'in Türkçe karakterleri doğru açması için)
- Ondalık ayırıcı: **virgül** (`,`) — Türkçe Excel uyumlu
- Tarih: `YYYY-AA-GG`

Dosyayı Excel'de açmak için çift tıklamanız yeterlidir. Excel sorarsa **UTF-8**
kodlamasını seçin.

### 12.2. İçe aktarma

İçe aktarma **yalnızca cari hesaplar ve ürünler** için vardır ve **yalnızca
yönetici** yapabilir.

1. İlgili listede **Şablon indir** ile `cari_sablon.csv` veya `urun_sablon.csv`
   dosyasını alın.
2. Şablonu Excel'de doldurun, CSV olarak kaydedin.
3. **CSV içe aktar** ile dosyayı seçin; **Önizleme** her satırı **Hazır** veya
   **Hata: <neden>** olarak gösterir.
4. Onaylarsanız **tüm geçerli satırlar tek işlemde** eklenir; bir hata olursa
   *"Hiçbir kayıt eklenmedi."* mesajı gelir.
5. Sonuç penceresinde eklenen ve atlanan satırlar nedenleriyle listelenir.

**Cari şablonu sütunları:**
`code`, `name`, `kind`, `phone`, `email`, `address`, `tax_office`, `tax_number`,
`national_id`, `contact_person`, `notes`, `active`

**Ürün şablonu sütunları:**
`code`, `barcode`, `name`, `unit`, `category`, `purchase_price`, `sale_price`,
`vat_rate`, `min_stock`, `notes`, `active`

Notlar:

- Sütun başlıkları **aynen** korunmalıdır.
- `kind` değeri `musteri`, `tedarikci` veya `ikisi` olabilir.
- `unit` değeri birim kısa adıdır (`adet`, `kg`, `g`, `litre`, `ml`, `metre`,
  `m2`, `paket`, `kutu`, `koli`, `ton`).
- `active` alanı `1`/`0` veya `true`/`false` olarak yazılabilir.
- Var olan kodlar atlanır (benzersizlik kuralı).
- İçe aktarma **fatura, ödeme veya stok hareketi** oluşturmaz.

### 12.3. CSV ile yedekleme arasındaki fark

| CSV | `.onm-backup` |
|-----|----------------|
| Tek bir liste/rapor aktarır | Tüm veritabanını taşır |
| Yeniden içe aktarılabilir (cari/ürün) | Yalnızca geri yükleme ile açılır |
| Excel'de düzenlenebilir | Düzenlenmemelidir |
| Yedek yerine geçmez | Gerçek yedektir |

---

## 13. Klavye Kısayolları

| Kısayol | İşlev |
|---------|-------|
| `Ctrl+N` | Yeni kayıt (aktif bölüme göre) |
| `Ctrl+S` | Aktif formu kaydet |
| `Ctrl+K` | Genel arama |
| `Ctrl+F` | Genel arama |
| `F5` | Aktif listeyi / durumu yenile |
| `Esc` | Formu veya açılır pencereyi kapat |
| `Ctrl+,` | Ayarlar |
| `Ctrl+Q` | Oturumu kapat (onay ister) |
| `Ctrl+1` | Genel Bakış |
| `Ctrl+2` | Cari Hesaplar |
| `Ctrl+3` | Ürün ve Stok |
| `Ctrl+4` | Alış Faturaları |
| `Ctrl+5` | Satış Faturaları |
| `Ctrl+6` | Kasa ve Banka |
| `Ctrl+7` | Gelir ve Gider |
| `Ctrl+8` | Raporlar |
| `Ctrl+9` | Ayarlar |

Kısayolların tam listesini program içinden **Yardım > Klavye kısayolları** ile de
görebilirsiniz.

Notlar:

- `Ctrl+S`, açık bir form yoksa "Kaydedilecek değişiklik yok." bilgisi verir.
- `Esc`, kaydedilmemiş değişiklik varsa onay ister.
- Metin alanında yazarken `Ctrl+N`, `Ctrl+S` gibi kısayollar çalışmaya devam eder;
  `Esc` önce açılır pencereyi kapatır.

---

## 14. Genel Arama

`Ctrl+K` veya `Ctrl+F` ile **genel arama** penceresi açılır.

- **Cari hesaplar**, **ürünler**, **faturalar** ve **kasa/banka hesapları** içinde
  arar.
- Sonuçlar türe göre **gruplanır**.
- Her grupta en fazla **5**, toplamda en fazla **20** sonuç gösterilir.
- Arama **Türkçe karakter duyarlıdır** (`ı/i`, `ş/s`, `ğ/g`, `ü/u`, `ö/o`, `ç/c`).
- Bir sonuca tıklamak ilgili bölümü açar ve kaydı seçer.
- `Esc` aramayı kapatır.

Arama; muhasebe kayıtlarını değiştirmez, yalnızca bulur.

---

## 15. Kullanıcı Rolleri

İki rol vardır:

| Yetki | Yönetici | Operatör |
|-------|:--------:|:--------:|
| Görüntüleme ve raporlar | ✔ | ✔ |
| Yeni cari/ürün/fatura/ödeme/gelir-gider ekleme | ✔ | ✔ |
| Kayıt düzenleme (fatura hariç) | ✔ | ✔ |
| Fatura / ödeme / gelir-gider **iptali** | ✔ | ✘ |
| Cari/ürün/hesap/kategori **pasife alma** | ✔ | ✘ |
| CSV **içe** aktarma | ✔ | ✘ |
| CSV dışa aktarma | ✔ | ✔ |
| Yedek **alma** | ✔ | ✔ |
| Yedeği **geri yükleme** | ✔ | ✘ |
| Kullanıcı yönetimi | ✔ | ✘ |
| Ayarlar > Genel / Yedekleme sekmeleri | ✔ | ✘ |
| Ayarlar > Güvenlik (kendi şifresi) | ✔ | ✔ |
| Kurulum sihirbazını yeniden açma | ✔ | ✘ |

Yetkisi olmayan bir işlem denenirse şu mesaj döner: *"Bu işlem için yönetici
yetkisi gerekir: <işlem>."* Düğmeler operatörde görünmez veya devre dışıdır.

### 15.1. Yeni kullanıcı ekleme

1. **Ayarlar > Kullanıcılar** (yönetici) > **Yeni kullanıcı**.
2. Kullanıcı adı, görünen ad, rol ve şifre girip kaydedin.

Kullanıcı adı benzersizdir. Şifre kuralları ilk kurulumdakiyle aynıdır.

### 15.2. Aynı bilgisayarda birden çok kullanıcı

Program aynı bilgisayarda birden çok kullanıcı hesabıyla kullanılabilir; ancak
**aynı anda yalnızca bir oturum** açıktır ve program ağ üzerinden paylaşılamaz.

---

## 16. İpuçları ve Püf Noktaları

- **Kırmızı alan hatası:** Hatalı alanın altında açıklama görünür. Yazmaya
  başladığınızda hata silinir; kaydetmeden önce tüm alanları düzeltin.
- **Girdi kaybolmaz:** Kaydetme başarısız olursa formdaki bilgiler korunur.
- **Kaydedilmemiş değişiklik:** Formdan çıkarken veya oturumu kapatırken onay
  istenir.
- **Başarı bildirimleri** 5 saniye sonra kendiliğinden kapanır.
- **Hata ayrıntıları:** **Yardım > Hata ayrıntıları** veya hata kartındaki
  **Bilgileri kopyala** ile metni kopyalayıp destek için gönderebilirsiniz.
- **Tarih biçimi:** Tüm tarih alanları `YYYY-AA-GG` (ör. `2026-10-27`).
- **Tutar biçimi:** Binlik ayırıcı nokta, ondalık ayırıcı virgül (`1.234,56`).
  Yalnızca rakam da yazabilirsiniz (`1234,56`).
- **Miktar biçimi:** Ondalık kesirler virgülle (`1,5` kg).
- **Ödeme ön-dağıtımı:** Fatura ayrıntısındaki **Tahsilat Al** / **Ödeme Yap**
  düğmesi, kalan tutarı otomatik doldurur; elle dağıtmak zorunda kalmazsınız.
- **Kısmi ödeme ve avans:** Aynı faturaya birden çok tahsilat girebilirsiniz;
  faturaya dağıtılmayan tutar avans olarak kaydedilir.
- **Pasif kayıt:** Silmek yerine pasife alın. Geçmiş kayıtlar bozulmaz.

---

## 17. Veri Konumları ve Güvenlik

### 17.1. Dosya konumları

| İçerik | Yol |
|--------|-----|
| Veritabanı | `%LOCALAPPDATA%\OnMuhasebe\OnMuhasebe\data\on_muhasebe.db` |
| Yedekler | `%LOCALAPPDATA%\OnMuhasebe\OnMuhasebe\backups\` |
| Günlükler | `%LOCALAPPDATA%\OnMuhasebe\OnMuhasebe\logs\` |

- `on_muhasebe.db-wal` ve `on_muhasebe.db-shm` dosyaları SQLite'ın çalışma
  dosyalarıdır. Program açıkken **silmeyin**, kopyalamayın.
- Veritabanını kopyalarken programı kapatın veya **Dosya > Tüm veriyi dışa
  aktar...** kullanın.

### 17.2. Güvenlik

- Veriler **yereldir**; internete gönderilmez.
- Veritabanı **şifrelenmemiştir**; bilgisayara erişen biri dosyayı okuyabilir.
  Windows oturum şifresi kullanın; hassas veriler için **BitLocker** önerilir.
- Kullanıcı şifreleri düz metin saklanmaz; Argon2id ile özetlenir.
- Yedekleri **başka bir diske** kopyalayın.
- Program **imzasızdır**; yalnızca güvendiğiniz kaynaktan indirin
  (<https://github.com/furcanomer/OnMuhasebe>).

### 17.3. Günlükler

- Günlük dosyaları `logs` klasöründe tutulur; tek dosya üst sınırı **10 MB**,
  **30 günden eski** dosyalar otomatik silinir.
- Ayrıntı düzeyi **Ayarlar > Genel > Log seviyesi** ile değiştirilir.

---

## 18. Lisans ve Katkı

- **Lisans:** BSD 3-Clause
- **Telif hakkı:** (c) 2026, @furcanomer
- **Kaynak kodu:** <https://github.com/furcanomer/OnMuhasebe>

Lisans metni program klasöründeki `LICENSE` dosyasındadır. BSD 3-Clause, kaynak
kodun kullanılmasına, değiştirilmesine ve dağıtılmasına izin verir; telif bildirimi
ve lisans metni korunmalıdır.

### 18.1. Hata bildirimi

Günlük dosyasını **Yardım > Log klasörünü aç** ile bulun, **Yardım > Hata
ayrıntıları > Bilgileri kopyala** ile metni kopyalayın ve GitHub **Issues**
bölümünde program/Windows sürümüyle birlikte paylaşın. Gerçek müşteri adları,
vergi numaraları veya tutarlar gibi hassas bilgileri paylaşmayın.

### 18.2. Bu sürümde olmayan özellikler

Aşağıdakiler bu sürümde **yoktur**; ileride eklenmesi planlanan ayrı çalışmalardır:

- PDF / Excel (XLSX) çıktısı ve fatura yazdırma — bu sürümde yoktur
- e-Fatura, e-Arşiv, e-Defter entegrasyonu — bu sürümde yoktur
- Çek/senet, kredi kartı, döviz ve virman — bu sürümde yoktur
- Otomatik güncelleme ve bulut yedekleme — bu sürümde yoktur
- Ağ üzerinden çok kullanıcılı çalışma — bu sürümde yoktur

---

Son güncelleme: 2026-10-27
