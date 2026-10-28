# On Muhasebe — Sorun Giderme

Bu belge, programı kullanırken karşılaşılan yaygın sorunları **belirti**, **neden**
ve **çözüm** başlıkları altında açıklar. Önce ilgili senaryoyu bulun; çözüm
adımlarını sırayla uygulayın.

> **Altın kural:** Bir sorunu çözmeye başlamadan önce mümkünse
> **Dosya > Tüm veriyi dışa aktar...** ile yedek alın. Veritabanı dosyasını elle
> düzenlemek son çaredir.

## İçindekiler

1. [Giriş ve şifre sorunları](#1-giriş-ve-şifre-sorunları)
2. [Veritabanı ve dosya sorunları](#2-veritabanı-ve-dosya-sorunları)
3. [Fatura ve ödeme sorunları](#3-fatura-ve-ödeme-sorunları)
4. [Stok ve bakiye sorunları](#4-stok-ve-bakiye-sorunları)
5. [Yedekleme ve geri yükleme sorunları](#5-yedekleme-ve-geri-yükleme-sorunları)
6. [Kurulum, güncelleme ve kaldırma](#6-kurulum-güncelleme-ve-kaldırma)
7. [Görüntü, yazı ve CSV sorunları](#7-görüntü-yazı-ve-csv-sorunları)
8. [Başarım ve donma](#8-başarım-ve-donma)
9. [Günlükler ve destek](#9-günlükler-ve-destek)

---

## 1. Giriş ve şifre sorunları

### 1.1. "Kullanıcı adı veya şifre hatalı."

**Belirti:** Giriş ekranında doğru bilgileri girdiğinizi düşündüğünüz hâlde bu
mesaj görünür.

**Neden:**

- Şifre **büyük/küçük harf duyarlıdır**; Caps Lock açık olabilir.
- Klavye dili farklı olabilir (`i/ı`, `q/w`, `z/y` tuşları).
- Kullanıcı adında baştaki/sondaki boşluk olabilir.
- Şifre gerçekten farklıdır (ör. ilk kurulumda yanlış yazılmıştır).

**Çözüm:**

1. Caps Lock'u kapatın, klavye dilini kontrol edin (Windows + Boşluk).
2. Kullanıcı adını ve şifreyi elle, dikkatlice yeniden yazın.
3. Başka bir yönetici hesabı varsa onunla giriş yapıp **Ayarlar > Kullanıcılar >
   İşlemler > Şifre sıfırla** ile şifreyi yenileyin.
4. Başka yönetici yoksa [SSS.md](SSS.md) belgesindeki "Şifremi unuttum" maddesini
   uygulayın.

### 1.2. "Çok fazla hatalı giriş denemesi. 30 saniye sonra tekrar deneyin."

**Belirti:** Giriş düğmesi çalışmıyor; ekranda geri sayım görünüyor.

**Neden:** Aynı kullanıcı adı için **5 hatalı denemeden** sonra giriş 30 saniye
kilitlenir. Sayaç yalnızca bellekte tutulur.

**Çözüm:**

1. 30 saniye bekleyin; süre dolunca tekrar deneyin.
2. Acele ediyorsanız programı kapatıp yeniden açın; sayaç sıfırlanır.
3. Şifreyi doğru yazdığınızdan emin olun (bkz. 1.1).

### 1.3. "Bu kullanıcı hesabı pasif durumda."

**Belirti:** Doğru şifreyle giriş yapılamıyor.

**Neden:** Hesap bir yönetici tarafından **pasife alınmıştır**.

**Çözüm:** Bir yönetici hesabıyla giriş yapıp **Ayarlar > Kullanıcılar >
İşlemler > Aktifleştir** ile hesabı yeniden açın.

### 1.4. "Son aktif yönetici silinemez, pasife alınamaz veya rolü düşürülemez."

**Belirti:** Kullanıcılar sekmesinde işlem reddediliyor.

**Neden:** Sistemde en az bir aktif yönetici kalmalıdır. Son aktif yönetici
üzerinde bu işlemler yapılamaz.

**Çözüm:** Önce başka bir kullanıcıyı **Yönetici** rolüne yükseltin veya yeni bir
yönetici oluşturun; sonra istediğiniz işlemi yapın.

### 1.5. "Kendi hesabınız üzerinde bu işlemi yapamazsınız."

**Belirti:** Kendi hesabınızı pasife almak, silmek veya rolünüzü değiştirmek
istiyorsunuz.

**Neden:** Program, kendi hesabınızı kilitleyip erişimi kaybetmenizi engeller.

**Çözüm:** Bu işlemleri başka bir yönetici hesabından yapın. Kendi şifrenizi
**Ayarlar > Güvenlik** sekmesinden değiştirebilirsiniz.

### 1.6. Oturum sürekli kilitleniyor

**Belirti:** Kısa süre işlem yapmayınca "Oturum kilitlendi" ekranı geliyor.

**Neden:** Otomatik kilit açıktır ve süre kısa seçilmiştir (varsayılan 15 dakika).

**Çözüm:**

1. **Ayarlar > Genel > Kilit süresi** ile 30 veya 60 dakika seçin.
2. Tamamen kapatmak için **Hareketsizlik sonrası otomatik kilit** onay kutusunu
   kaldırın.
3. **Ayarları kaydet** düğmesine basın.

---

## 2. Veritabanı ve dosya sorunları

### 2.1. "Veritabanı şu an kullanımda. Birkaç saniye sonra tekrar deneyin."

**Belirti:** Kaydetme veya listeleme sırasında bu mesaj çıkıyor.

**Neden:** Veritabanı dosyası başka bir işlem tarafından kilitlenmiştir. Genellikle
programın ikinci bir kopyası açıktır veya bir yedekleme/antivirüs taraması dosyayı
tutmaktadır.

**Çözüm:**

1. **Görev Yöneticisi**'ni açın (Ctrl+Shift+Esc).
2. **Ayrıntılar** sekmesinde birden fazla `OnMuhasebe.exe` varsa fazlalarını kapatın.
3. Birkaç saniye bekleyip tekrar deneyin.
4. Sürerse programı kapatıp yeniden açın.
5. Veritabanı dosyasını hiçbir zaman ağ sürücüsüne veya senkronizasyon klasörüne
   (OneDrive, Dropbox vb.) koymayın.

### 2.2. "Disk dolu. Yer açıp tekrar deneyin."

**Belirti:** Kayıt veya yedekleme başarısız; disk uyarısı görünüyor.

**Neden:** Veri diskinin boş alanı bitmiştir.

**Çözüm:**

1. Windows **Ayarlar > Sistem > Depolama** ile yer açın.
2. **Ayarlar > Yedekleme > Son yedekler** listesinden eski **manuel** yedekleri
   silin (otomatik yedekler rotasyonla zaten sınırlıdır).
3. `%LOCALAPPDATA%\OnMuhasebe\OnMuhasebe\logs` klasöründeki eski günlükleri
   temizleyin (30 günden eskiler zaten otomatik silinir).
4. Yer açtıktan sonra işlemi tekrarlayın.

### 2.3. "Disk okuma/yazma hatası. Diskin bağlı olduğundan emin olun."

**Belirti:** Kaydetme sırasında disk hatası.

**Neden:** Disk bağlantısı kopmuş, disk arızalı veya dosya sistemi bozulmuş
olabilir.

**Çözüm:**

1. Diskin bağlı olduğunu ve sağlıklı çalıştığını kontrol edin.
2. Programı kapatın; veritabanı klasörünü başka bir diske kopyalayın.
3. Windows `chkdsk` ile diski denetleyin.
4. Sorun sürerse yedekten geri yükleyin ve diski değiştirin.

### 2.4. "Veritabanı salt okunur durumda. Dosya izinlerini kontrol edin."

**Belirti:** Hiçbir kayıt yazılamıyor.

**Neden:** Veritabanı dosyası veya klasörü salt okunur işaretlenmiş ya da kullanıcı
hesabının yazma izni yok.

**Çözüm:**

1. Programı kapatın.
2. `%LOCALAPPDATA%\OnMuhasebe\OnMuhasebe\data` klasörüne sağ tıklayın >
   **Özellikler** > **Salt okunur** işaretini kaldırın.
3. **Güvenlik** sekmesinde kullanıcı hesabınıza **Yazma** izni verin.
4. Programı yeniden açın.

### 2.5. "Veritabanı dosyası bozuk. Yedekten geri yükleyin."

**Belirti:** Program açılışta hata veriyor; veriler okunamıyor.

**Neden:** Veritabanı dosyası bozulmuştur (elektrik kesintisi, disk hatası, dosyanın
elle değiştirilmesi).

**Çözüm:**

1. Programı kapatın.
2. Bozuk dosyayı **silmeyin**; `on_muhasebe.db.bozuk-<tarih>` olarak yeniden
   adlandırıp saklayın.
3. `backups` klasöründeki en yeni sağlam yedeği kullanarak geri yükleme yapın
   (bkz. 5.4).
4. Destek için bozuk dosyayı ve günlükleri saklayın.

### 2.6. "Veritabanı şeması bu sürümden daha yeni… Lütfen uygulamayı güncelleyin."

**Belirti:** Program açılmıyor; şema sürümü uyarısı var.

**Neden:** Veritabanı, programın **daha yeni** bir sürümüyle oluşturulmuş veya
güncellenmiştir. Eski sürüm, yeni şemayı okuyamaz.

**Çözüm:**

1. Programı **kapatmayın**, veritabanını silmeyin.
2. En son sürümü GitHub Releases sayfasından indirip kurun.
3. Yeni sürüm veritabanını otomatik olarak açar.

### 2.7. "Veritabanı migration zinciri bozuk: vX kaydı eksik."

**Belirti:** Açılışta migration hatası; program veritabanını açmıyor.

**Neden:** `schema_migrations` tablosundaki sürüm kayıtlarından biri eksik veya
elle değiştirilmiş.

**Çözüm:**

1. Programı kapatın.
2. En yeni sağlam yedeği geri yükleyin (bkz. 5.4).
3. Yedek yoksa veritabanı dosyasını ve günlükleri saklayıp GitHub'da destek isteyin.
4. Veritabanı dosyasını elle düzenlemeye çalışmayın; bu durum veri kaybına yol
   açabilir.

---

## 3. Fatura ve ödeme sorunları

### 3.1. "Bu faturaya bağlı tahsilat/ödeme bulunuyor. Önce ilgili ödemeleri iptal edin."

**Belirti:** Fatura iptal edilemiyor.

**Neden:** Faturaya kesin durumda tahsilat/ödeme dağıtılmıştır. Ödenmiş fatura
doğrudan iptal edilemez.

**Çözüm:**

1. **Kasa ve Banka > Hareketler** bölümüne gidin.
2. İlgili tahsilat/ödeme kaydını bulun (tarih, cari veya tutarla arayın).
3. **Ayrıntı > İptal et** ile iptal edin; **neden** yazın.
4. Faturaya dönüp **İptal Et** ile iptal edin.

### 3.2. "Bu fatura zaten iptal edilmiş."

**Belirti:** İptal düğmesi hata veriyor.

**Neden:** Fatura daha önce iptal edilmiştir.

**Çözüm:** Listeyi `F5` ile yenileyin. Fatura "İptal" durumundadır; yeniden iptal
gerekmez. Düzeltme için yeni fatura oluşturun.

### 3.3. "Bu tahsilat/ödeme zaten iptal edilmiş."

**Belirti:** Ödeme iptal edilemiyor.

**Neden:** Kayıt daha önce iptal edilmiştir.

**Çözüm:** Listeyi yenileyin; kayıt "İptal" durumundadır. Gerekirse yeni bir
tahsilat/ödeme girin.

### 3.4. "Dağıtım tutarı faturanın kalanını aşıyor."

**Belirti:** Tahsilat/ödeme kaydedilemiyor.

**Neden:** Bir faturaya, kalan tutarından fazla dağıtım girilmiştir.

**Çözüm:**

1. Dağıtım panelindeki **Dağıtılacak** alanını küçültün.
2. **Tümünü dağıt** düğmesi kalanların tamamını doğru şekilde doldurur.
3. Kalan tutarın üzerindeki fark avans olarak kalabilir; bu normaldir.

### 3.5. "Tahsilat/ödeme tutarı sıfırdan büyük olmalıdır."

**Belirti:** Kaydetme reddediliyor.

**Neden:** Tutar boş, sıfır veya negatiftir.

**Çözüm:** Sıfırdan büyük bir tutar girin. Ondalık ayırıcı olarak **virgül**
kullanın (ör. `1.250,75`).

### 3.6. "İptal edilmiş faturaya tahsilat/ödeme dağıtılamaz."

**Belirti:** Dağıtım listesinde fatura seçilemiyor veya kayıt reddediliyor.

**Neden:** Seçilen fatura iptal edilmiştir.

**Çözüm:** İptal edilmiş faturaya ödeme bağlanamaz. Doğru faturayı seçin veya
faturayı yeniden oluşturun.

### 3.7. "Seçilen cariye ait olmayan faturaya dağıtım yapılamaz."

**Belirti:** Dağıtım reddediliyor.

**Neden:** Fatura başka bir cariye aittir.

**Çözüm:** Ödeme formundaki cariyi kontrol edin; yalnızca aynı cariye ait faturalar
listelenir. Yanlış cari seçilmişse değiştirin.

### 3.8. "Faturayı iptal ettim ama cari bakiyesi değişmedi."

**Belirti:** İptalden sonra bakiye aynı görünüyor.

**Neden:** Ekran yenilenmemiştir veya iptal işlemi tamamlanmamıştır.

**Çözüm:**

1. `F5` ile listeyi yenileyin.
2. Faturayı açıp durumunun **İptal** olduğunu doğrulayın.
3. Cari ayrıntısındaki bakiyeyi kontrol edin; iptal edilen fatura bakiyeye
   katılmaz.

---

## 4. Stok ve bakiye sorunları

### 4.1. "Stok eksiye düşecek… Devam edilsin mi?"

**Belirti:** Satış faturası veya stok çıkışı kaydederken onay isteniyor.

**Neden:** İşlem, ürünün mevcut stoğundan fazlasını düşürecektir.

**Çözüm:**

- Doğruysa **Onayla** ile devam edin; stok negatif görünür.
- Yanlışsa **Vazgeç** ile iptal edin; hiçbir kayıt oluşmaz.
- Önce **Giriş** hareketiyle stoğu düzeltmek daha sağlıklıdır.

### 4.2. "Stok hareketini silmek/düzenlemek istiyorum."

**Belirti:** Hareket listesinde silme veya düzenleme düğmesi yok.

**Neden:** İzlenebilirlik için hareketler silinemez ve düzenlenemez.

**Çözüm:** Ters yönlü yeni bir **Düzeltme** hareketi girin (açıklama zorunludur).
Örneğin yanlışlıkla 10 adet çıkış yapıldıysa 10 adet **Giriş** veya uygun yönde
**Düzeltme** girin.

### 4.3. "Ürün stoğu beklediğimden farklı."

**Belirti:** Güncel stok, elle hesapladığınız değerle uyuşmuyor.

**Neden:** Stok; başlangıç stoğu, stok hareketleri ve **kesinleşmiş faturaların**
toplamıdır. İptal edilen faturalar ters hareket üretir.

**Çözüm:**

1. Ürün ayrıntısındaki **Stok hareketleri** listesini inceleyin.
2. Fatura kaynaklı hareketleri (`alis_faturasi`, `satis_faturasi`) kontrol edin.
3. Fark varsa **Düzeltme** hareketiyle gerçek sayımı yansıtın.

### 4.4. "Cari bakiyesi yanlış görünüyor."

**Belirti:** Cari bakiyesi beklediğiniz tutarla uyuşmuyor.

**Neden:** Bakiye; kesinleşmiş satış − alış − tahsilat + ödeme formülüyle
hesaplanır. İptaller hesaba katılmaz.

**Çözüm:**

1. Cari ayrıntısında **Faturaları gör** ve **Ekstresini gör** ile hareketleri
   inceleyin.
2. **Raporlar > Cari Ekstre** ile tarih aralığı vererek borç/alacak dökümünü alın.
3. İptal edilmiş bir kaydın bakiyeye katılmadığını unutmayın.

### 4.5. "Kasa bakiyesi eksi görünüyor."

**Belirti:** Kasa/banka hesabında negatif bakiye var.

**Neden:** Ödemeler tahsilatlardan fazladır. Program negatif bakiyeyi engellemez.

**Çözüm:** Gerçek durumu yansıtıyorsa bir şey yapmayın. Değilse ilgili tahsilat/
ödeme kaydını inceleyip yanlış olanı iptal edin ve doğrusunu girin.

### 4.6. "Açılış bakiyesini değiştiremiyorum."

**Belirti:** Hesap formunda açılış bakiyesi alanı salt okunur.

**Neden:** Hesapta hareket vardır. Açılış bakiyesi yalnızca yeni hesapta veya hiç
hareketi olmayan hesapta değiştirilebilir.

**Çözüm:** Açılış bakiyesi düzeltmesi için ters yönlü bir tahsilat/ödeme veya
gelir/gider kaydı girin; geçmişi değiştirmeyin.

---

## 5. Yedekleme ve geri yükleme sorunları

### 5.1. "Otomatik yedek alınamadı. Ayarlar > Yedekleme'den manuel deneyin."

**Belirti:** Açılışta uyarı görünüyor.

**Neden:** Disk dolu, `backups` klasörüne yazma izni yok veya veritabanı kilitli.

**Çözüm:**

1. Disk alanını kontrol edin.
2. `%LOCALAPPDATA%\OnMuhasebe\OnMuhasebe\backups` klasörünün var olduğunu ve yazma
   izni olduğunu doğrulayın.
3. **Ayarlar > Yedekleme > Şimdi yedek al** ile manuel deneyin.
4. Antivirüsün klasörü engellemediğinden emin olun.

### 5.2. "Yedek doğrulanamadı."

**Belirti:** Yedek oluşturulurken veya geri yüklenirken doğrulama hatası.

**Neden:** Dosya eksik/bozuk kopyalanmış, yarıda kalmış veya disk hatası olmuş.

**Çözüm:**

1. Başka bir yedek dosyası deneyin (önceki günün otomatik yedeği).
2. Dosyayı USB'ye kopyalarken işlemin tamamlandığından emin olun.
3. Yedek dosyasını açmaya/çıkarmaya çalışmayın.
4. Tüm yedekler bozuksa destek için günlükleri saklayın.

### 5.3. "Bu yedek, programın daha yeni bir sürümüyle oluşturulmuş. Önce programı güncelleyin."

**Belirti:** Geri yükleme reddediliyor.

**Neden:** Yedek, kurulu sürümden daha yeni bir şema sürümü içeriyor.

**Çözüm:** Programı en son sürüme güncelleyin, sonra geri yükleyin.

### 5.4. "Bu dosya bir OnMuhasebe yedeği değil."

**Belirti:** Geri yükleme reddediliyor.

**Neden:** Seçilen dosya `.onm-backup` değildir, bozuktur veya başka bir programın
dosyasıdır.

**Çözüm:**

1. **Dosya > Tüm veriyi dışa aktar...** ile alınmış bir dosya kullanın.
2. Dosya uzantısının `.onm-backup` olduğunu doğrulayın.
3. CSV dosyaları yedek değildir; geri yüklenemez.

### 5.5. "Yarıda kalan geri yükleme" ekranı geldi.

**Belirti:** Program açılışta üç seçenekli bir uyarı gösteriyor.

**Neden:** Önceki geri yükleme işlemi tamamlanmadan program kapanmıştır (elektrik
kesintisi, çökme).

**Çözüm:**

- **Güvenlik yedeğine dön** — geri yükleme öncesi duruma döner (önerilen).
- **Mevcut veritabanıyla devam et** — o anki dosyayla devam eder; verilerin
  tutarlılığını kontrol edin.
- **Uygulamayı kapat** — hiçbir değişiklik yapmadan çıkar.

Karar verdikten sonra programı açıp verilerinizi kontrol edin.

### 5.6. "Yedeği geri yükledim ama eski veriler görünüyor."

**Belirti:** Geri yükleme sonrası beklenen kayıtlar yok.

**Neden:** Yanlış yedek dosyası seçilmiş olabilir veya geri yükleme tamamlanmamıştır.

**Çözüm:**

1. Geri yükleme ekranında gösterilen **yedek tarihini** kontrol edin.
2. Doğru tarihli `.onm-backup` dosyasını seçin.
3. Geri yükleme sonrası programın veritabanını yeniden açtığından emin olun.
4. Gerekirse güvenlik yedeğine dönüp tekrar deneyin.

---

## 6. Kurulum, güncelleme ve kaldırma

### 6.1. Program açılmıyor / hiçbir pencere görünmüyor

**Belirti:** Kısayola tıklıyorsunuz, pencere açılmıyor.

**Neden:** Önceki süreç arka planda takılı kalmış, veritabanı kilitli veya kurulum
bozulmuş olabilir.

**Çözüm:**

1. **Görev Yöneticisi > Ayrıntılar**'da `OnMuhasebe.exe` varsa **Görevi sonlandır**.
2. Programı yeniden açın.
3. Açılmazsa **Yardım > Log klasörünü aç** yerine
   `%LOCALAPPDATA%\OnMuhasebe\OnMuhasebe\logs` klasörünü elle açıp son günlüğü
   inceleyin.
4. Programı kaldırıp yeniden kurun; veriler korunur.

### 6.2. Windows "bilinmeyen yayıncı" uyarısı veriyor

**Belirti:** Kurulumda SmartScreen uyarısı.

**Neden:** Program kod imzalama sertifikasıyla imzalanmamıştır.

**Çözüm:** **Ek bilgi > Yine de çalıştır**. Dosyayı yalnızca
<https://github.com/furcanomer/OnMuhasebe/releases> adresinden indirdiğinizden emin
olun.

### 6.3. "Daha yeni bir sürüm zaten kurulu. Yükseltme yapılamaz."

**Belirti:** Eski sürümün MSI dosyası çalıştırılamıyor.

**Neden:** Windows Installer sürüm düşürmeye izin vermez.

**Çözüm:** Önce mevcut sürümü kaldırın, sonra eski sürümü kurun. Verileriniz için
önce yedek alın. Genellikle en son sürümü kullanmanız önerilir.

### 6.4. Programlar listesinde iki "On Muhasebe" girişi var

**Belirti:** Ayarlar > Uygulamalar listesinde iki kayıt görünüyor.

**Neden:** Çok eski veya farklı yöntemle yapılmış bir kurulumdan kalan ikinci kayıt.

**Çözüm:**

1. İki girişi de kaldırın.
2. `C:\Program Files\OnMuhasebe` klasörünün silindiğini doğrulayın.
3. Güncel MSI dosyasını yeniden kurun.
4. Bu sürümde (1.0.0 ve sonrası) yükseltme otomatiktir; tek giriş kalır.

### 6.5. Güncelleme sonrası verilerim kayboldu

**Belirti:** Yeni sürümde cariler/faturalar boş görünüyor.

**Neden:** Program farklı bir Windows kullanıcı hesabıyla açılmış olabilir;
`%LOCALAPPDATA%` kullanıcıya özeldir.

**Çözüm:**

1. Programı **eski Windows kullanıcı hesabıyla** açın.
2. Veritabanının `%LOCALAPPDATA%\OnMuhasebe\OnMuhasebe\data` altında olduğunu
   doğrulayın.
3. Gerekirse eski profildeki `.onm-backup` dosyasını yeni profilde geri yükleyin.

---

## 7. Görüntü, yazı ve CSV sorunları

### 7.1. Türkçe karakterler bozuk görünüyor (Excel'de CSV)

**Belirti:** CSV dosyasında `ş`, `ğ`, `İ` gibi harfler yanlış görünüyor.

**Neden:** Excel dosyayı farklı kodlamayla açmıştır. Program UTF-8 BOM yazar; bazı
Excel sürümleri yine de sorabilir.

**Çözüm:**

1. Excel'de **Veri > Metinden/CSV'den** ile dosyayı açın.
2. Dosya kaynağı olarak **UTF-8** seçin, ayırıcı olarak **noktalı virgül** (`;`)
   belirtin.
3. Alternatif: Dosyayı Not Defteri'nde açıp **Farklı Kaydet > UTF-8** ile
   kaydedin.

### 7.2. CSV'de tutarlar metin gibi görünüyor

**Belirti:** Excel'de tutarlar toplanamıyor.

**Neden:** Ondalık ayırıcı virgüldür ve Windows bölge ayarları farklı olabilir.

**Çözüm:** Excel'de hücreleri seçip **Veri > Metni Sütunlara Dönüştür** ile
ondalık ayırıcıyı **virgül** olarak belirtin. Bölge ayarlarınız virgülü ondalık
ayırıcı olarak kullanmıyorsa dosyayı içe aktarırken bu ayarı seçin.

### 7.3. Yazılar çok küçük / arayüz bulanık

**Belirti:** Yüksek DPI ekranda yazılar okunmuyor.

**Neden:** Windows ölçekleme ayarı (%150, %200) ile uygulama ölçeği uyuşmuyor
olabilir.

**Çözüm:**

1. Windows **Ayarlar > Sistem > Ekran > Ölçek** ayarını kontrol edin.
2. Programı kapatıp yeniden açın (ölçek değişikliği yeniden başlatma gerektirir).
3. Pencereyi büyütün; arayüz yeniden boyutlandırmaya uyumludur.

### 7.4. CSV içe aktarmada "Hiçbir kayıt eklenmedi."

**Belirti:** İçe aktarma sonucunda hiçbir satır eklenmemiş.

**Neden:** Satırlardan en az biri hatalıdır; içe aktarma **tümü ya da hiçbiri**
mantığıyla çalışır. Ayrıca kodlar zaten kullanılıyor olabilir.

**Çözüm:**

1. Sonuç penceresindeki **Atlama sebebi** sütununu okuyun.
2. Şablon başlıklarının **aynen** korunduğunu doğrulayın.
3. Zorunlu alanları (`code`, `name`) doldurun.
4. Var olan kodları değiştirin veya o satırları silin.
5. `kind`, `unit`, `active` değerlerinin geçerli olduğundan emin olun.
6. Düzeltip yeniden içe aktarın.

### 7.5. CSV içe aktarma düğmesi görünmüyor

**Belirti:** Listede **CSV içe aktar** yok.

**Neden:** İçe aktarma yalnızca **yönetici** rolüne açıktır.

**Çözüm:** Yönetici hesabıyla giriş yapın. Operatör rolüyle içe aktarma yapılamaz.

---

## 8. Başarım ve donma

### 8.1. Arama yavaş veya ekran kısa süre donuyor

**Belirti:** Büyük veri kümelerinde arama yaparken kısa takılmalar.

**Neden:** Arama büyük tablolarda çalışır; listeler sanallaştırılmış olsa da
veritabanı sorgusu zaman alabilir.

**Çözüm:**

1. Aramayı daha spesifik yapın (kod veya ada göre).
2. Filtreleri daraltın (tarih, tür, durum).
3. Arama sürerken aynı anda başka işlem başlatmayın.
4. Sürekli donma varsa günlükleri saklayıp destek isteyin.

### 8.2. Program yanıt vermiyor

**Belirti:** Pencere "Yanıt vermiyor" durumuna geçiyor.

**Neden:** Uzun süren bir veritabanı işlemi, kilitlenme veya disk sorunu.

**Çözüm:**

1. Birkaç saniye bekleyin; işlem tamamlanabilir.
2. Uzun süre düzelmiyorsa **Görev Yöneticisi**'nden kapatın.
3. Programı yeniden açın; verileriniz büyük olasılıkla yerindedir (işlemler tek
   parçadır).
4. Tekrarlayan donmalarda günlükleri ve yaptığınız işlemi bildirin.

### 8.3. Liste çok kalabalık, kaydı bulamıyorum

**Belirti:** Binlerce kayıt arasında arama zor.

**Çözüm:** Arama kutusunu ve filtreleri kullanın; `Ctrl+K` genel araması türler
arasında hızlı sonuç verir. Sütun başlıklarına tıklayarak sıralayın (desteklenen
listelerde).

---

## 9. Günlükler ve destek

### 9.1. Log dosyası çok büyük

**Belirti:** `logs` klasörü beklenenden fazla yer kaplıyor.

**Neden:** Ayrıntılı (`Info`) günlük seviyesi uzun süre açık kalmıştır.

**Çözüm:**

1. **Ayarlar > Genel > Log seviyesi** = **Warn (varsayılan)**.
2. **Ayarları kaydet**.
3. 30 günden eski dosyalar otomatik silinir; acil yer gerekiyorsa eski dosyaları
   elle silin.

### 9.2. Destek için hangi bilgiler gerekli?

Aşağıdakileri toplayın:

1. Program sürümü (**Yardım > Hakkında** veya Genel Bakış > Program bilgisi).
2. Windows sürümü (Win + R > `winver`).
3. Sorunun oluştuğu tarih/saat ve yaptığınız işlem.
4. Hata metni: **Yardım > Hata ayrıntıları > Bilgileri kopyala**.
5. İlgili günlük dosyası: **Yardım > Log klasörünü aç**.
6. Mümkünse ekran görüntüsü.

Bunları GitHub **Issues** bölümünde paylaşın:
<https://github.com/furcanomer/OnMuhasebe/issues>

> Gerçek müşteri adları, vergi numaraları ve tutarlar gibi hassas bilgileri
> paylaşmayın; örnek verilerle anlatın.

### 9.3. Veritabanını elle düzenlemeli miyim?

**Genellikle hayır.** Veritabanını elle düzenlemek; bakiye, stok ve rapor
tutarsızlığına yol açabilir. Yalnızca şu durumlarda ve **yedek aldıktan sonra**
düşünün:

- Şifre kurtarma (yalnızca `users` tablosu; bkz. [SSS.md](SSS.md)).
- Destek ekibinin açıkça istediği bir işlem.

İş kayıtlarını (fatura, ödeme, stok) doğrudan SQL ile değiştirmeyin; program
üzerinden iptal/düzeltme yapın.

---

Son güncelleme: 2026-10-27
