# Ön Muhasebe

> **Yapay Zeka Beyanı:** Bu projenin kaynak kodu, belgeleri ve tanıtım metinleri, [@furcanomer](https://github.com/furcanomer)'in yönlendirmesi ve denetimi altında yapay zeka (LLM) desteğiyle geliştirilmiştir. Kod, testler ve mimari kararlar insan tarafından gözden geçirilmiş ve onaylanmıştır. Hatalar veya eksikler olabilir; katkılar ve düzeltmeler memnuniyetle karşılanır.

Windows üzerinde **yerel (native) masaüstü uygulaması** olarak çalışan ön muhasebe
programı. Arayüz de iş mantığı da Rust ile yazılmıştır; tarayıcı, web sunucusu,
Tauri veya Electron kullanılmaz. Program internet bağlantısı olmadan çalışır.

> **Durum:** Bu sürüm günlük kullanıma hazırdır. Cari hesaplar, ürün ve stok,
> alış/satış faturaları, kasa/banka ile tahsilat/ödeme, gelir/gider kayıtları,
> altı rapor, kullanıcı ve rol yönetimi, otomatik/manuel yedekleme ve CSV
> içe/dışa aktarma işlevlerinin tamamı çalışır durumdadır. Veriler yerel SQLite
> veritabanında tutulur; program çevrimdışı çalışır.

## Belgeler

Son kullanıcı belgeleri `docs/` klasöründedir:

- [Kurulum Rehberi](docs/KURULUM.md) — MSI kurulumu, ilk açılış, güncelleme, kaldırma
- [Kullanım Kılavuzu](docs/KULLANIM_KILAVUZU.md) — tüm ekranlar ve iş akışları
- [Sık Sorulan Sorular](docs/SSS.md) — kısa soru-cevap
- [Sorun Giderme](docs/SORUN_GIDERME.md) — belirti / neden / çözüm
- [Ekran Görüntüleri Rehberi](docs/EKRAN_GORUNTULERI/README.md) — gerekli görüntüler ve alım kuralları

Geliştirici belgeleri:

- [Geliştirici Rehberi](GELISTIRICI_REHBERI.md) — mimari gerekçeleri, katmanlar, test felsefesi
- [Fork ve Ticari Kullanım](FORK_REHBERI.md) — BSD 3-Clause, marka ve yayın adımları
- [Katkı Rehberi](CONTRIBUTING.md) — issue, PR, commit ve AI beyanı
- [Yol Haritası](ROADMAP.md) — 1.0 sonrası planlar ve fork'a bırakılanlar

> Bu README teknik bir özettir. Günlük kullanım için yukarıdaki belgeleri
> kullanın; kurulum ve veri yolları için Kurulum Rehberi'ne bakın.

## Ekran Görüntüleri

![Alış Faturaları Ekranı](<docs/EKRAN_GORUNTULERI/Alis faturalari ekrani.png>)

![Ayarlar Ekranı](<docs/EKRAN_GORUNTULERI/Ayarlar ekrani.png>)

![Cari Hesaplar Ekranı](<docs/EKRAN_GORUNTULERI/Cari hesaplar ekrani.png>)

![Gelir ve Gider Ekranı](<docs/EKRAN_GORUNTULERI/Gelir ve gider ekrani.png>)

![Genel Bakış Ekranı](<docs/EKRAN_GORUNTULERI/Genel bakis ekrani.png>)

Tümü için: [docs/EKRAN_GORUNTULERI/](docs/EKRAN_GORUNTULERI/)

## Uzun Vadeli Dayanıklılık

Bu proje, uzun yıllar boyunca çalıştırılabilir ve yeniden derlenebilir olacak
şekilde tasarlanmıştır:

- Windows 10+ native; harici runtime gerekmez (MSVC runtime statik linklidir).
- Tüm veriler yerel SQLite dosyasında; sunucu/bulut bağımlılığı yok.
- `Cargo.lock` tam pinli; `rust-toolchain.toml` derleyici sürümünü sabitler.
- 1.0.0 sürümünün arşivi (MSI + kaynak + belgeler + `Cargo.lock`) ayrıca
  paketlenmiştir (bkz. `RELEASE_NOTES.md` ve `ARSIV_NOTLARI.md`).
- Yedekleme: `.onm-backup` dosyası SQLite veritabanının bayt kopyasıdır; herhangi
  bir SQLite aracıyla 20 yıl sonra da açılabilir.
- Kod imzalama yapılmamıştır; ileride imzalamak isteyenler için
  `KOD_IMZALAMA.md` rehberi vardır.

## Yapay Zeka ile Geliştirme (Vibe Coding)

Bu projeyi fork edip yapay zeka (ChatGPT, Claude, Cursor, OpenCode vb.) ile
geliştirmek istiyorsanız:

- [AI_DEVRALMA.md](AI_DEVRALMA.md) — Yapay zekaya yapıştırılacak proje tanıtımı
- [AI_PROMPTS.md](AI_PROMPTS.md) — Hazır başlangıç promptları

İlk adım: AI_DEVRALMA.md'yi yapay zekanıza yapıştırın, "bu projeyi öğren, henüz
kod yazma" deyin. Sonra AI_PROMPTS.md'den ihtiyacınız olan promptu kullanın.

## Kullanılan teknolojiler

| Amaç | Teknoloji |
|------|-----------|
| Programlama dili | Rust (edition 2024) |
| Arayüz (GUI) | [`egui`](https://crates.io/crates/egui) + [`eframe`](https://crates.io/crates/eframe) 0.37 |
| Veritabanı | SQLite ([`rusqlite`](https://crates.io/crates/rusqlite) 0.40, `bundled`) |
| Veri dizini çözümlemesi | [`directories`](https://crates.io/crates/directories) 6 |
| Hata yönetimi | [`thiserror`](https://crates.io/crates/thiserror) 2 |

`rusqlite` **`bundled`** özelliğiyle derlenir: SQLite kaynak koddan derlenip
programın içine gömülür, bu nedenle kullanıcının makinesinde ayrıca bir SQLite
DLL'i bulunması gerekmez. `eframe`/`egui` varsayılan olarak `wgpu` çizim
arka ucunu kullanır.

## Katmanlar

```
src/
├── main.rs              Yalnızca katmanları birbirine bağlar (giriş noktası)
├── lib.rs               Çekirdek kütüphane kökü ve ortak sabitler
├── error.rs             Ortak hata tipi (AppError) ve Result kısayolu
├── config.rs            Veri dizini ve veritabanı yolu
├── money.rs             Kuruş (i64) tabanlı para tipi
├── quantity.rs          Mili-birim (1/1000) tabanlı miktar tipi
├── customer.rs          Cari hesap iş modeli ve doğrulama kuralları
├── product.rs           Ürün ve stok hareketi iş modeli ve doğrulama kuralları
├── invoice.rs           Fatura iş modeli: satır hesabı, toplam, numara, doğrulama
├── cash.rs              Kasa/banka hesabı iş modeli ve doğrulama kuralları
├── payment.rs           Tahsilat/ödeme iş modeli, dağıtım ve doğrulama kuralları
├── app.rs               Uygulama durumu (OnMuhasebeApp + eframe::App)
├── app/
│   ├── customer_screen.rs  Cari hesap ekran durumu ve komutları
│   ├── product_screen.rs   Ürün ve stok ekran durumu ve komutları
│   ├── invoice_screen.rs   Fatura ekran durumu ve komutları
│   └── cash_screen.rs      Kasa/banka ve tahsilat/ödeme ekran durumu ve komutları
├── db/
│   ├── mod.rs           Veritabanı bağlantısı, ayarlar, durum özeti
│   ├── migrations.rs    Şema sürümleme (migration) altyapısı
│   ├── customers.rs     Cari hesap SQL sorguları (yalnızca burada SQL)
│   ├── products.rs      Ürün ve stok hareketi SQL sorguları (yalnızca burada SQL)
│   ├── invoices.rs      Fatura SQL sorguları (yalnızca burada SQL)
│   ├── cash.rs          Kasa/banka hesabı SQL sorguları (yalnızca burada SQL)
│   └── payments.rs      Tahsilat/ödeme ve dağıtım SQL sorguları (yalnızca burada SQL)
└── ui/
    ├── mod.rs
    ├── theme.rs         Renk paleti, ölçüler (metrics) ve egui stili
    ├── format.rs        Türkçe tarih ve bayt biçimlendirme
    ├── widgets.rs       Ortak arayüz bileşenleri (kart, başlık, madde)
    ├── navigation.rs    Sol gezinme menüsü
    ├── layout.rs        Menü çubuğu, üst bilgi, içerik alanı, durum çubuğu
    ├── overview.rs      "Genel Bakış" ekranı
    ├── customers.rs     Cari hesap listesi, formu ve ayrıntısı (yalnızca çizim)
    ├── products.rs      Ürün listesi, formu, ayrıntısı ve stok hareketi formu
    ├── invoices.rs      Alış/satış fatura listesi, formu ve ayrıntısı
    ├── cash.rs          Kasa/banka hesapları ve tahsilat/ödeme çizimi
    └── placeholder.rs   Geliştirilmemiş modüller için bilgi ekranı
```

Bağımlılık yönü tek yönlüdür: `main → app → ui / db → error`. **Arayüz katmanı
veritabanına doğrudan erişmez**; tüm veri erişimi `db` katmanı ve
`OnMuhasebeApp` üzerinden yapılır.

## Arayüz

Ana pencere dört bölümden oluşur ve pencere yeniden boyutlandırıldığında
yerleşim bozulmaz:

* **Menü çubuğu** — Dosya / Modüller / Yardım.
* **Üst bilgi** — solda program adı ve açık sayfanın başlığı, sağda sistem tarihi.
* **Sol gezinme** — Genel Bakış, Cari Hesaplar, Ürün ve Stok, Alış Faturaları,
  Satış Faturaları, Kasa ve Banka, Gelir ve Gider, Raporlar, Ayarlar.
* **Durum çubuğu** — solda veritabanı durumu, sağda program sürümü.

Dokuz bölümün tamamı işlevseldir: **Genel Bakış**, **Cari Hesaplar**, **Ürün ve
Stok**, **Alış Faturaları**, **Satış Faturaları**, **Kasa ve Banka**, **Gelir ve
Gider**, **Raporlar** ve **Ayarlar**. Genel Bakış; sistem tarihini, modül
kısayollarını, veritabanı bağlantı durumunu, şema sürümünü, veritabanı dosyasının
yolunu ve program sürümünü gösterir. Ayrıca **gerçek kayıt sayılarını** (aktif ürün
sayısı, düşük stoklu ürün sayısı, cari hesap sayısı), **kesinleşmiş fatura
toplamlarını** (satış/alış, KDV dâhil), **bu ay gelir/gider/net** tutarlarını,
**toplam kasa**, **toplam banka** ve **toplam nakit** tutarlarını, son faturaları,
son stok hareketlerini ve son tahsilat/ödemeleri listeler; **uydurma satış, kâr
veya kasa bakiyesi gösterilmez.** Gelir ve Gider bölümü hareket, kategori ve özet
sekmelerini; Raporlar bölümü altı salt okunur raporu; Ayarlar bölümü genel,
güvenlik, yedekleme ve kullanıcı sekmelerini içerir.

### Cari hesaplar

Cari hesaplar bölümü üç ekrandan oluşur:

* **Liste** — cari kodu, ad/unvan, tür, telefon, vergi dairesi, vergi/TC kimlik
  numarası, güncel bakiye ve durum gösterilir. Ada, koda, telefona ve vergi
  numarasına göre arama; tür ve duruma göre filtreleme yapılabilir. Sonuç
  bulunamazsa açıklayıcı bir mesaj gösterilir.
* **Form** — yeni kayıt ekleme ve düzenleme. Cari kodu otomatik önerilir ve
  değiştirilebilir; kodun benzersizliği hem arayüzde hem veritabanında
  (`UNIQUE`) güvence altındadır. Zorunlu alanlar, e-posta biçimi, telefon ve
  numara alanları doğrulanır. Kaydetme başarısız olursa **kullanıcının girdiği
  bilgiler korunur** ve hatalı alanlar işaretlenir.
* **Ayrıntı** — cari bilgileri, oluşturulma/güncellenme tarihi, güncel bakiye,
  hesap özeti, o cariye ait **fatura listesi** (numara, tarih, tür, tutar, iptal
  durumu) ile kompakt **tahsilat** ve **ödeme** listeleri. İlgili kayıt yoksa
  "bu cari için henüz fatura kaydı bulunmuyor" / "kayıt yok" bilgisi gösterilir.
  Ayrıca **Tahsilat Al** ve **Ödeme Yap** düğmeleri vardır (cari türü uyuşmazsa
  uyarı ödeme formunda gösterilir).

Kayıtlar **silinmez**; bunun yerine "pasife alınır" (Aktif/Pasif). Cari bakiye
elle değiştirilemez ve **kesinleşmiş kayıtlardan** otomatik hesaplanır:
`bakiye = toplam kesinleşmiş satış − toplam kesinleşmiş alış − toplam kesinleşmiş
tahsilat + toplam kesinleşmiş ödeme`. Pozitif değer cari bize borçlu, negatif
değer biz cariye borçluyuz demektir; iptal edilen fatura ve ödemeler hesaba
katılmaz.

### Ürün ve stok

Ürün ve stok bölümü dört ekrandan oluşur:

* **Liste** — ürün kodu, barkod, ad, birim, alış/satış fiyatı, KDV oranı, güncel
  stok, minimum stok ve durum gösterilir. Koda, barkoda, ada ve kategoriye göre
  arama (Türkçe karakter normalizasyonu ile); durum ve "yalnızca düşük stoklu
  ürünler" filtreleri vardır. Düşük stoklu satırlar (mevcut stok < minimum stok)
  görsel olarak işaretlenir.
* **Form** — yeni kayıt ekleme ve düzenleme. Ürün kodu otomatik önerilir
  (`URN-0001`, `URN-0002` …), değiştirilebilir. Kod ve barkodun benzersizliği hem
  arayüzde hem veritabanında garantidir. Alış/satış fiyatı `Money` ile, miktarlar
  `Quantity` ile ayrıştırılır; negatif değerler reddedilir. Yeni kayıtta isteğe
  bağlı **başlangıç stoğu** girilirse tek bir `İlk Stok` hareketi oluşturulur.
  Kaydetme başarısız olursa **kullanıcının girdiği bilgiler korunur**.
* **Ayrıntı** — ürün bilgileri, oluşturulma/güncelleme tarihi, durum, güncel stok,
  düşük stok uyarısı ve stok hareketleri listesi (tarihe göre yeniden eskiye).
  Boşsa "bu ürün için henüz stok hareketi bulunmuyor" bilgisi gösterilir.
  Düzenle / Yeni hareket / Geri dön seçenekleri vardır.
* **Stok hareketi formu** — Giriş / Çıkış / Düzeltme türleri. Giriş ve çıkışta
  yön sistem tarafından belirlenir; düzeltmede yön kullanıcı tarafından seçilir ve
  açıklama zorunludur. Kaydetmeden önce "Yeni stok" önizlemesi gösterilir. Hareket
  sonucu stok eksiye düşecekse onay istenir ("Stok eksiye düşecek … Devam
  edilsin mi?"); kullanıcı vazgeçerse hiçbir şey kaydedilmez.

Ürün kayıtları **silinmez**, yalnızca pasife alınır. Güncel stok elle
değiştirilemez; güncel stok yalnızca stok hareketleriyle değişir ve her kayıt
kendi işlemi (transaction) içinde yazılır. Hareketler doğrudan düzenlenemez veya
silinemez; bir hareketi düzeltmek için ters yönlü yeni bir **Düzeltme** hareketi
girilir (izlenebilirlik korunur).

### Alış ve satış faturaları

Alış ve satış faturaları aynı ekranı paylaşır (sol menüde iki ayrı bölüm olarak
açılır) ve üç ekrandan oluşur:

* **Liste** — numara, tarih, cari, matrah, KDV, genel toplam, **kalan** ve durum
  (Kesin/İptal) gösterilir. Kalan sütununda ödeme rozeti vardır: kalan 0 ⇒
  "Ödendi" (yeşil), kısmi ödenmiş ⇒ "Kısmi" (sarı), ödeme yok ⇒ "Ödenmemiş"
  (kırmızı), iptal edilmiş fatura ⇒ "—". Tarih aralığı, cari ve durum filtreleri
  ile numara veya cari adına göre arama (Türkçe karakter normalizasyonu ile)
  yapılabilir. Alt bilgide "X fatura, genel toplam: Y" yazar; iptal edilen
  faturalar toplama katılmaz. Eylemler: **Yeni fatura**, **Ayrıntı**, **İptal et**
  (yalnızca kesin faturalarda).
* **Form (yalnızca yeni fatura)** — fatura türü (yalnızca oluştururken seçilir),
  numara (seri ve yıla göre otomatik önerilir, düzenlenebilir), fatura tarihi
  (varsayılan bugün), vade tarihi (isteğe bağlı, yalnızca bilgi amaçlı), cari
  hesap (aranabilir liste; beklenen cari türü uyuşmazsa uyarı gösterilir ama
  kayıt engellenmez), notlar ve dinamik satır tablosu. Her satırda ürün
  (aranabilir liste; seçilince kodu, birimi ve varsayılan fiyatı gelir), miktar,
  birim fiyat, iskonto %, KDV % (hazır oran seçenekleriyle) ve salt okunur satır
  net tutarı bulunur. Aynı ürün birden fazla satırda kullanılabilir. Toplamlar
  (brüt, iskonto, matrah, KDV, genel toplam) yazdıkça canlı hesaplanır.
* **Ayrıntı** — fatura üst bilgisi, satırlar, toplamlar, kayıt zamanı, **ödenen
  ve kalan tutar + ödeme durumu rozeti**, faturaya bağlı ödeme kayıtları listesi
  (numara yoksa "Bu faturaya henüz ödeme kaydı yok.") ve **Tahsilat Al** (satış)
  / **Ödeme Yap** (alış) düğmesi. Bu düğme, ödeme formunu o faturaya kalan tutar
  kadar ön-dağıtılmış olarak açar ve kayıttan sonra fatura ayrıntısına döner.
  Fatura kesinleştikten sonra **düzenlenemez**; düzeltmek için iptal edip yeniden
  oluşturulur.

Satır tutarları `Money` (kuruş) ve `Quantity` (mili-birim) üzerinden `i128` ara
sonuçla hesaplanır; yarım kuruş uzak yönde yuvarlanır. Fatura numarası büyük/küçük
harf duyarsız benzersizdir (`SF-2026-0001`, `AF-2026-0001`); son kararı veritabanı
`UNIQUE` kısıtı verir.

Kaydetme ve iptal **tek bir işlem (transaction)** içinde yürür. Kesinleştirmede
fatura başlığı, satırlar, her satır için bir stok hareketi (alış: `alis_faturasi`
artı yön, satış: `satis_faturasi` eksi yön) ve ilgili ürünlerin güncel stoğu
birlikte yazılır. Satış kaydı bir ürünün stoğunu eksiye düşürecekse kullanıcıdan
onay istenir; onaylanmazsa hiçbir kayıt oluşmaz. Kaydetmeden önce "Fatura
kesinleşecek. Stok ve cari bakiyesi güncellenecek. Onaylıyor musunuz?" onayı
alınır.

Faturalar **silinmez**; bunun yerine **iptal** edilir. İptal, iptal nedenini
zorunlu kılar ve her satır için ters yönlü bir `duzeltme` stok hareketi (açıklama:
"Fatura iptali: &lt;no&gt;") üretip stok ile cari bakiyesini geri alır. **Ödenmiş
(paid_amount &gt; 0) bir fatura iptal edilemez**: "Bu faturaya bağlı
tahsilat/ödeme bulunuyor. Önce ilgili ödemeleri iptal edin." mesajı döner. İptal
edilen fatura listede kalır ve durumuyla birlikte görünür.

### Kasa, banka ve tahsilat/ödeme

Kasa ve Banka bölümü iki sekmeden oluşur: **Hesaplar** (varsayılan) ve
**Hareketler**.

* **Hesaplar sekmesi** — nakit kasa ve banka hesapları. Sütunlar: ad, tür
  (Kasa/Banka), banka adı, IBAN (kısaltılmış), açılış bakiyesi, güncel bakiye ve
  durum (Aktif/Pasif). Üstte toplam kasa, toplam banka ve genel toplam (yalnızca
  **aktif** hesaplar) gösterilir. Ada göre arama (Türkçe karakter normalizasyonu
  ile), tür ve durum filtreleri vardır. Eylemler: **Yeni hesap**, **Düzenle**,
  **Pasife al / Aktife al**.
  Hesap formunda ad (zorunlu, benzersiz), tür, banka adı (yalnızca banka
  hesabında), IBAN (yalnızca banka hesabında; 26 karakter, "TR" ile başlar,
  isteğe bağlı), açılış bakiyesi, notlar ve Aktif/Pasif durumu bulunur. **Açılış
  bakiyesi yalnızca yeni hesapta veya hiç hareketi olmayan hesapta
  değiştirilebilir**; hareketi olan hesapta salt okunur ve nedeni açıklanır.
* **Hareketler sekmesi** — tahsilat ve ödemeler; "Tümü / Tahsilatlar / Ödemeler"
  alt sekmeleri. Sütunlar: tarih, tür, cari, kasa/banka, tutar ve durum. Tarih
  aralığı, cari (aranabilir), hesap, tür ve durum filtreleri; açıklama veya cari
  adında arama. Alt bilgide (iptaller hariç) toplam tahsilat, toplam ödeme ve net
  gösterilir. Eylemler: **Ayrıntı** ve **İptal et** (yalnızca kesin kayıtlarda).
  Yeni Tahsilat ve Yeni Ödeme düğmeleri her zaman görünür.
* **Ödeme formu** — tür (yalnızca yeni kayıtta; değiştirilince cari filtresi ve
  fatura listesi yenilenir), cari (aranabilir; tahsilat için müşteri, ödeme için
  tedarikçi türü önce gelir; tür uyuşmazsa turuncu **uyarı** gösterilir ama kayıt
  **engellenmez**), kasa/banka (yalnızca aktif hesaplar), tarih (varsayılan bugün)
  ve açıklama. Dağıtım paneli, seçili carinin uygun türdeki (tahsilat: satış,
  ödeme: alış) kesin ve kalanı > 0 faturalarını listeler; her satır için
  "dağıtılacak miktar" elle girilebilir, **Tümünü dağıt** düğmesi kalanları
  doldurur. Özette ödeme tutarı, dağıtılan toplam ve **kalan (avans)** gösterilir.
  Fazla dağıtım kırmızı uyarı verir ve kaydı engeller; dağıtılan &lt; tutar olabilir
  (fark avans sayılır); fatura listesi boşsa tüm tutar avans olarak kaydedilebilir.
* **Ödeme ayrıntısı ve iptal** — üst bilgi (tür, tarih, cari, kasa/banka, tutar,
  dağıtılan, avans, açıklama, durum) ve fatura dağıtımları listesi. İptal
  edilmişse iptal zamanı ve nedeni gösterilir. Düğmeler: **İptal et** ve
  **Listeye dön**.

Kaydetme ve iptal **tek bir işlem (transaction)** içinde yürür. Kesinleştirmede
`payments` kaydı, her dağıtım satırı, ilgili faturaların `paid_amount` artışı ve
kasa/banka güncel bakiyesi (tahsilatta artı, ödemede eksi yön) birlikte yazılır.
İptalde bu etkiler tersine alınır (`paid_amount` düşer, kasa bakiyesi geri gelir),
kayıt `iptal` durumuna geçer ve iptal nedeni saklanır. Kayıtlar **silinmez**;
düzeltme = iptal + yeni kayıt. Kasa bakiyesi eksiye düşebilir (engellenmez).

Bir kayıt, cari türüyle eşleşen fatura türüne dağıtılabilir (tahsilat → yalnızca
`satis`, ödeme → yalnızca `alis`), yalnızca aynı cariye ait, `kesin` ve kalanı > 0
faturalara; `dağıtım ≤ kalan` ve `dağıtım > 0` koşulları geçerlidir. Bir faturaya
birden çok ödeme dağıtılabilir. Bu sürümde çek/senet, kredi kartı, döviz ve
virman yoktur; tüm hesaplar TL'dir.

### Gelir, gider ve raporlar

**Gelir ve Gider** bölümü üç sekmeden oluşur: **Hareketler**, **Kategoriler** ve
**Özet**. Hareket formunda tür (Gelir/Gider), kategori, kasa/banka hesabı, tarih,
tutar, belge/fiş no ve açıklama bulunur; kaydetmeden önce "Yeni kasa/banka
bakiyesi" önizlemesi gösterilir. Kategori adı aynı türde benzersizdir; pasif
kategoriye yeni hareket eklenemez. Kayıtlar silinmez, **neden zorunlu** olarak
iptal edilir ve iptal kasa bakiyesini geri alır. Özet sekmesi dönem toplamlarını,
kategori dağılımını ve altı aylık trendi gösterir.

**Raporlar** bölümü salt okunurdur ve altı rapor içerir: **Cari Ekstre**, **Cari
Bakiye Özeti**, **Stok Durumu**, **Satış/Alış Özeti**, **Kasa/Banka Hareket Özeti**
ve **Gelir/Gider Özeti**. Her raporun kendi filtreleri (tarih aralığı, cari, hesap,
tür, durum, gruplama) ve **CSV dışa aktar** düğmesi vardır. Rapor verileri mevcut
tablolardan hesaplanır; ayrı rapor tablosu yoktur.

### Kullanıcılar, oturum ve ayarlar

İlk açılışta **ilk yönetici** hesabı oluşturulur; ardından dört adımlı kurulum
sihirbazı (şirket bilgileri, ilk kasa/banka, kategori paketleri, özet) açılır ve
atlanabilir. Şifreler Argon2id ile özetlenir; düz metin saklanmaz. Şifre kuralı:
en az 8 karakter, en az bir harf ve bir rakam. Aynı kullanıcı adı için 5 hatalı
denemeden sonra giriş 30 saniye kilitlenir. Hareketsizlik sonrası otomatik kilit
varsayılan olarak açıktır (15 dakika; 5/15/30/60 seçilebilir).

İki rol vardır: **Yönetici** ve **Operatör**. Operatör kayıt ekleyip
düzenleyebilir; ancak iptal/pasife alma, kullanıcı yönetimi, yedek geri yükleme ve
CSV içe aktarma yapamaz. Ayarlar bölümünde **Genel** (kilit, log seviyesi, kurulum
sihirbazı), **Güvenlik** (kendi şifresi), **Yedekleme** (manuel yedek, rotasyon,
son yedekler, geri yükleme) ve **Kullanıcılar** (yalnızca yönetici) sekmeleri
bulunur. Otomatik yedek, son yedekten 24 saat geçtiyse uygulama açılışında alınır;
varsayılan rotasyon 30 günlük / 12 aylık / 5 yıllıktır.

## Veritabanı

* Konum: kullanıcının yerel uygulama verisi dizini.
  * Windows: `%LOCALAPPDATA%\OnMuhasebe\OnMuhasebe\data\on_muhasebe.db`
* `ON_MUHASEBE_DATA_DIR` ortam değişkeni ile konum geçici olarak değiştirilebilir
  (taşınabilir kullanım ve testler için).
* Dosya ve üst dizin ilk açılışta otomatik oluşturulur.
* Açılışta `PRAGMA foreign_keys = ON`, `busy_timeout = 5000`, `journal_mode = WAL`
  ayarlanır.

### Şema sürümleme (migration)

* `src/db/migrations.rs` içindeki `MIGRATIONS` listesi tüm şema değişikliklerini
  sırayla tutar.
* Uygulanan migration'lar `schema_migrations` tablosuna yazılır.
* Her migration kendi işlemi (transaction) içinde uygulanır.
* Veritabanı, uygulamadan daha yeni bir şema içeriyorsa açılış hata verir
  (`AppError::SchemaTooNew`) — sessizce bozulma olmaz.

**Yeni migration eklemek için:** `MIGRATIONS` listesinin sonuna yeni bir kayıt
ekleyin, `version` değerini bir artırın ve SQL'i `IF NOT EXISTS` kullanmadan yazın.

Bu sürümde oluşturulan tablolar:

| Tablo | Sürüm | Amaç |
|-------|-------|------|
| `schema_migrations` | 1 | Uygulanmış şema sürümlerinin kaydı (altyapı) |
| `app_metadata` | 1 | Uygulama meta verisi: `created_at`, `last_app_version` (altyapı) |
| `customers` | 2 | Cari hesaplar (müşteri/tedarikçi kartları) |
| `products` | 3 | Ürün/stok kartları |
| `product_stock_movements` | 3 → 4 | Stok hareketleri (v4'te fatura bağlantısı için yeniden oluşturuldu) |
| `invoices` | 4 → 5 | Alış/satış fatura başlıkları (v5'te `paid_amount` eklendi) |
| `invoice_lines` | 4 | Fatura satırları (ürün bilgisi anlık görüntülenir) |
| `cash_accounts` | 5 | Kasa/banka hesapları |
| `payments` | 5 | Tahsilat/ödeme kayıtları |
| `payment_allocations` | 5 | Ödemelerin faturalara dağıtımı |
| `expense_categories` | 6 | Gelir/gider kategorileri |
| `income_expense_entries` | 6 | Gelir/gider hareketleri |
| `users` | 7 | Kullanıcı hesapları (Argon2id şifre özeti, rol, durum) |
| `app_settings` | 7 | Anahtar-değer uygulama ayarları |

`customers` tablosunda `code` alanı **büyük/küçük harf duyarsız `UNIQUE`**'tir
(`COLLATE NOCASE`), tür ve durum alanlarında `CHECK` kısıtları vardır. `id`
alanı `AUTOINCREMENT`'tir; böylece kimlikler yeniden kullanılmaz ve sonraki
aşamalarda eklenecek hareket tabloları `REFERENCES customers(id)` ile güvenle
bağlanabilir. Ada, duruma ve türe göre indeksler oluşturulur.

`products` tablosunda `code` **büyük/küçük harf duyarsız `UNIQUE`**, `barkod`
yalnızca dolu olduğunda benzersizdir (kısmi indeks) ve `birim`, `KDV oranı` ile
`durum` alanlarında `CHECK` kısıtları vardır. `product_stock_movements` tablosu
`product_id` ile `products(id)` tablosuna bağlanır, hareket türü `CHECK` ile
sınırlandırılır ve miktar farkı sıfır olamaz (`CHECK(quantity_delta <> 0)`).
Güncel stok, bu hareketlerin toplamına eşittir.

`invoices` tablosunda `invoice_number` **büyük/küçük harf duyarsız `UNIQUE`**'tir
(`COLLATE NOCASE`), tür (`alis`/`satis`) ve durum (`kesin`/`iptal`) alanlarında
`CHECK` kısıtları, tutar alanlarında `>= 0` koşulları vardır. `invoice_lines`
tablosu faturaya `ON DELETE CASCADE` ile bağlanır; ürün kodu/adı/birimi **anlık
görüntü olarak** saklanır (ürün kartı sonradan değişse bile fatura bozulmaz),
`(invoice_id, line_no)` çifti benzersizdir ve satır tutarları hesaplanmış olarak
yazılır.

**Migration v4** yalnızca `invoices` ve `invoice_lines` tablolarını eklemekle
kalmaz: `product_stock_movements` tablosunu, hareket türü `CHECK` kısıtına
`alis_faturasi`/`satis_faturasi` değerlerini eklemek ve yeni `invoice_id` yabancı
anahtarını (nullable) taşımak için **yeniden oluşturur** (yeni tablo → `INSERT
... SELECT` ile veri taşıma → eski tabloyu bırakma → yeniden adlandırma). Mevcut
stok hareketleri ve indeksler korunur.

**Gelir-gider** tabloları (v6) ve **kullanıcı/uygulama ayarı** tabloları (v7)
oluşturulmuştur. Raporlar ayrı tablo gerektirmez; mevcut cari, fatura, stok, kasa
ve gelir-gider tablolarından hesaplanır. Yeni hareketler `customers(id)`,
`invoices(id)` ve `cash_accounts(id)` tablolarına `REFERENCES` ile bağlanır.

**Migration v5** yalnızca ekleme (additive) niteliğindedir:

* `invoices` tablosuna `paid_amount INTEGER NOT NULL DEFAULT 0` eklenir (mevcut
  faturalar için 0 olur; ödenen tutar dağıtımlardan burada tutulur).
* Yeni tablolar: `cash_accounts` (ad `NOCASE UNIQUE`, tür `CHECK ('kasa','banka')`,
  `opening_balance >= 0`, işaretli `current_balance`, `active` 0/1),
  `payments` (tür `CHECK ('tahsilat','odeme')`, `amount > 0`, durum
  `CHECK ('kesin','iptal')`, `cancelled_at`, `cancel_reason`) ve
  `payment_allocations` (ödeme → fatura, `amount > 0`, `UNIQUE(payment_id,
  invoice_id)`, faturaya `ON DELETE CASCADE`).
* Yeni indeksler: `idx_cash_accounts_active`, `idx_payments_customer`,
  `idx_payments_cash_account`, `idx_payments_type_date`, `idx_payments_status`,
  `idx_payment_allocations_payment`, `idx_payment_allocations_invoice`,
  `idx_invoices_paid_amount`.
* v1–v4 değişmez; mevcut veri (faturalar, satırlar, stok ve `SUM(current_stock)`)
  korunur (testle doğrulanır).

## Para hesapları

Para tutarları hiçbir zaman `f32`/`f64` ile saklanmaz. `money::Money` tipi tutarı
**kuruş** (1/100 TL) cinsinden `i64` olarak tutar; böylece kayan noktalı yuvarlama
hataları oluşmaz ve veritabanına doğrudan `INTEGER` kolonu olarak yazılabilir.

```rust
use on_muhasebe::money::Money;

let birim_fiyat = Money::from_kurus(12_550)?;   // 125,50 TL
let toplam = birim_fiyat.checked_mul(3)?;        // 376,50 TL
assert_eq!(toplam.format_with_symbol(), "376,50 ₺");

let okunan = Money::parse("1.234,56")?;          // 1234,56 TL
```

Tüm aritmetik işlemler taşma kontrolü yapar (`checked_*`) ve `Result` döner.

## Miktar hesapları

Stok miktarları ondalık kesirleri (ör. 1,5 kg) kesin olarak ifade edebilmek için
`quantity::Quantity` tipiyle **mili-birim** (1/1000) cinsinden `i64` olarak
saklanır; `f32`/`f64` kullanılmaz. Türkçe biçimlendirme desteklenir: ondalık
ayırıcı virgül, binlik ayırıcı noktadır.

```rust
use on_muhasebe::quantity::Quantity;

let miktar = Quantity::parse("12,5")?;          // 12,5 birim
assert_eq!(miktar.format_turkish(), "12,5");

let toplam = miktar.checked_add(Quantity::parse("2,5")?)?;
assert_eq!(toplam.format_turkish(), "15");
```

## Derleme ve çalıştırma

Gereksinimler: Rust (MSVC araç zinciri) ve Visual Studio C++ derleme araçları +
Windows SDK. SQLite gömülü olarak derlendiği için ayrıca kurulum gerekmez.

```powershell
# Bağımlılıkları çöz ve indir
cargo fetch

# Derleme kontrolü
cargo check

# Testler
cargo test

# Geliştirme derlemesi ile çalıştır
cargo run

# Dağıtım derlemesi (bağımsız .exe)
cargo build --release
```

Çalıştırılabilir dosya: `target\release\OnMuhasebe.exe`

### Ortam notu

`cargo` ve `rustc`, `%USERPROFILE%\.cargo\bin` dizininde bulunur. Bu dizin
`PATH` üzerinde değilse komutlar tanınmaz. Kalıcı olarak eklemek için:

```powershell
[Environment]::SetEnvironmentVariable(
  "Path",
  [Environment]::GetEnvironmentVariable("Path", "User") + ";$env:USERPROFILE\.cargo\bin",
  "User")
```

## Testler

| Dosya | Kapsam |
|-------|--------|
| `src/config.rs` | Veri dizini/veritabanı yolu, dizin oluşturma |
| `src/error.rs` | Hata dönüşümleri |
| `src/money.rs` | Para tipi: dönüşüm, biçimlendirme, ayrıştırma, taşma |
| `src/quantity.rs` | Miktar tipi: dönüşüm, biçimlendirme, ayrıştırma, taşma |
| `src/customer.rs` | Cari modeli, alan doğrulaması, kod önerisi, Türkçe arama normalizasyonu |
| `src/product.rs` | Ürün/stok modeli, alan doğrulaması, birim ve hareket türleri, kod önerisi |
| `src/invoice.rs` | Fatura modeli: satır hesabı ve yuvarlama, toplam, numara önerisi, doğrulama, ödeme durumu |
| `src/cash.rs` | Kasa/banka hesabı modeli: ad/tür/IBAN doğrulaması, bakiye, Türkçe arama, kısaltma |
| `src/payment.rs` | Tahsilat/ödeme modeli: dağıtım, avans/kalan, tür uyumu uyarısı, doğrulama |
| `src/db/mod.rs` | Bellek içi veritabanı, meta veri okuma/yazma |
| `src/db/migrations.rs` | Migration sırası, tekrarlanmama, yeni şema reddi, tablolar ve kısıtlar, v4/v5'te verinin korunması |
| `src/db/customers.rs` | Cari ekleme/güncelleme/listeleme, filtreler, bakiye (tahsilat/ödeme dâhil), hata eşlemesi |
| `src/db/products.rs` | Ürün ve stok hareketi ekleme/güncelleme/listeleme, stok toplamı, hata eşlemesi |
| `src/db/invoices.rs` | Fatura ekleme/iptal (tek işlem), listeleme, numara benzersizliği, `paid_amount`, iptal koruması |
| `src/db/cash.rs` | Kasa/banka hesabı ekleme/güncelleme/listeleme, pasifleştirme, toplamlar, hata eşlemesi |
| `src/db/payments.rs` | Tahsilat/ödeme kesinleştirme ve iptal (tek işlem), dağıtımlar, bağıntı sorguları |
| `src/ui/format.rs` | Türkçe uzun/kısa tarih, ISO kısa tarih ve bayt biçimlendirme |
| `src/ui/theme.rs` | Yazı tiplerinde Türkçe karakter kapsamı |
| `src/ui/layout.rs` | Yerleşim ölçüleri, dar pencerede taşma denetimi, tüm bölüm ve formların çizimi |
| `tests/database.rs` | Uçtan uca: dosya oluşturma, kalıcılık, durum özeti |
| `tests/customers.rs` | Uçtan uca: cari ekleme/düzenleme, kod benzersizliği, doğrulama, arama/filtre, aktif-pasif, yeniden açılışta kalıcılık |
| `tests/products.rs` | Uçtan uca: ürün ekleme/düzenleme, kod/barkod benzersizliği, doğrulama, stok hareketleri, negatif stok onayı, arama/filtre, aktif-pasif, yeniden açılışta kalıcılık |
| `tests/invoices.rs` | Uçtan uca: fatura kesinleştirme, stok/cari etkisi, negatif stok onayı, numara benzersizliği, iptal (ters kayıt), işlem geri alma, arama/filtre, yeniden açılışta kalıcılık |
| `tests/cash.rs` | Uçtan uca: hesap ekleme/düzenleme, ad benzersizliği, IBAN doğrulaması, açılış bakiyesi kuralı, arama/filtre, pasife alma, migration v5 |
| `tests/payments.rs` | Uçtan uca: tam/kısmi/çoklu dağıtım, tam avans, fazla dağıtım reddi, iptal ve geri alma, fatura iptal koruması, bağıntı testleri, yeniden açılışta kalıcılık |
| `tests/stress/` | Aşama 10: büyük veri, korelasyon, fuzz, hata enjeksiyonu, migration zinciri ve soak testleri |

### Ağır testler (Aşama 10)

Normal test koşusu (`cargo test --all-targets`) hızlı testleri çalıştırır;
büyük veri, fuzz ve soak testleri `#[ignore]` ile işaretlidir ve CI'da
çalıştırılmaz. Bu testler yalnızca release profilinde ve tek iş parçacığında
anlamlı sürelerde tamamlanır:

```powershell
# Yalnızca stress testleri (önerilen)
cargo test --release --test stress -- --ignored --nocapture --test-threads=1

# Depodaki tüm ignore edilmiş testler
cargo test --release -- --ignored --test-threads=1
```

Ağır paket 7 testtir: 1.000.000 cari, 10.000.000 stok hareketi, 50.000
fatura/ödeme, 1.000 satırlı fatura, 1.000.000 para/miktar fuzz girdisi ve
100 turluk soak. Son ölçümde tamamı tek iş parçacığında yaklaşık 153 saniyede
tamamlanmıştır. Geçici veritabanları `tempfile` ile oluşturulur ve test sonunda
otomatik silinir; kullanıcı veri dizinine dokunulmaz.

## Sonraki aşamalar

1. Geliştirici rehberi ve mimari dokümantasyon (aşama 17)
2. GitHub Releases yayını ve sürüm duyurusu (aşama 18)

AI devir paketi (aşama 16) tamamlandı: [AI_DEVRALMA.md](AI_DEVRALMA.md) ve
[AI_PROMPTS.md](AI_PROMPTS.md).

Bu sürümde PDF/XLSX çıktısı, e-Fatura/e-Arşiv/e-Defter, çek/senet, kredi kartı,
döviz, virman, otomatik güncelleme ve bulut yedekleme **yoktur**.

## MSI kurulum paketi

MSI, `cargo-wix` ve **WiX Toolset v3.14** ile üretilir. `candle.exe` ve
`light.exe` dosyalarının bulunduğu `bin` klasörü `PATH` veya `WIX` ortam
değişkeninde olmalıdır.

```powershell
cargo install cargo-wix
.\build-msi.bat
```

Sürüm yalnızca `Cargo.toml` içindeki `[package] version` alanından okunur.
Çıktı: `dist\OnMuhasebe-<sürüm>.msi`. Ayrıntılı adımlar için
`wix/YENI_SURUM.md` dosyasına bakın.

## Şifre kurtarma

Başka bir **yönetici** hesabı varsa onunla giriş yapıp **Ayarlar > Kullanıcılar >
İşlemler > Şifre sıfırla** ile şifre yenilenir.

Başka yönetici yoksa:

1. Programı kapatın ve veritabanının bir kopyasını alın.
2. Bir SQLite aracıyla (ör. DB Browser for SQLite) veritabanını açın:
   `%LOCALAPPDATA%\OnMuhasebe\OnMuhasebe\data\on_muhasebe.db`
3. Şu komutu çalıştırın: `DELETE FROM users;`
4. Değişikliği kaydedip programı açın. **İlk yönetici oluştur** ekranı gelir.
5. Yeni yönetici hesabını oluşturun.

`users` tablosu silinir; cari, ürün, fatura, ödeme ve diğer iş kayıtları
**silinmez**. Bu yöntem veritabanına erişimi olan herkese yönetici yetkisi verir;
bilgisayarı ve yedekleri fiziksel olarak koruyun. Ayrıntılar:
[docs/SSS.md](docs/SSS.md).

## Lisans

Bu proje BSD 3-Clause lisansı ile lisanslanmıştır. Telif hakkı (c) 2026, @furcanomer. Ayrıntılar için LICENSE dosyasına bakın.

## Geliştirici

**Geliştirici:** @furcanomer — https://github.com/furcanomer

---

Son güncelleme: 2026-10-27
