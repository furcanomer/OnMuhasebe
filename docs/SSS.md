# On Muhasebe — Sık Sorulan Sorular (SSS)

Bu belge, programı kullanırken en sık sorulan soruları kısa ve net biçimde
yanıtlar. Ayrıntılı anlatım için [KULLANIM_KILAVUZU.md](KULLANIM_KILAVUZU.md),
kurulum için [KURULUM.md](KURULUM.md), hatalar için
[SORUN_GIDERME.md](SORUN_GIDERME.md) belgelerine bakın.

---

### 1. Verilerim nerede saklanıyor?

Bilgisayarınızın yerel uygulama verisi klasöründe:

- Veritabanı: `%LOCALAPPDATA%\OnMuhasebe\OnMuhasebe\data\on_muhasebe.db`
- Yedekler: `%LOCALAPPDATA%\OnMuhasebe\OnMuhasebe\backups\`
- Günlükler: `%LOCALAPPDATA%\OnMuhasebe\OnMuhasebe\logs\`

`%LOCALAPPDATA%` genellikle `C:\Users\<kullanıcı>\AppData\Local` klasörüdür.

### 2. Verilerim buluta veya bir sunucuya gönderiliyor mu?

**Hayır.** Program tamamen çevrimdışı çalışır. Hiçbir veri internete, buluta veya
üçüncü taraf bir sunucuya gönderilmez. Programın çalışması için internet gerekmez.

### 3. Programı silsem verilerim silinir mi?

**Hayır.** Kaldırma işlemi yalnızca program dosyalarını ve kısayolları siler.
`%LOCALAPPDATA%\OnMuhasebe` klasöründeki veritabanı, yedekler ve günlükler korunur.
Programı yeniden kurarsanız verileriniz yerinde olur.

Verileri de silmek istiyorsanız programı kaldırdıktan sonra
`%LOCALAPPDATA%\OnMuhasebe` klasörünü elle silin. Bu işlem geri alınamaz.

### 4. Güncelleme yapınca eski verilerim kaybolur mu?

**Hayır.** Veritabanı `%LOCALAPPDATA%` altında olduğu için MSI kurulum ve kaldırma
işlemlerinden etkilenmez. Güncelleme sonrası tüm cariler, ürünler, faturalar,
ödemeler, kullanıcılar ve ayarlar aynen kalır.

Yine de güncellemeden önce **Ayarlar > Yedekleme > Şimdi yedek al** ile yedek
almanız önerilir.

### 5. Güncelleme sonrası programdan iki tane olur mu?

**Hayır.** Windows Installer, aynı `UpgradeCode` ile kurulmuş eski sürümü otomatik
olarak kaldırır ve yenisini kurar. **Programlar listesinde tek giriş kalır** ve
sürüm numarası yeni sürümü gösterir. Bu davranış gerçek MSI testiyle doğrulanmıştır.

Yalnızca çok eski, farklı bir kurulum yöntemiyle kurulmuş ikinci bir kopya varsa
Programlar listesinde iki giriş görülebilir; bu durumda ikisini de kaldırıp yeni
MSI'ı kurun.

### 6. Faturayı nasıl iptal ederim?

1. İlgili fatura listesinde (**Alış Faturaları** veya **Satış Faturaları**) satırı
   bulun.
2. **İptal Et** düğmesine tıklayın.
3. **İptal nedenini** yazın (zorunlu).
4. Onaylayın.

İptal, stok ve cari bakiyesini geri alır. Fatura listede "İptal" durumuyla kalır;
silinmez.

### 7. İptal ettiğim faturayı nasıl düzeltirim?

Kesinleşmiş fatura **düzenlenemez**. Doğru akış:

1. Hatalı faturayı **iptal** edin (neden yazın).
2. Doğru bilgilerle **yeni fatura** oluşturun.

Böylece hangi kaydın neden iptal edildiği izlenebilir kalır.

### 8. Aynı cari koduyla ikinci kayıt ekleyemiyorum, neden?

Cari kodu **benzersizdir** (büyük/küçük harf duyarsız). Veritabanında `UNIQUE`
kısıtı vardır. Aynı kodla ikinci kayıt açılamaz. **Yeni kod öner** düğmesi
kullanılmayan bir kod üretir. Aynı kural ürün kodu, barkod, fatura numarası,
kasa/banka hesap adı ve kategori adı için de geçerlidir.

### 9. Stok eksiye düşebilir mi?

**Evet.** Program stoğu eksiye düşürmeyi engellemez; yalnızca **onay ister**:

> "Stok eksiye düşecek … Devam edilsin mi?"

Onaylarsanız kayıt yapılır ve stok negatif görünür. Bu, sayım farkı veya geçmişe
dönük kayıt gibi durumlarda bilinçli bir tercihtir. Onaylamazsanız hiçbir şey
kaydedilmez.

### 10. Aynı faturaya iki tahsilat girebilir miyim?

**Evet.** Bir faturaya birden çok tahsilat/ödeme dağıtılabilir (kısmi ödeme).
Fatura ayrıntısında **kalan** tutar ve **Kısmi** rozeti görünür. Yeni tahsilat
girerken dağıtım panelinde faturaya kalan tutar kadar (veya daha az) dağıtım
yapabilirsiniz.

### 11. Faturayı iptal edemiyorum, "önce ödemeleri iptal edin" diyor. Ne yapmalıyım?

Faturaya bağlı kesin bir tahsilat/ödeme varsa fatura iptal edilemez. Sıra:

1. **Kasa ve Banka > Hareketler** bölümüne gidin.
2. İlgili tahsilat/ödeme kaydını bulun.
3. **İptal et** ile iptal edin (neden yazın).
4. Faturaya dönüp **İptal Et** ile faturayı iptal edin.

### 12. Yedeklerimi başka bilgisayara nasıl taşırım?

1. Eski bilgisayarda **Dosya > Tüm veriyi dışa aktar...** ile `.onm-backup`
   dosyasını alın (veya `backups` klasöründeki bir yedeği kullanın).
2. Dosyayı USB bellek, harici disk veya ağ üzerinden yeni bilgisayara kopyalayın.
3. Yeni bilgisayarda programı kurun, ilk yönetici hesabını oluşturun.
4. **Dosya > Yedeği geri yükle...** ile dosyayı seçin ve onaylayın.

Geri yükleme öncesi mevcut veritabanı otomatik olarak güvenlik yedeğine alınır.

### 13. Programı iki kişi aynı bilgisayarda kullanabilir mi?

**Evet, ama aynı anda değil.** Birden çok kullanıcı hesabı tanımlanabilir; her
kullanıcı kendi kullanıcı adı ve şifresiyle giriş yapar. Aynı anda yalnızca bir
oturum açıktır. Oturum kapatıldığında diğer kullanıcı giriş yapabilir.

### 14. Programı ağda paylaşabilir miyim?

**Bu sürümde hayır.** Program tek bilgisayarda, yerel veritabanıyla çalışır. Ağ
üzerinden çok kullanıcılı erişim, ortak veritabanı sunucusu veya bulut senkronizasyonu
yoktur. Veritabanı dosyasını ağ sürücüsüne koymak desteklenmez ve veri bozulmasına
yol açabilir.

### 15. Excel'e nasıl aktarırım?

İlgili liste veya raporda **CSV dışa aktar** (tahsilat/ödemelerde
**Tahsilat/ödemeleri CSV**) düğmesine tıklayın. Dosya:

- Noktalı virgül (`;`) ayırıcılı,
- UTF-8 BOM kodlamalı,
- Ondalık ayırıcısı virgül (`,`) olacak şekilde

kaydedilir. Excel'de çift tıklayarak açabilirsiniz. Türkçe karakterler bozuk
görünürse Excel'de **Veri > Metinden/CSV'den** ile **UTF-8** kodlamasını seçin.

### 16. Excel'den nasıl içe aktarırım?

İçe aktarma yalnızca **cari hesaplar** ve **ürünler** için vardır ve yalnızca
**yönetici** yapabilir.

1. İlgili listede **Şablon indir** ile `cari_sablon.csv` veya `urun_sablon.csv`
   dosyasını alın.
2. Şablonu Excel'de doldurun, **CSV UTF-8** olarak kaydedin.
3. **CSV içe aktar** ile dosyayı seçin.
4. Önizlemede satır durumlarını kontrol edin.
5. Onaylayın.

Hatalı satır varsa **hiçbir kayıt eklenmez**; sonuç penceresinde atlanan satırlar
ve nedenleri gösterilir. Fatura, ödeme veya stok hareketi içe aktarılamaz.

### 17. Şifremi unuttum, ne yapmalıyım?

Sırayla deneyin:

1. **Başka bir yönetici hesabı** varsa onunla giriş yapıp **Ayarlar > Kullanıcılar
   > İşlemler > Şifre sıfırla** ile şifrenizi yenileyin.
2. Başka yönetici yoksa:
   - Programı kapatın.
   - Veritabanının bir kopyasını alın (güvenlik için).
   - Bir SQLite aracıyla (ör. DB Browser for SQLite) veritabanını açın ve
     `users` tablosundaki kayıtları silin:
     `DELETE FROM users;`
   - Değişikliği kaydedip programı açın. Program **"İlk yönetici oluştur"**
     ekranını gösterir; yeni bir yönetici hesabı oluşturun.
   - `users` tablosu silinir; cari, ürün, fatura, ödeme ve diğer iş kayıtları
     **silinmez**.

> Bu yöntem, veritabanına erişimi olan herkese yönetici yetkisi verir. Bilgisayarı
> fiziksel olarak koruyun ve yedekleri güvenli tutun.

### 18. Güncelleme nasıl yapılır?

1. GitHub Releases sayfasından yeni sürümün `OnMuhasebe-<sürüm>.msi` dosyasını
   indirin.
2. Programı kapatın.
3. MSI dosyasına çift tıklayın; SmartScreen uyarısında **Ek bilgi > Yine de
   çalıştır**.
4. Sihirbazı tamamlayın.

Eski sürüm otomatik kaldırılır, yeni sürüm kurulur, veriler korunur. Ayrıntılar:
[KURULUM.md](KURULUM.md#6-güncelleme).

### 19. Program çöktü, ne yapmalıyım?

1. Programı yeniden açın. Açılıyorsa verileriniz büyük olasılıkla yerindedir.
2. Açılmıyorsa **Görev Yöneticisi**'nde `OnMuhasebe.exe` sürecini kapatın ve tekrar
   deneyin.
3. **Yardım > Log klasörünü aç** ile günlük dosyalarını bulun; son dosyayı saklayın.
4. **Yardım > Hata ayrıntıları > Bilgileri kopyala** ile hata metnini kopyalayın.
5. Sorun sürüyorsa yedekten geri yükleyin (bkz. [KURULUM.md](KURULUM.md#8-yedekten-geri-yükleme))
   ve GitHub'da **Issues** bölümünde kayıt açın.

### 20. Windows "bilinmeyen yayıncı" diyor, program güvenli mi?

Program **kod imzalama sertifikasıyla imzalanmamıştır**; bu nedenle SmartScreen
uyarısı normaldir. Kaynak kodu açıktır ve incelenebilir:
<https://github.com/furcanomer/OnMuhasebe>.

Güvenlik için:

- Kurulum dosyasını **yalnızca** projenin GitHub Releases sayfasından indirin.
- E-posta veya üçüncü taraf sitelerden gelen MSI dosyalarını çalıştırmayın.
- Şüphe duyuyorsanız kurulumu iptal edin.

### 21. Verilerimin şifrelenmesi gerekli mi?

Veritabanı **şifrelenmemiştir**; dosyayı okuyabilen biri muhasebe kayıtlarını
görebilir. Öneriler:

- Windows oturum şifresi kullanın.
- Bilgisayarı fiziksel olarak koruyun.
- Hassas veriler için **BitLocker** (veya benzeri disk şifreleme) etkinleştirin.
- Yedekleri şifreli bir diskte veya güvenli bir konumda tutun.

Kullanıcı şifreleri düz metin saklanmaz (Argon2id ile özetlenir), ancak bu durum
muhasebe kayıtlarını şifrelemez.

### 22. Otomatik yedek ne sıklıkta alınır?

Program her açılışta, **son otomatik yedekten 24 saat geçtiyse** yeni bir otomatik
yedek alır. Varsayılan rotasyon: son **30 günlük**, **12 aylık** ve **5 yıllık**
yedek tutulur. Manuel yedekler rotasyona dâhil değildir.

### 23. Faturaya vade tarihi girdim, gecikme faizi hesaplanıyor mu?

**Hayır.** Vade tarihi yalnızca bilgi amaçlı saklanır. Program gecikme faizi,
otomatik hatırlatma veya vade takibi yapmaz. Fatura listesinde **kalan** tutarı ve
ödeme durumunu takip edebilirsiniz.

### 24. Kasa bakiyesi eksiye düşebilir mi?

**Evet.** Kasa/banka bakiyesi eksiye düşebilir; program engellemez. Bu, örneğin
önce ödeme yapıp sonra tahsilat girdiğinizde görülebilir. Bakiye her zaman
kesinleşmiş hareketlerden hesaplanır.

---

Son güncelleme: 2026-10-27
