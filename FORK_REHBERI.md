# Fork ve Ticari Kullanım Rehberi — Ön Muhasebe

> Bu belge, projeyi fork edip kendi ürününe dönüştürmek isteyen geliştirici ve
> girişimci içindir. **Hukuki danışmanlık değildir**; lisans metninin kendisi
> (`LICENSE`) her zaman önceliklidir. Ticari bir ürün yayınlamadan önce bir
> hukukçuya danışın.

## 1. BSD 3-Clause Lisansı Ne Verir?

Proje **BSD 3-Clause** ("New BSD" veya "Revised BSD" olarak da bilinir) ile
lisanslanmıştır. Telif: Copyright (c) 2026, @furcanomer.

| Hak | Durum | Not |
|-----|-------|-----|
| Kopyalama | Serbest | Kaynak veya ikili |
| Değiştirme | Serbest | Kapalı kaynak dâhil |
| Dağıtma | Serbest | Ücretli veya ücretsiz |
| Satma | Serbest | Ticari ürün olabilir |
| Özel kullanım | Serbest | Şirket içi kullanım |
| Alt lisans / farklı lisans | Koşullu | Telif ve lisans bildirimi korunur |
| Garanti | **Yok** | "AS IS" — yazılım olduğu gibi |

### 1.1. Üç şart

1. **Telif bildirimi korunur.** Kaynak kod dağıtıyorsanız `LICENSE` dosyasındaki
   telif satırı ve lisans metni korunmalıdır.
2. **İkili dağıtımda lisans metni verilir.** Yalnızca `.exe`/`.msi` dağıtsanız
   bile lisans metni belgeye, kurulum sihirbazına veya "Hakkında" ekranına
   eklenmelidir.
3. **İsim ve marka onay imasında kullanılamaz.** Orijinal yazarın adı
   (@furcanomer) veya "Ön Muhasebe" adı, **onay, ortaklık veya sponsorluk iması**
   yaratacak şekilde kullanılamaz. Bu, 3. maddedir ve en sık ihlal edilen
   maddedir.

### 1.2. 3. maddeyi doğru okumak

3. madde **ismi hiç anamazsınız** demek değildir. Dediği şudur: kendi ürününüzün
tanıtımında, orijinal yazarın/Projenin sizi onayladığı izlenimini yaratmayın.

| İfade | Durum | Neden |
|-------|-------|-------|
| "X, @furcanomer'in Ön Muhasebe projesine dayanır." | Uygun | Gerçek, tarafsız atıf |
| "Ön Muhasebe destekli" | Riskli | Orijinal ürünle karışır |
| "Ön Muhasebe Resmî Sürüm" | İhlal | Resmî ilişki iması |
| "@furcanomer tarafından onaylanmıştır" | İhlal | Açık onay iması |
| "Ön Muhasebe ekibiyle geliştirildi" | İhlal | Ortaklık iması |
| Ürün adı: "Ön Muhasebe Pro" | İhlal riski | Marka karışıklığı |

Güvenli yol: kendi markanızı yaratın ve README'de tek cümlelik, tarafsız bir
atıf bırakın.

## 2. Neler Yapabilirsiniz?

- Fork'layın, değiştirin, kendi markanızla yayınlayın.
- Ücretli satın, abonelik modeli kurun, kurulum/destek hizmeti verin.
- Kapalı kaynak yapın: değiştirdiğiniz kısmı paylaşmak zorunda değilsiniz.
- Kendi özelliklerinizi ekleyin: e-Fatura, çoklu şirket, döviz, PDF çıktısı,
  ağ paylaşımı. Bunlar bu projede yoktur (bkz. `ROADMAP.md`); eklemek size
  kalmıştır.
- Kendi ürününüz için ticari marka başvurusu yapın — ancak "Ön Muhasebe" veya
  @furcanomer adını içeren bir başvuru 3. maddeyi ihlal eder.

## 3. Neler Yapamazsınız?

- Telif bildirimini ve lisans metnini kaldıramazsınız.
- "Resmî devamı", "onaylı sürüm", "ortaklık" gibi ifadeler kullanamazsınız.
- Orijinal simgeyi/logoyu kendi ürününüzde onay iması olarak kullanamazsınız;
  yeni bir simge tasarlayın (`assets/icon.ico` kopyalanabilir ama marka olarak
  kullanılamaz).
