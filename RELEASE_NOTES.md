# Ön Muhasebe v1.0.0

İlk kararlı sürüm. Windows üzerinde yerel (native) çalışan, internet bağlantısı
gerektirmeyen ön muhasebe programı.

## Öne Çıkanlar

- Cari hesaplar, ürün/stok, alış ve satış faturaları, kasa/banka, tahsilat/ödeme,
  gelir/gider kayıtları ve 6 rapor.
- Windows 10+ için native uygulama (Rust + egui + SQLite); harici çalışma
  zamanı, webview veya sunucu bağımlılığı yok.
- MSI kurulum paketi. Aynı `UpgradeCode` + değişen `ProductCode` ile Windows
  Installer Major Upgrade kullanılır; yeni sürüm eskisinin üzerine kurulur,
  "programdan iki tane olma" durumu oluşmaz. Kullanıcı verisi korunur.
- Veriler yalnızca yerel bilgisayarda tutulur; bulut senkron, telemetri veya
  reklam yoktur.
- Kullanıcı rolleri (yönetici / operatör), Argon2id şifre özeti, hatalı giriş
  sınırı ve otomatik kilit.
- Otomatik yedekleme ve rotasyon (30 günlük / 12 aylık / 5 yıllık), manuel
  yedek ve geri yükleme (`.onm-backup`).
- UTF-8 BOM'lu, noktalı virgül ayırıcılı CSV dışa/içe aktarma (Excel uyumlu).
- Klavye kısayolları, global arama ve ilk kurulum sihirbazı.

## Sistem Gereksinimleri

| Kalem | Gereksinim |
|-------|------------|
| İşletim sistemi | Windows 10 64-bit veya üzeri |
| Disk | Yaklaşık 200 MB (kurulum + veri) |
| Bellek | 2 GB veya üzeri |
| Ekran | En az 900 × 600 piksel |
| Ek çalışma zamanı | Gerekmez; MSVC çalışma zamanı statik bağlıdır (VC++ Redistributable kurulmaz) |

## Kurulum

1. `OnMuhasebe-1.0.0.msi` dosyasını indirin.
2. Çift tıklayın ve kurulum sihirbazını izleyin (yönetici onayı istenir;
   kurulum `Program Files` altına yapılır).
3. Windows SmartScreen "bilinmeyen yayıncı" uyarısı gösterebilir. Program kod
   imzalı değildir: **Ek bilgi** → **Yine de çalıştır** ile devam edin.
4. İlk açılışta yönetici hesabını oluşturun ve kurulum sihirbazını tamamlayın.

Güncelleme: yeni sürümün MSI dosyasını çalıştırın. Eski sürüm otomatik
kaldırılır, veriler korunur. Program içinde otomatik indirme/kurma özelliği
yoktur.

## Veri Konumu

| İçerik | Yol |
|--------|-----|
| Veritabanı | `%LOCALAPPDATA%\OnMuhasebe\OnMuhasebe\data\on_muhasebe.db` |
| Yedekler | `%LOCALAPPDATA%\OnMuhasebe\OnMuhasebe\data\backups\` |
| Günlükler | `%LOCALAPPDATA%\OnMuhasebe\OnMuhasebe\data\logs\` |

Kaldırma işlemi bu klasörleri silmez. Yedek almak için **Ayarlar → Yedekleme**
ekranını kullanın.

## Bilinen Sınırlamalar

- e-Fatura / e-Arşiv gönderimi yoktur.
- PDF fatura ve XLSX dışa aktarma yoktur (CSV vardır).
- Çoklu şirket, döviz, virman, çek/senet ve ağ üzerinden çok kullanıcı desteği
  yoktur.
- Genel muhasebe (mizan, yevmiye, defteri kebir), vergi beyannamesi ve bordro
  kapsam dışıdır.
- Program kod imzalı değildir; SmartScreen uyarısı beklenen davranıştır.
- Arayüz yalnızca Türkçedir.

## Dosyalar

| Dosya | Açıklama |
|-------|----------|
| `OnMuhasebe-1.0.0.msi` | Windows x64 kurulum paketi |
| `OnMuhasebe-1.0.0-kaynak.zip` | Sürümün kaynak kodu (git arşivi) |
| `SHA256SUMS.txt` | İndirilen dosyaların SHA-256 özetleri |

İndirdikten sonra doğrulama:

```text
certutil -hashfile OnMuhasebe-1.0.0.msi SHA256
```

Çıktı, `SHA256SUMS.txt` içindeki değerle aynı olmalıdır.

## Lisans

BSD 3-Clause. Telif hakkı (c) 2026, @furcanomer. Ayrıntılar için `LICENSE`
dosyasına bakın. Üçüncü taraf bağımlılıkları ve lisansları:
`THIRD_PARTY_LICENSES.md`.

## Yapay Zeka Beyanı

Bu projenin kaynak kodu, belgeleri ve tanıtım metinleri, @furcanomer'in
yönlendirmesi ve denetimi altında yapay zeka (LLM) desteğiyle geliştirilmiştir.
Kod, testler ve mimari kararlar insan tarafından gözden geçirilmiş ve
onaylanmıştır. Hatalar veya eksikler olabilir; katkılar ve düzeltmeler
memnuniyetle karşılanır.

Son güncelleme: 2026-10-27
