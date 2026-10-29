# Proje Retrospektifi — Ön Muhasebe 1.0.1

## 1. Özet

Ön Muhasebe 1.0.1, 20 adımlı bir geliştirme sürecinde üretilmiş, 453 test
geçen, Windows native, BSD 3-Clause açık kaynak bir ön muhasebe
uygulamasıdır. Rust + egui/eframe + SQLite üzerine kuruludur; webview, sunucu
ve bulut bağımlılığı yoktur. Tek bir `.exe` olarak çalışır ve verisini yerel
SQLite dosyasında tutar.

Bu sürüm "erken sürüm" (early release) olarak işaretlenmiştir. Kapsamlı
otomatik testlerden geçmiş olsa da geniş ölçekli gerçek kullanıcı beta dönemi
tamamlanmamıştır; geri bildirimler 1.0.2 veya 1.1 için yol gösterici olacaktır.

## 2. Sayılarla Proje

| Metrik | Değer |
|--------|-------|
| Geliştirme adımı | 20 (işlevsel: 12, yayına hazırlık: 8) |
| Toplam test | 453 (432 hızlı + 21 ağır) |
| Veritabanı tablosu | 14 |
| Migration | v1–v7 |
| Belge sayısı | Kullanıcı (5), geliştirici/AI (6), proje (5) |
| Release | v1.0.1 (önerilen), v1.0.0 (tarihsel) |
| CI | GitHub Actions, Windows runner, yeşil |
| Bağımlılık | 229 crate (0 GPL/AGPL) |

Ek teknik ölçüler:

| Kalem | Değer |
|-------|-------|
| `OnMuhasebe.exe` boyutu | ~20,1 MB (statik MSVC runtime) |
| MSI boyutu | ~8,5 MB |
| Hedef işletim sistemi | Windows 10+ x64 |
| Minimum pencere | 900 × 600 |
| Para/miktar tipi | `i64` (kuruş / binde bir) |
| Lisans | BSD 3-Clause |

## 3. Ne İyi Gitti

**Erken mimari kararlar.** Para ve miktarların tam sayı (`i64` kuruş) olarak
tutulması, tek yönlü katman (UI → servis → veritabanı) ve bir işlemin tek
transaction içinde tamamlanması 20 adım boyunca bozulmadı. Sonradan eklenen
raporlar, dışa aktarma ve yedekleme bu kararların üzerine sancısız oturdu.

**Her adımda aynı kontrol.** `cargo fmt --check`, `clippy -D warnings`,
`cargo test` ve gerçek GUI doğrulaması her adımda tekrarlandı. Bu disiplin
sayesinde hatalar birikmeden, eklendikleri yerde yakalandı. 900 × 600 yerleşim
testleri her yeni ekranda taşmayı erken gösterdi.

**Ağır testler işe yaradı.** Stres ve fuzz testleri üç gerçek hata buldu:

1. Cari ekstre sorgusunda N+1 problemi (6 s → 14 ms),
2. `i64::MIN` değerinin CSV round-trip'inde bozulması,
3. Kopuk migration zinciri (v3 → v5 atlaması).

Bu üç hata, "ağır testler gereksiz" varsayımını çürüttü.

**Kurulumun gerçekten çalışması.** MSI'ın "programdan iki tane olma" sorunu,
Major Upgrade + sabit `UpgradeCode` + değişen `ProductCode` ile çözüldü ve
gerçek kur/kaldır testiyle kanıtlandı. Bu, masaüstü dağıtımında en sık atlanan
adımdır.

**Dayanıklılık somutlaştı.** `crt-static` ile MSVC çalışma zamanı exe'ye
gömüldü; VC++ Redistributable bağımlılığı kalktı. Offline testte hiçbir ağ
bağlantısı denemesi görülmedi. `Cargo.lock` ve `rust-toolchain.toml` pinlendi,
arşiv paketi üretildi. "20 yıl sonra çalışsın" hedefi bir temenni olmaktan
çıkıp doğrulanabilir bir dosya kümesine dönüştü.

