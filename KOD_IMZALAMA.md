# Kod İmzalama Rehberi — Ön Muhasebe

## 1. Bu Belge Ne İçin

Ön Muhasebe 1.0.0 **imzasızdır**. İmzasız bir kurulum dosyası çalıştırıldığında
Windows SmartScreen "Windows korumalı bilgisayarınızı korudu / Bilinmeyen
yayıncı" uyarısı gösterebilir. Bu beklenen davranıştır; dosya bozuk değildir.

Bu belge, ileride programı dijital olarak imzalamak isteyen geliştirici için yol
gösterir. **Bu aşamada gerçek imzalama yapılmamıştır**; yalnızca rehberdir.
İmzalama, yönetici yetkisi ve (üretimde) ücretli sertifika gerektirir.

## 2. Kod İmzalama Nedir?

Dijital imza, bir dosyanın (`.exe`, `.msi`, `.ps1`) belirli bir yayıncı
tarafından üretildiğini ve sonradan değiştirilmediğini kanıtlayan kriptografik
bir ekdir.

| Kavram | Anlamı |
|--------|--------|
| Sertifika | Yayıncı kimliğini taşıyan, bir CA tarafından imzalanmış dosya |
| CA (Sertifika Otoritesi) | Sertifikayı veren ve itibarını yöneten kuruluş |
| Özel anahtar | İmzayı üreten gizli anahtar (PFX dosyasında parolayla korunur) |
| Zaman damgası (timestamp) | İmzanın atıldığı anı kanıtlar; sertifika süresi dolsa bile imza geçerli kalır |
| SmartScreen | Windows'un itibar tabanlı uyarı sistemi; imza tek başına yeterli değildir |

İmza, Windows'ta "Dosya özellikleri → Dijital İmzalar" sekmesinde görünür.
`Get-AuthenticodeSignature` ile de kontrol edilebilir.

## 3. Neden 1.0.0'da İmzalamıyoruz?

| Gerekçe | Açıklama |
|---------|----------|
| Maliyet | OV sertifika yıllık yaklaşık 200–300 USD; EV 400–600 USD |
| Donanım | EV sertifikalar genellikle USB token ister; CI'da kullanımı zorlaşır |
| İtibar süresi | Yeni sertifika bile SmartScreen itibarını hemen kazanmaz (OV) |
| Açık kaynak normu | Küçük açık kaynak projelerde imza yaygın değildir |
| İşlev | İmza, programın çalışmasını etkilemez; yalnızca uyarıyı azaltır |

İmzasız kurulumda kullanıcıya **Ek bilgi → Yine de çalıştır** adımı
`RELEASE_NOTES.md` içinde açıkça anlatılmıştır.

## 4. Self-Signed Sertifika (Ücretsiz, Yalnızca Test İçin)

Kendi sertifikanızı üretip kendi makinenizde güvenilir yapabilirsiniz. Bu,
**kapalı ortam / test** içindir; dağıtılan bir üründe SmartScreen itibarı
kazanmaz ve başka bilgisayarlarda "bilinmeyen yayıncı" uyarısı sürer.

Adımlar (PowerShell; yönetici gerekir):

```powershell
$cert = New-SelfSignedCertificate -Type CodeSigningCert `
  -Subject "CN=furcanomer" `
  -KeyUsage DigitalSignature `
  -FriendlyName "OnMuhasebe Self-Signed" `
  -CertStoreLocation "Cert:\CurrentUser\My" `
  -NotAfter (Get-Date).AddYears(5)

Export-PfxCertificate -Cert $cert `
  -FilePath "OnMuhasebe-selfsigned.pfx" `
  -Password (ConvertTo-SecureString -String "Sifre123" -Force -AsPlainText)
```

İmzalama (Windows SDK ile gelen `signtool.exe`):

```text
signtool sign /f OnMuhasebe-selfsigned.pfx /p Sifre123 /fd sha256 ^
  /tr http://timestamp.digicert.com /td sha256 target\release\OnMuhasebe.exe
