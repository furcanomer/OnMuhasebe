# On Muhasebe — Kurulum Rehberi

Bu belge, **On Muhasebe** programının Windows bilgisayara kurulumunu, ilk açılışını,
güncellenmesini ve kaldırılmasını adım adım anlatır. Programı günlük olarak kullanacak
kişiler için yazılmıştır; teknik bilgi gerektirmez.

> Kısa özet: MSI dosyasına çift tıklayın, sihirbazı ilerletin, ilk yönetici hesabını
> oluşturun. Verileriniz bilgisayarınızda kalır, hiçbir sunucuya gönderilmez.

## İçindekiler

1. [Sistem gereksinimleri](#1-sistem-gereksinimleri)
2. [İndirme](#2-i̇ndirme)
3. [Kurulum adımları](#3-kurulum-adımları)
4. [İlk açılış](#4-i̇lk-açılış)
5. [Kurulum sonrası: dosyalar nerede?](#5-kurulum-sonrası-dosyalar-nerede)
6. [Güncelleme](#6-güncelleme)
7. [Kaldırma](#7-kaldırma)
8. [Yedekten geri yükleme](#8-yedekten-geri-yükleme)
9. [Güvenlik ve önemli uyarılar](#9-güvenlik-ve-önemli-uyarılar)

---

## 1. Sistem gereksinimleri

| Gereksinim | Değer |
|------------|-------|
| İşletim sistemi | Windows 10 64-bit veya üzeri (Windows 11 dâhil) |
| Mimari | x64 (64-bit) |
| Boş disk alanı | En az 200 MB (veriler ve yedekler için ek yer önerilir) |
| Bellek (RAM) | En az 2 GB |
| Ekran | 1180 × 760 piksel veya üzeri önerilir; pencere yeniden boyutlandırılabilir |
| Ek yazılım | Gerekmez. SQLite programın içine gömülüdür; ayrı veritabanı kurulumu yoktur. |
| İnternet | Kurulum dosyasını indirmek dışında gerekmez; program çevrimdışı çalışır. |

Program tek bilgisayarda, yerel masaüstü uygulaması olarak çalışır. Web tarayıcısı,
sunucu, bulut hesabı veya ek servis gerekmez.

---

## 2. İndirme

Kurulum dosyası **GitHub Releases** sayfasından indirilir:

<https://github.com/furcanomer/OnMuhasebe/releases>

1. Sayfadaki en son sürümün altında bulunan **Assets** (dosyalar) bölümünü açın.
2. `OnMuhasebe-<sürüm>.msi` dosyasını indirin (örnek: `OnMuhasebe-1.0.0.msi`).
3. İndirilen dosyayı kaydettiğiniz klasörü açın.

> **Not:** Releases sayfasının içeriği yayın aşamasında doldurulur. Bu belgede anlatılan
> dosya adı biçimi `OnMuhasebe-<sürüm>.msi` şeklindedir. Sürüm numarası `Cargo.toml`
> dosyasındaki `[package] version` değerinden gelir.

İndirdiğiniz dosyanın bir **MSI kurulum paketi** olduğunu doğrulayın. `.exe` uzantılı
tanımadığınız dosyaları çalıştırmayın.

---

## 3. Kurulum adımları

### 3.1. Kurulumu başlatma

1. `OnMuhasebe-<sürüm>.msi` dosyasına **çift tıklayın**.
2. Windows **"Windows korumalı bilgisayarınızı korudu"** (SmartScreen) uyarısı
   gösterebilir. Bu, programın dijital olarak imzalanmamış olmasından kaynaklanır.
   - **Ek bilgi** (More info) bağlantısına tıklayın.
   - **Yine de çalıştır** (Run anyway) düğmesine tıklayın.
3. **Kullanıcı Hesabı Denetimi (UAC)** penceresi açılırsa **Evet** ile onaylayın.
   Kurulum makine genelinde (`C:\Program Files`) yapıldığı için yönetici izni gerekir.

### 3.2. Kurulum sihirbazı

Sihirbaz sırasıyla şu ekranları gösterir:

| Adım | Ekran | Yapılacak |
|------|-------|-----------|
| 1 | Hoş geldiniz | **İleri** |
| 2 | Lisans sözleşmesi (BSD 3-Clause) | Sözleşmeyi okuyup **Kabul ediyorum** seçeneğini işaretleyin, **İleri** |
| 3 | Kurulum klasörü | Varsayılan `C:\Program Files\OnMuhasebe` önerilir. Değiştirmek isterseniz **Gözat** ile seçin, **İleri** |
| 4 | Kuruluma hazır | **Kur** |
| 5 | Kurulum ilerlemesi | Bekleyin; birkaç saniye sürer |
| 6 | Tamamlandı | **Son** |

Kurulum tamamlandığında:

- **Masaüstünde** "On Muhasebe" kısayolu oluşur.
- **Başlat menüsünde** "On Muhasebe" klasörü ve içinde program kısayolu ile
  **Kaldır** kısayolu oluşur.
- **Ayarlar > Uygulamalar > Yüklü uygulamalar** (Denetim Masası > Programlar) listesinde
  **On Muhasebe** görünür. Yayıncı: **furcanomer**.

### 3.3. Kurulumda sorun çıkarsa

- "Daha yeni bir sürüm zaten kurulu" mesajı alırsanız bilgisayarda daha yüksek sürümlü
  bir On Muhasebe kuruludur; mevcut sürümü kullanın veya önce onu kaldırın.
- Kurulum yarıda kalırsa tekrar çalıştırın. Ayrıntılar için
  [SORUN_GIDERME.md](SORUN_GIDERME.md) belgesine bakın.

---

## 4. İlk açılış

Programı **Başlat menüsü > On Muhasebe** veya masaüstündeki kısayoldan açın.

İlk açılışta program veri klasörünü ve veritabanını otomatik oluşturur. Ardından
**İlk Yönetici Oluştur** ekranı gelir.

### 4.1. İlk yönetici hesabı

| Alan | Açıklama |
|------|----------|
| Kullanıcı adı | Girişte kullanılacak ad (zorunlu, benzersiz) |
| Görünen ad | İsteğe bağlı; ekranda gösterilecek ad |
| Şifre | Zorunlu |
| Şifre tekrarı | Şifrenin aynısı |

**Şifre kuralları:**

- En az **8 karakter**.
- En az **1 harf** ve **1 rakam** içermelidir.
- Büyük/küçük harf duyarlıdır.

Kurallara uymayan şifrede alanın altında kırmızı hata mesajı görünür. Bilgileri girip
**Yöneticiyi oluştur** düğmesine tıklayın.

> İlk yönetici hesabı oluşturulduktan sonra program bir daha bu ekranı göstermez.
> Şifrenizi not edin. Kurtarma adımları için [SSS.md](SSS.md) belgesine bakın.

### 4.2. Kurulum sihirbazı (4 adım)

İlk yönetici oluşturulduktan sonra **Kurulum sihirbazı** açılır. Sihirbaz, programı
kullanmaya başlamadan önce temel bilgileri tanımlar:

| Adım | Başlık | İçerik |
|------|--------|--------|
| 1 | Şirket bilgileri | Ticari unvan, yetkili kişi, telefon, e-posta, adres, vergi dairesi, vergi numarası |
| 2 | İlk kasa/banka hesabı | Hesap adı, tür (Kasa/Banka), banka adı ve IBAN (banka için), açılış bakiyesi, notlar |
| 3 | Kategori paketleri | Hazır gelir/gider kategorileri; varsayılan olarak tümü seçilidir |
| 4 | Özet | Girilen bilgilerin özeti ve **Tamamla** düğmesi |

- Adımlar arasında **İleri →** ve geri dönerek ilerleyebilirsiniz.
- Tüm alanlar isteğe bağlıdır; boş bırakabilirsiniz.
- **Daha sonra** düğmesiyle sihirbaz atlanabilir. Atlarsanız onay istenir ve hiçbir
  kayıt yazılmaz.
- Sihirbaz tamamlandığında şirket bilgileri, kasa/banka hesabı ve seçilen kategoriler
  **tek bir işlemde** kaydedilir. Bir hata olursa hiçbiri yazılmaz.
- Sihirbazı daha sonra **Ayarlar > Genel > Kurulum sihirbazını yeniden aç** ile
  yeniden çalıştırabilirsiniz (yalnızca yönetici).

### 4.3. Giriş

Sihirbazdan sonra uygulama açılır. Programı her kapatıp açtığınızda **Giriş** ekranı
gelir: kullanıcı adı ve şifre girin, **Giriş yap** düğmesine tıklayın.

- 5 hatalı denemeden sonra giriş **30 saniye** kilitlenir; sayaç ekranda gösterilir.
- Hareketsizlik sonrası otomatik kilit varsayılan olarak **açıktır** ve süre
  **15 dakikadır**. Süre **Ayarlar > Genel**'den 5/15/30/60 dakika olarak değiştirilir.
- Kilit ekranında şifreyi girerek **Kilidi aç** veya **Oturumu kapat** seçilebilir.

---

## 5. Kurulum sonrası: dosyalar nerede?

Program verilerini kullanıcı hesabınızın yerel uygulama verisi klasöründe tutar:

| İçerik | Yol |
|--------|-----|
| Veritabanı | `%LOCALAPPDATA%\OnMuhasebe\OnMuhasebe\data\on_muhasebe.db` |
| Yedekler | `%LOCALAPPDATA%\OnMuhasebe\OnMuhasebe\backups\` |
| Günlükler (log) | `%LOCALAPPDATA%\OnMuhasebe\OnMuhasebe\logs\` |
| Program dosyaları | `C:\Program Files\OnMuhasebe\` |

`%LOCALAPPDATA%` genellikle `C:\Users\<kullanıcı>\AppData\Local` anlamına gelir.
Klasöre hızlıca gitmek için Windows Gezgini adres çubuğuna `%LOCALAPPDATA%\OnMuhasebe`
yazabilirsiniz.

- Veritabanı dosyası ilk açılışta otomatik oluşturulur.
- Aynı klasörde `on_muhasebe.db-wal` ve `on_muhasebe.db-shm` yardımcı dosyaları
  görünebilir; bunlar SQLite'ın normal çalışma dosyalarıdır, **silmeyin**.
- Günlük klasörünü program içinden açmak için **Yardım > Log klasörünü aç**.
- Yedeklerin bir kopyasını başka bir diske almak için bkz.
  [KULLANIM_KILAVUZU.md](KULLANIM_KILAVUZU.md).

---

## 6. Güncelleme

Program, **Windows Installer (MSI)** tabanlı otomatik yükseltme kullanır. Bu davranış
gerçek MSI testiyle doğrulanmıştır (14-5 aşaması).

### 6.1. Güncelleme adımları

1. GitHub Releases sayfasından **yeni sürümün** MSI dosyasını indirin
   (örnek: `OnMuhasebe-1.0.1.msi`).
2. Programı kapatın (açıksa **Dosya > Çıkış** veya `Ctrl+Q`).
3. Yeni MSI dosyasına **çift tıklayın**.
4. SmartScreen uyarısı çıkarsa **Ek bilgi > Yine de çalıştır**.
5. UAC onayı istenirse **Evet**.
6. Sihirbazı tamamlayın (**İleri → Kur → Son**).

### 6.2. Güncellemede ne olur?

- Windows Installer **eski sürümü otomatik olarak kaldırır**, yenisini kurar.
- **Programlar listesinde tek giriş kalır**; sürüm numarası yeni sürümdür.
- **Verileriniz korunur.** Veritabanı dosyası `%LOCALAPPDATA%` altında olduğu için
  kurulum/kaldırma işlemlerinden etkilenmez.
- Ayarlar, kullanıcılar, cariler, ürünler, faturalar, ödemeler ve yedekler aynen kalır.
- Masaüstü ve başlat menüsü kısayolları yenilenir.

### 6.3. Sürüm düşürme

Daha **eski** bir sürümün MSI dosyası çalıştırılırsa Windows Installer
"Daha yeni bir sürüm zaten kurulu. Yükseltme yapılamaz." mesajı verir. Bu durumda
önce mevcut sürümü kaldırıp sonra eski sürümü kurabilirsiniz; ancak verilerinizi
korumak için önce yedek alın.

---

## 7. Kaldırma

### 7.1. Kaldırma adımları

**Yol 1 — Ayarlar:**

1. **Başlat > Ayarlar > Uygulamalar > Yüklü uygulamalar**.
2. Listede **On Muhasebe** (yayıncı: furcanomer) satırını bulun.
3. **Kaldır** düğmesine tıklayın ve onaylayın.

**Yol 2 — Denetim Masası:**

1. **Denetim Masası > Programlar > Programlar ve Özellikler**.
2. **On Muhasebe** > **Kaldır**.

**Yol 3 — Başlat menüsü:**

1. **Başlat > On Muhasebe** klasörünü açın.
2. **Kaldır** kısayoluna tıklayın.

### 7.2. Kaldırınca verilerim silinir mi?

**Hayır.** Kaldırma işlemi yalnızca program dosyalarını (`C:\Program Files\OnMuhasebe`)
ve kısayolları siler. `%LOCALAPPDATA%\OnMuhasebe` klasöründeki veritabanı, yedekler ve
günlükler **korunur**. Programı yeniden kurarsanız verileriniz yerinde olur.

### 7.3. Verileri tamamen silmek

Verileri de silmek istiyorsanız:

1. Programı kaldırın (yukarıdaki adımlar).
2. Windows Gezgini'nde `%LOCALAPPDATA%\OnMuhasebe` klasörünü açın.
3. Klasörü tamamen silin.

> **Uyarı:** Bu işlem geri alınamaz. Silmeden önce yedek almanız önerilir.

---

## 8. Yedekten geri yükleme

Yeni bir bilgisayara geçtiyseniz veya verileri geri döndürmek istiyorsanız:

1. Programı kurun ve açın.
2. İlk yönetici hesabını oluşturun (yeni boş veritabanı için gereklidir).
3. **Dosya > Yedeği geri yükle...** menüsünü açın (yalnızca yönetici).
4. Eski bilgisayardan aldığınız `.onm-backup` dosyasını seçin.
5. Ekranda yedeğin şema sürümü ve tarihi gösterilir; **Yedeği geri yükle** düğmesine
   tıklayın.
6. Onay istenir. Mevcut veritabanınız **otomatik olarak güvenlik yedeğine** alınır
   (`on_muhasebe.db.pre-restore-<zaman>`).
7. Geri yükleme tamamlanınca program veritabanını yeniden açar ve yedekteki tüm
   veriler görünür.

Geri yükleme sırasında program kapanırsa, bir sonraki açılışta **"Yarıda kalan geri
yükleme"** ekranı gelir. Üç seçenek sunulur:

- **Güvenlik yedeğine dön** — geri yükleme öncesi duruma döner (önerilen).
- **Mevcut veritabanıyla devam et** — o anki dosyayla devam eder.
- **Uygulamayı kapat** — hiçbir değişiklik yapmadan çıkar.

Eski bilgisayarda yedek almak için: **Ayarlar > Yedekleme > Şimdi yedek al** veya
**Dosya > Tüm veriyi dışa aktar...**. Ayrıntılar:
[KULLANIM_KILAVUZU.md](KULLANIM_KILAVUZU.md).

---

## 9. Güvenlik ve önemli uyarılar

### 9.1. İmzasız program

Bu programın **kod imzalama sertifikası yoktur**. Bu nedenle Windows SmartScreen
"bilinmeyen yayıncı" uyarısı gösterebilir. Bu beklenen bir durumdur.

- Kaynak kodu açıktır: <https://github.com/furcanomer/OnMuhasebe>
- İndirdiğiniz MSI dosyasının yalnızca bu projenin Releases sayfasından geldiğinden
  emin olun.
- Şüpheli e-posta veya üçüncü taraf sitelerden indirilen kurulum dosyalarını
  çalıştırmayın.

### 9.2. Veriler yereldir

- Tüm veriler **yalnızca bu bilgisayarda** saklanır.
- Program hiçbir veriyi internete veya bir sunucuya göndermez; bulut yedeği yoktur.
- Ağ üzerinden paylaşım desteklenmez; program tek bilgisayarda çalışır.

### 9.3. Yedekleme

- **Otomatik yedek varsayılan olarak açıktır.** Program her açılışta, son otomatik
  yedekten 24 saat geçtiyse yeni bir yedek alır.
- Otomatik yedekler `onmuhasebe-auto-YYYYAAGG-SSDDss.onm-backup` biçiminde adlandırılır.
- Varsayılan rotasyon: son **30 günlük**, **12 aylık** ve **5 yıllık** yedek tutulur.
  Manuel yedekler rotasyona dâhil değildir, silinmez.
- Yedekleri **başka bir diske veya ağ konumuna** kopyalayın (3-2-1 kuralı: 3 kopya,
  2 farklı ortam, 1 farklı konum).

### 9.4. Veritabanı şifrelemesi

- Veritabanı dosyası **şifrelenmemiştir**. Bilgisayara erişebilen biri dosyayı
  okuyabilir.
- Bilgisayarınızı fiziksel olarak koruyun; Windows oturum şifresi kullanın.
- Hassas veriler için **BitLocker** (veya benzeri disk şifreleme) önerilir.
- Program şifreleri düz metin saklamaz; kullanıcı şifreleri Argon2id ile özetlenir.
  Ancak bu, veritabanındaki muhasebe kayıtlarını şifrelemez.

### 9.5. Lisans

Bu program **BSD 3-Clause** lisansı ile lisanslanmıştır.
Telif hakkı (c) 2026, **@furcanomer**.

Ayrıntılar için program klasöründeki `LICENSE` dosyasına veya
<https://github.com/furcanomer/OnMuhasebe> adresine bakın.

## Ekran Görüntüleri

İlgili ekranların görselleri: [docs/EKRAN_GORUNTULERI/](EKRAN_GORUNTULERI/)

---

Son güncelleme: 2026-10-27