- Orijinal yazarın adını ürün adı, şirket adı veya alan adı olarak kullanamazsınız.
- BSD 3-Clause metnini "bu proje şu lisansla gelir" diyerek **silmek** veya
  yanlış tanıtmak (ör. farklı bir lisans adıyla anmak) 1. ve 2. maddeyi ihlal
  eder.

## 4. Fork Adımları (GitHub)

1. GitHub'da **Fork** düğmesine basın; kendi hesabınızda bir kopya oluşur.
2. Klonlayın:

```text
git clone https://github.com/<kullanici>/OnMuhasebe.git
cd OnMuhasebe
```

3. Orijinali `upstream` olarak ekleyin:

```text
git remote add upstream https://github.com/furcanomer/OnMuhasebe.git
git remote -v
```

4. İlk derlemeyi doğrulayın:

```text
cargo build --release
cargo test --all-targets
```

5. İlk değişiklik: `LICENSE` içindeki telif satırını **silmeden** kendi satırınızı
   ekleyin:

```text
Copyright (c) 2026, @furcanomer
Copyright (c) 2026, <sizin adınız>
All rights reserved.
```

6. `README.md` başına tek cümlelik atıf ekleyin:

```text
Bu proje, @furcanomer tarafından BSD 3-Clause ile lisanslanmış
"Ön Muhasebe" projesinin bir fork'udur.
```

7. Kendi markanızı belirleyin: `Cargo.toml` içindeki `description`, `authors`,
   `repository`; `wix/main.wxs` içindeki `Manufacturer`, `ProductName`; pencere
   başlığı (`APP_NAME`) ve `assets/icon.ico`.

## 5. Upstream ile Senkron Kalma

```text
git fetch upstream
git merge upstream/main        # veya: git rebase upstream/main
```

Çatışma çıkması beklenen dosyalar:

| Dosya | Neden çatışır | Kural |
|-------|---------------|-------|
| `src/db/migrations.rs` | İki taraf da yeni migration ekler | Upstream'in v1..vN'ini koru, kendi migration'ını **sona** ekle |
| `Cargo.toml` | Sürüm ve bağımlılık farkı | Kendi sürümünüzü koruyun, bağımlılıkları gözden geçirin |
| `README.md` | Marka ve atıf | Kendi metninizi koruyun |
| `wix/main.wxs` | Ürün adı, UpgradeCode | Kendi GUID'iniz kalmalı |

Altın kural: **upstream'in migration numaralarını değiştirmeyin.** Kendi
migration'ınız upstream'de varsa yeni numara verin; aksi hâlde iki farklı şema
aynı sürüm numarasını taşır.

## 6. Kendi Sürümünüzü Yayınlama (MSI)

1. `Cargo.toml` içindeki `version` alanını kendi sürümünüze çekin.
2. `wix/main.wxs` içinde **yeni** bir `UpgradeCode` üretin:

```text
[guid]::NewGuid()
```

3. `Manufacturer` ve `ProductName` alanlarını kendi markanızla değiştirin.
4. Derleyin:

```text
build-msi.bat
```

5. Çıktı: `dist\OnMuhasebe-<sürüm>.msi`. Kendi GitHub Releases sayfanızda
   yayınlayın.

`UpgradeCode` aynı kalırsa iki farklı ürün birbirinin üzerine kurulur veya
kaldırır; bu, kullanıcı verisinin kaybına yol açabilir.

## 7. Ticari Satışta Dikkat Edilecekler

| Konu | Ne yapmalı |
|------|-----------|
| Vergi | Yazılım satışı KDV'ye tabidir; mali müşavirle çalışın, fatura kesin |
| Garanti | Lisans "AS IS"; yine de ticari itibar için kendi destek/SLA sözleşmenizi hazırlayın |
| KVKK | Yerel çalışan sürüm veri toplamaz. Bulut senkron, telemetri veya lisans sunucusu eklerseniz aydınlatma metni ve saklama politikası gerekir |
| Marka | "Ön Muhasebe" adını kullanmayın; kendi adınızı yaratın |
| Telif | `LICENSE` korunur; fork olduğunu açıkça belirtin |
| Destek | Kullanıcı verisi sizde değilse bile yedekleme danışmanlığı beklenir; kurulum belgesi hazırlayın |
| Sürüm | Semver kullanın; kullanıcı verisini bozan değişiklikte MAJOR artırın |

