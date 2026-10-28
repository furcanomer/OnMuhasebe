# Katkı Rehberi (CONTRIBUTING)

Ön Muhasebe'ye katkı vermek istediğiniz için teşekkürler. Bu belge, katkı
sürecini ve kabul ölçütlerini açıklar. Katkılar (hata bildirimi, belge, kod,
çeviri, tasarım) memnuniyetle karşılanır.

## 1. Davranış Kuralları

Bu proje, [Contributor Covenant](https://www.contributor-covenant.org/)
ruhuna uyar:

- Saygılı olun; kişiyi değil, kodu ve fikri eleştirin.
- Taciz, ayrımcılık, hakaret ve kişisel saldırı kabul edilmez.
- Yapıcı eleştiri yapın; "bu çalışmıyor" yerine "şu girdide şu hata oluşuyor"
  deyin.
- Kapsayıcı dil kullanın; yeni gelenlere yardımcı olun.
- Teknik tartışmayı kişisel tartışmaya çevirmeyin.

Kurallara uymayan katkılar uyarı almadan reddedilebilir.

## 2. Nasıl Katkı Sunulur?

### 2.1. Hata bildirimi

GitHub Issues > **Bug report** şablonunu kullanın. Şunları ekleyin:

| Bilgi | Örnek |
|-------|-------|
| Sürüm | 1.0.0 (Hakkında ekranı veya `Cargo.toml`) |
| İşletim sistemi | Windows 10 22H2 x64 |
| Yeniden üretme adımları | 1) Cari ekle 2) Fatura kes 3) İptal et |
| Beklenen / gerçek | "Bakiye 0 olmalıydı, 100,00 kaldı" |
| Günlük dosyası | `%LOCALAPPDATA%\OnMuhasebe\OnMuhasebe\data\logs\` |
| Ekran görüntüsü | Varsa (kişisel verileri kapatın) |

Gerçek veritabanınızı **yüklemeyin**. Gerekirse örnek veriyle yeniden üretin
veya kişisel bilgileri temizleyin.

### 2.2. Özellik isteği

GitHub Issues > **Feature request**. Üç soruya yanıt verin: ne istiyorsunuz,
hangi sorunu çözüyor, hangi kullanıcı için? "Şu ekranın şu alanı" düzeyinde
somut olun. Kapsam dışı olduğu bilinen başlıklar için `ROADMAP.md` 3. bölüme
bakın.

### 2.3. Kod katkısı (PR)

- Tek konu = tek PR. İlgisiz refactor'ı aynı PR'a karıştırmayın.
- Test ekleyin; testi olmayan davranış değişikliği kabul edilmez.
- Mevcut testleri silmeyin veya gevşetmeyin.
- Commit geçmişini okunur tutun (bkz. 5. bölüm).

### 2.4. Belge katkısı

`docs/` altındaki kullanıcı belgeleri ve kök dizindeki geliştirici belgeleri
için PR açabilirsiniz. Belgede örnek veri kullanın; gerçek müşteri adı, vergi
numarası veya telefon yazmayın.

## 3. Geliştirme Ortamı

| Gereksinim | Not |
|------------|-----|
| Rust 1.99+ | `rustup` önerilir; `edition 2024` |
| MSVC toolchain | Windows'ta derleme için |
| Windows SDK | `link.exe` ve sistem kütüphaneleri |
| WiX Toolset 3.14 + `cargo-wix` | Yalnızca MSI üretimi için |

```text
cargo build
cargo test --all-targets
cargo run
```

İlk derleme uzundur (SQLite kaynaktan derlenir). `cargo run` ile arayüz açılır;
veri dizini `%LOCALAPPDATA%\OnMuhasebe\OnMuhasebe\data` altındadır.
Geliştirme sırasında gerçek verinizi kirletmemek için ayrı dizin kullanın:

```text
set ON_MUHASEBE_DATA_DIR=C:\gecici\onmuhasebe-dev
cargo run
```

## 4. Kod Stili ve Test Ölçütleri

| Kural | Komut |
|-------|-------|
| Biçim | `cargo fmt --check` |
| Lint (uyarı = hata) | `cargo clippy --all-targets -- -D warnings` |
| Hızlı testler | `cargo test --all-targets` |
| Ağır testler (isteğe bağlı, yerel) | `cargo test --release -- --ignored --test-threads=1` |

Ek kurallar:

- Para ve miktar `Money` / `Quantity` tipleriyle; `f32`/`f64` muhasebe
  hesabında kullanılmaz.
- UI katmanına SQL yazılmaz; sorgular `src/db/` altında.
- Kayıt silinmez; iptal veya pasife alma kullanılır.
- Şema değişikliği yalnızca yeni migration ile yapılır.
- Türkçe hata mesajları korunur.

Ayrıntılı gerekçeler: `GELISTIRICI_REHBERI.md`.

## 5. Commit Mesajı Stili

Conventional Commits benzeri kısa format:

```text
<tip>(<kapsam>): <kısa açıklama>
```

| Tip | Kullanım |
|-----|----------|
| `feat` | Yeni özellik |
| `fix` | Hata düzeltmesi |
| `docs` | Belge |
| `test` | Test |
| `refactor` | Davranış değişmeyen düzenleme |
| `perf` | Performans |
| `chore` | Bağımlılık, yapılandırma |

Örnekler:

```text
feat(invoice): fatura listesine kalan tutar sütunu ekle
fix(payment): iptal sonrası paid_amount sıfırlanmıyordu
docs(roadmap): fork'a bırakılan başlıkları netleştir
```

Kurallar: kısa açıklama 72 karakteri geçmesin; gövdede "neden" anlatılsın; her
commit derlenmeli ve testler geçmeli. Kırık commit bırakmayın.

## 6. PR Süreci

1. Depoyu fork'layın, kendi dalınızı açın: `feat/xyz` veya `fix/abc`.
2. Değişikliği küçük tutun; gerekirse birden çok PR'a bölün.
3. PR açıklamasında şunlar bulunsun: ne değişti, neden, hangi testler eklendi,
   varsa ekran görüntüsü (UI değişikliği), varsa migration numarası.
4. İnceleme yorumlarını yanıtlayın; itiraz varsa gerekçesiyle tartışın.
5. Birleştirme: squash tercih edilir; commit mesajı son hâlini alır.

Kabul ölçütü: testler geçiyor, katman sınırları korunuyor, belge güncel,
davranış için test var. "Çalışıyor" tek başına yeterli değildir.

## 7. Yapay Zekâ Kullanımı Beyanı

Yapay zekâ araçları (ChatGPT, Claude, OpenCode, Cursor, Cline, Aider vb.)
serbesttir; sorumluluk katkıyı gönderende kalır.

**Beyan zorunludur.** Commit mesajına veya PR açıklamasına şu satırı ekleyin:

```text
AI-assisted: <araç adı> (<hangi kısım>)
```

Örnek: `AI-assisted: Claude (test iskeleti ve hata mesajları)`.

Beklentiler:

- Yapay zekâ, projeyi tanımadan kod yazmasın; önce `AI_DEVRALMA.md` okutulsun.
- Üretilen kod okunmadan gönderilmesin; her satırın ne yaptığını bilin.
- Yapay zekâya "testleri sil" veya "hatayı sustur" dedirtmeyin.
- Lisans uyumu sizin sorumluluğunuzdadır; kopyalanan kodun kaynağını belirtin.
- Gerçek kullanıcı verisi veya kişisel bilgi yapay zekâya verilmesin.

## 8. İletişim

- Hata ve öneri: GitHub Issues
- Tartışma ve sorular: GitHub Discussions (varsa) veya ilgili issue
- Aynı konuda yeni issue açmayın; mevcut olana yorum ekleyin.
- Güvenlik açığı: herkese açık issue açmayın; doğrudan geliştiriciye ulaşın.

## 9. Lisans

Katkılarınız BSD 3-Clause koşulları altında projeye dâhil edilir. Telif
(c) 2026, @furcanomer. Ayrıntılar: `LICENSE`. Ticari kullanım ve marka kuralları
için `FORK_REHBERI.md`.

Son güncelleme: 2026-10-27