**Şeffaflık.** Yapay zeka beyanı üç yerde (README, repo açıklaması, sürüm
notları) yer alıyor. Kodun LLM desteğiyle, insan yönlendirmesi ve denetimi
altında yazıldığı gizlenmiyor.

## 4. Ne Farklı Yapardık

**SPDX başlıkları en baştan.** 97 dosyaya lisans başlığı sonradan eklendi.
Her dosya oluşturulurken eklenmesi, toplu taramayı ve doğrulamayı gereksiz
kılardı.

**Ekran görüntüleri daha erken.** 27 planlanan görselin 17'si alınabildi.
Görseller her ekran tamamlandığında alınsaydı, hem belge turu kısalır hem de
arayüz tutarlılığı erken görülürdü.

**Tag ve Release tutarlılığı.** v1.0.0 tag'i ilk commit'e işaret ediyordu;
Release asset'i ise sonradan statik exe ile güncellendi. Tag'i clone eden eski
exe'yi, Release'ten indiren yenisini görüyordu. Bu sürümde (v1.0.1) tag, Release
ve asset'ler aynı commit'ten üretilerek tutarsızlık kapatıldı.

**`screenshots/` orijinalleri.** Ham görsellerin repoda değil yerelde kalması
doğru karardı; yine de bunun en baştan `.gitignore`'da olması gerekirdi.

**Beta süreci.** Gerçek kullanıcıyla geniş beta yapılmadı. Bu eksik, "erken
sürüm" etiketi ve README/Release uyarılarıyla açıkça telafi edildi.

## 5. Açık Uçlar (1.0.2 veya 1.1 Hedefi)

- 10 eksik ekran görüntüsü (27'den 17 mevcut): cari/ürün/fatura formları,
  arama paleti, klavye kısayolları, hakkında, rapor ekleri.
- `cargo vendor` ile bağımlılıkların arşive gömülmesi.
- `archive/` zip'inin harici diske, buluta ve fiziksel ortama kopyalanması.
- Kod imzalama yok; rehber (`KOD_IMZALAMA.md`) hazır, sertifika alınmadı.
- Gerçek kullanıcı beta geri bildirimi sonrası hata düzeltmeleri.
- e-Fatura/e-Arşiv entegrasyonu (GİB onaylı entegratör gerekir).
- Çoklu şirket ve döviz desteği (bkz. `SURUM_2_GECIS.md`).

## 6. Öğrenilenler

**"Migration asla değiştirilmez" kuralı kritikti.** Yayınlanmış bir migration
dosyasını düzeltmek, sahadaki veritabanlarını sessizce bozar. Yeni ihtiyaç
her zaman yeni migration olarak eklenmeli.

**Para ve miktar tam sayı olmadan muhasebe yazılmaz.** `f64` ile kuruş
hesaplamak, yuvarlama farklarını faturalara ve raporlara taşır. `i64` kararı
en çok geri dönüşü olan tek karardı.

**Yerleşim testleri ucuz ve etkili.** 900 × 600 gibi dar bir pencerede taşma
kontrolü, kullanıcı şikayetinden önce hatayı yakalar.

**Korelasyon testleri mimariyi disiplinde tuttu.** "Şu tablo şu tabloya
bağlıysa şu kural geçerli" biçimindeki testler, ileride yapılan değişikliklerin
sessizce kuralı bozmasını engelledi.

**Dokümantasyon bir ürün parçası.** Kurulum, kullanım, sorun giderme, SSS ve
geliştirici rehberleri olmadan "çalışan exe" tek başına yeterli değil.

## 7. Kapanış

v1.0.1 erken sürüm olarak yayınlanmıştır. Fork'lar serbesttir (BSD 3-Clause).
Sürdürücü: @furcanomer.

Son güncelleme: 2026-10-29