### 7.1. Veri sahipliği

Program veriyi kullanıcının bilgisayarında tutar:
`%LOCALAPPDATA%\OnMuhasebe\OnMuhasebe\data\on_muhasebe.db`. Ticari ürününüzde
bu yolu değiştirirseniz (marka klasörü), kullanıcıya yükseltme sırasında veri
taşıma aracı sunmak **sizin** sorumluluğunuzdadır.

### 7.2. Yedek ve geri yükleme

Yedek dosyaları `.onm-backup` uzantılıdır ve sürüm bilgisi taşır. Yeni sürüm,
eski yedekleri okuyabilmelidir; okunamıyorsa `BackupTooNew` hatası verir. Kendi
şemanızı eklerken bu uyumluluğu koruyun veya kullanıcıya açık bir dönüştürme
adımı sunun.

## 8. Sık Yapılan Hatalar

| Hata | Sonuç | Doğrusu |
|------|-------|---------|
| Telif satırını silmek | 1. madde ihlali | Satırı koru, altına kendi satırını ekle |
| "Ön Muhasebe" adıyla satmak | Marka ihlali, karışıklık | Kendi markanı yarat |
| Aynı `UpgradeCode` | İki ürün çakışır, veri kaybı | Yeni GUID üret |
| Upstream migration'larını değiştirmek | Kullanıcı DB'leri bozulur | Sona ekle |
| Testleri silmek | Kısa vadede yeşil, uzun vadede kör | Testleri koru |
| Lisans metnini MSI'ya koymamak | 2. madde ihlali | `wix/LICENSE.rtf` ve Hakkında |
| Fork olduğunu gizlemek | Güven kaybı, topluluk tepkisi | README'de açık atıf |

### 8.1. Lisans metnini ürüne yerleştirme

İkili dağıtımda lisans metni "bir yerde" olmalıdır. Bu projede üç yer vardır;
fork'unuzda da en az ikisini koruyun:

| Yer | Dosya | Not |
|-----|-------|-----|
| Depo kökü | `LICENSE` | Kaynak dağıtımında zorunlu |
| MSI sihirbazı | `wix/LICENSE.rtf` | Kurulumda onay ekranı |
| Uygulama içi | Hakkında penceresi | Kullanıcı sonradan okuyabilir |

Lisans metnini kendi lisansınızla değiştirmek yerine **yanına** ekleyin:
orijinal BSD 3-Clause metni ve telif satırı korunur, kendi koşullarınız ayrı bir
bölümde yer alır. Çelişki varsa kullanıcı için daha koruyucu olan yorumu
seçmek yerine hukukçuya danışın.

### 8.2. Fork kontrol listesi

- [ ] `LICENSE` korundu, kendi telif satırı eklendi.
- [ ] README'de tarafsız fork atfı var.
- [ ] Ürün adı ve simge kendi markanız.
- [ ] `UpgradeCode` yeni GUID.
- [ ] `Manufacturer` / `ProductName` güncellendi.
- [ ] Veri dizini yolu değiştiyse taşıma aracı var.
- [ ] `cargo test --all-targets` geçiyor.
- [ ] MSI temiz makinede kurulup açılıyor.
- [ ] Lisans metni MSI'da ve Hakkında'da görünüyor.
- [ ] Sürüm numarası `Cargo.toml` ile MSI adı arasında tutarlı.

## 9. İletişim ve Katkı (Opsiyonel)

- Upstream'e hata düzeltmesi göndermek isterseniz: `CONTRIBUTING.md`.
- "Şu özelliği ekledim, upstream'e katar mısınız?" diye issue açabilirsiniz;
  kapsam uygunsa tartışılır (bkz. `ROADMAP.md` 6. bölüm).
- Fork'unuzu README'de açıkça belirtmek zorunlu değildir ama iyi niyet
  göstergesidir ve kullanıcı güvenini artırır.

Son güncelleme: 2026-10-27