```

Kullanıcı sertifikayı "Güvenilen Kök Sertifikalar" deposuna eklemedikçe imza
doğrulanmaz. Sertifikayı dağıtmayın; özel anahtar sızdığında herkes sizin
adınıza imza atabilir.

> Uyarı: `Sifre123` yalnızca örnektir. Gerçek PFX dosyasını parolasız veya zayıf
> parolayla saklamayın; depoya (git) **koymayın**.

## 5. Ücretli Sertifika (Üretim İçin)

| Tür | Yaklaşık yıllık maliyet | SmartScreen | Not |
|-----|-------------------------|-------------|-----|
| OV (Organization Validation) | 200–300 USD | İtibar zamanla kazanılır | Kullanımı kolay; PFX olarak alınabilir |
| EV (Extended Validation) | 400–600 USD | İtibar **anında** | Donanım token; CI entegrasyonu zahmetli |

Bilinen sağlayıcılar: DigiCert, Sectigo, GlobalSign, GoDaddy, SSL.com. Fiyat ve
koşullar yıldan yıla değişir; satın almadan önce karşılaştırın.

Sertifika alırken dikkat:

- Şirket/şahıs bilgileri ile başvuru sahibi **birebir** eşleşmelidir.
- Sertifikanın "Code Signing" amacı taşıdığını doğrulayın.
- Özel anahtarın teslim biçimini (token / PFX / bulut HSM) önceden netleştirin.
- Yenileme tarihini takvime ekleyin; süresi dolan sertifikayla yeni imza atılamaz.

## 6. İmzalama Adımları (Ücretli Sertifika)

Önce exe, sonra MSI imzalanır. MSI, içine gömülen exe'yi de taşıdığı için
sıralama önemlidir: **exe → MSI**.

```text
signtool sign /f sertifika.pfx /p SIFRE /fd sha256 ^
  /tr http://timestamp.digicert.com /td sha256 target\release\OnMuhasebe.exe

signtool sign /f sertifika.pfx /p SIFRE /fd sha256 ^
  /tr http://timestamp.digicert.com /td sha256 dist\OnMuhasebe-1.0.0.msi
```

Doğrulama:

```text
signtool verify /pa /v target\release\OnMuhasebe.exe
signtool verify /pa /v dist\OnMuhasebe-1.0.0.msi
Get-AuthenticodeSignature dist\OnMuhasebe-1.0.0.msi | Format-List
```

**Zaman damgası (timestamp) zorunlu kabul edilmelidir.** Zaman damgası olmadan
sertifika süresi dolduğunda eski imza da geçersiz sayılır. Zaman damgası ile
imzalanmış dosya, sertifika süresi dolsa bile "imzalandığı tarihte geçerliydi"
kanıtıyla doğrulanır.

## 7. Sürüm Çıkarma Akışına Ekleme

`build-msi.bat` içine imza adımı eklenebilir (örnek taslak):

```text
if defined SIGN_PFX (
  signtool sign /f "%SIGN_PFX%" /p "%SIGN_PASSWORD%" /fd sha256 ^
    /tr http://timestamp.digicert.com /td sha256 ^
    "target\release\OnMuhasebe.exe"
  signtool sign /f "%SIGN_PFX%" /p "%SIGN_PASSWORD%" /fd sha256 ^
    /tr http://timestamp.digicert.com /td sha256 ^
    "dist\OnMuhasebe-1.0.0.msi"
)
```

GitHub Actions ile imzalama:

- PFX dosyasını base64 olarak `SIGN_PFX_BASE64`, parolayı `SIGN_PASSWORD`
  adlı **secret** olarak saklayın.
- İş akışında dosyayı geçici dizine çözün, imzalayın, iş sonunda silin.
- Secret'ları log'a yazdırmayın; `signtool` çıktısında parola görünmemelidir.
- EV token bulut HSM gerektiriyorsa sağlayıcının CI eklentisini kullanın.

İmzalama sonrası `SHA256SUMS.txt` **yeniden** üretilmelidir; imza dosyayı
değiştirir.

## 8. 20 Yıl Dayanıklılık Notu

Zaman damgalı bir imza, sertifikanın süresi dolduktan sonra da doğrulanabilir.
Bu nedenle arşivdeki imzalı `.msi` dosyası 20 yıl sonra da "imzalı ve
değiştirilmemiş" olarak kontrol edilebilir.

Bununla birlikte:

- SmartScreen itibarı zamanla ve indirme sayısıyla ilişkilidir; imza tek başına
  garanti değildir.
- Arşivde hem imzalı dosyayı hem de `SHA256SUMS.txt` ve bu belgeyi saklayın.
- Sertifika yenileme kayıtlarını (CA, seri numarası, geçerlilik tarihleri)
  arşiv notlarına eklemek, ileride doğrulamayı kolaylaştırır.
- Özel anahtarı arşivlemeyin; yalnızca genel sertifika zinciri ve imzalı
  dosyalar arşivlenir.

Son güncelleme: 2026-10-29
