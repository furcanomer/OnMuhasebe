// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Uygulama genelinde kullanılan ortak hata tipi.
//!
//! Her katman kendi hatasını (veritabanı, dosya sistemi, geçersiz tutar ...)
//! bu tipe dönüştürür. Böylece arayüz katmanı tek bir tip üzerinden hata
//! gösterebilir ve `?` operatörü tüm katmanlarda çalışır.

use std::path::{Path, PathBuf};

/// Ön Muhasebe uygulamasının birleşik hata tipi.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// SQLite kaynaklı hatalar (kullanıcıya gösterilecek Türkçe metin).
    #[error("{0}")]
    Database(String),

    /// Dosya sistemi hataları; mesajda ilgili yol da gösterilir.
    #[error("'{path}' yolunda dosya sistemi hatası: {source}")]
    FileSystem {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// İşletim sistemi yerel uygulama verisi dizinini bildirmedi.
    #[error("Uygulama veri dizini belirlenemedi")]
    DataDirectoryUnavailable,

    /// Veritabanı, bu uygulama sürümünün desteklediğinden daha yeni bir şemaya sahip.
    #[error(
        "Veritabanı şeması bu sürümden daha yeni (bulunan sürüm: {found}, desteklenen: {supported}). \
         Lütfen uygulamayı güncelleyin."
    )]
    SchemaTooNew { found: i32, supported: i32 },

    /// Uygulanmış migration zincirinde bilinen bir sürüm eksik.
    #[error(
        "Veritabanı migration zinciri bozuk: v{missing} kaydı eksik. \
         Veritabanı değiştirilmedi; lütfen yedekten geri yükleyin."
    )]
    MigrationChainBroken { missing: i32 },

    /// Metinden tutar çözümlenemedi.
    #[error("Geçersiz tutar biçimi: '{0}'")]
    InvalidAmount(String),

    /// Tutar hesaplamasında tam sayı taşması oluştu.
    #[error("Tutar hesaplamasında taşma oluştu")]
    AmountOverflow,

    /// Aynı cari koduyla ikinci bir kayıt oluşturulmaya çalışıldı.
    #[error("'{0}' cari kodu zaten kullanılıyor. Lütfen başka bir kod girin.")]
    CustomerCodeTaken(String),

    /// Güncellenmek veya okunmak istenen cari hesap bulunamadı.
    #[error("Cari hesap bulunamadı (kayıt no: {0}).")]
    CustomerNotFound(i64),

    /// Aynı ürün koduyla ikinci bir kayıt oluşturulmaya çalışıldı.
    #[error("'{0}' ürün kodu zaten kullanılıyor. Lütfen başka bir kod girin.")]
    ProductCodeTaken(String),

    /// Aynı barkodla ikinci bir kayıt oluşturulmaya çalışıldı.
    #[error("'{0}' barkodu başka bir üründe kullanılıyor. Lütfen başka bir barkod girin.")]
    ProductBarcodeTaken(String),

    /// Güncellenmek veya okunmak istenen ürün bulunamadı.
    #[error("Ürün bulunamadı (kayıt no: {0}).")]
    ProductNotFound(i64),

    /// Metinden miktar çözümlenemedi.
    #[error("Geçersiz miktar biçimi: '{0}'")]
    InvalidQuantity(String),

    /// Miktar hesaplamasında tam sayı taşması oluştu.
    #[error("Miktar hesaplamasında taşma oluştu")]
    QuantityOverflow,

    /// Aynı fatura numarasıyla ikinci bir kayıt oluşturulmaya çalışıldı.
    #[error("'{0}' fatura numarası zaten kullanılıyor. Lütfen başka bir numara girin.")]
    InvoiceNumberTaken(String),

    /// Okunmak veya iptal edilmek istenen fatura bulunamadı.
    #[error("Fatura bulunamadı (kayıt no: {0}).")]
    InvoiceNotFound(i64),

    /// Zaten iptal edilmiş bir fatura yeniden iptal edilmek istendi.
    #[error("Bu fatura zaten iptal edilmiş (kayıt no: {0}).")]
    InvoiceAlreadyCancelled(i64),

    /// Veritabanından beklenmeyen/geçersiz bir değer okundu.
    #[error("Veritabanındaki kayıt geçersiz: {0}")]
    InvalidRecord(String),

    /// Aynı ada sahip ikinci bir kasa/banka hesabı oluşturulmaya çalışıldı.
    #[error("'{0}' kasa/banka hesabı adı zaten kullanılıyor. Lütfen başka bir ad girin.")]
    CashAccountNameTaken(String),

    /// Okunmak veya güncellenmek istenen kasa/banka hesabı bulunamadı.
    #[error("Kasa/banka hesabı bulunamadı (kayıt no: {0}).")]
    CashAccountNotFound(i64),

    /// Faturaya bağlı tahsilat/ödeme varken fatura iptal edilmek istendi.
    #[error("Bu faturaya bağlı tahsilat/ödeme bulunuyor. Önce ilgili ödemeleri iptal edin.")]
    InvoiceHasPayments(i64),

    /// Fatura ödenen tutarı, iptal sonrası negatife düşürülecekti.
    #[error("Faturanın ödenen tutarı negatife düşeceği için işlem yapılamadı (kayıt no: {0}).")]
    InvoicePaidAmountUnderflow(i64),

    /// Güncellenmek veya okunmak istenen tahsilat/ödeme kaydı bulunamadı.
    #[error("Tahsilat/ödeme kaydı bulunamadı (kayıt no: {0}).")]
    PaymentNotFound(i64),

    /// Zaten iptal edilmiş bir tahsilat/ödeme yeniden iptal edilmek istendi.
    #[error("Bu tahsilat/ödeme zaten iptal edilmiş (kayıt no: {0}).")]
    PaymentAlreadyCancelled(i64),

    /// İptal edilmiş bir faturaya dağıtım yapılmak istendi.
    #[error("İptal edilmiş faturaya tahsilat/ödeme dağıtılamaz (fatura no: {0}).")]
    AllocationToCancelledInvoice(i64),

    /// Seçilen cariye ait olmayan bir faturaya dağıtım yapılmak istendi.
    #[error("Seçilen cariye ait olmayan faturaya dağıtım yapılamaz (fatura no: {0}).")]
    AllocationCustomerMismatch(i64),

    /// Tahsilat/ödeme türü ile fatura türü uyuşmuyor.
    #[error(
        "Dağıtım türü uyuşmuyor: tahsilat yalnızca satış, ödeme yalnızca alış faturasına \
         dağıtılabilir (fatura no: {0})."
    )]
    AllocationTypeMismatch(i64),

    /// Dağıtım tutarı faturanın kalanını aşıyor.
    #[error("Dağıtım tutarı faturanın kalanını aşıyor (fatura no: {0}).")]
    AllocationExceedsRemaining(i64),

    /// Ödeme tutarı sıfırdan büyük olmalıdır.
    #[error("Tahsilat/ödeme tutarı sıfırdan büyük olmalıdır.")]
    PaymentAmountNotPositive,

    /// Aynı adla ikinci bir gelir/gider kategorisi oluşturulmaya çalışıldı.
    #[error("'{0}' adlı kategori bu türde zaten kullanılıyor. Lütfen başka bir ad girin.")]
    ExpenseCategoryNameTaken(String),

    /// Okunmak veya güncellenmek istenen gelir/gider kategorisi bulunamadı.
    #[error("Gelir/gider kategorisi bulunamadı (kayıt no: {0}).")]
    ExpenseCategoryNotFound(i64),

    /// Pasif bir kategoriye yeni hareket eklenmek istendi.
    #[error("Pasif kategoriye yeni gelir/gider kaydı eklenemez (kayıt no: {0}).")]
    ExpenseCategoryInactive(i64),

    /// Hareket türü ile kategorinin türü uyuşmuyor.
    #[error("Seçilen kategori bu hareket türüyle uyuşmuyor (kategori no: {0}).")]
    ExpenseCategoryTypeMismatch(i64),

    /// Pasif bir kasa/banka hesabına gelir/gider kaydı eklenmek istendi.
    #[error("Pasif kasa/banka hesabına gelir/gider kaydı eklenemez (kayıt no: {0}).")]
    CashAccountInactive(i64),

    /// Okunmak veya iptal edilmek istenen gelir/gider kaydı bulunamadı.
    #[error("Gelir/gider kaydı bulunamadı (kayıt no: {0}).")]
    IncomeExpenseNotFound(i64),

    /// Zaten iptal edilmiş bir gelir/gider kaydı yeniden iptal edilmek istendi.
    #[error("Bu gelir/gider kaydı zaten iptal edilmiş (kayıt no: {0}).")]
    IncomeExpenseAlreadyCancelled(i64),

    /// CSV metni veya içe aktarma satırı okunamadı.
    #[error("{0}")]
    Csv(String),

    /// Yedek dosyası açılamadı, bozuk veya gerekli tabloları içermiyor.
    #[error("{0}")]
    BackupInvalid(String),

    /// Yedek, bu uygulama sürümünün desteklediğinden daha yeni bir şemaya sahip.
    #[error("Bu yedek, programın daha yeni bir sürümüyle oluşturulmuş. Önce programı güncelleyin.")]
    BackupTooNew { found: i32, supported: i32 },

    /// Dosya başka bir uygulamanın (veya tanınmayan) veritabanı.
    #[error("Bu dosya bir OnMuhasebe yedeği değil.")]
    BackupWrongApp,

    /// İçe aktarma işlemi geri alındı.
    #[error("İçe aktarma başarısız: {0}. Hiçbir kayıt eklenmedi.")]
    ImportFailed(String),

    /// Oturumdaki kullanıcı bu işlem için gerekli role sahip değil.
    #[error("Bu işlem için yönetici yetkisi gerekir: {operation}")]
    PermissionDenied { operation: String },

    /// Kullanıcı adı veya şifre doğrulanamadı (hangisi olduğu söylenmez).
    #[error("Kullanıcı adı veya şifre hatalı.")]
    InvalidCredentials,

    /// Kullanıcı hesabı pasif durumda.
    #[error("Bu kullanıcı hesabı pasif durumda.")]
    UserInactive,

    /// Aynı kullanıcı adıyla ikinci bir hesap oluşturulamaz.
    #[error("'{0}' kullanıcı adı zaten kullanılıyor.")]
    UsernameTaken(String),

    /// İstenen kullanıcı bulunamadı.
    #[error("Kullanıcı bulunamadı (kayıt no: {0}).")]
    UserNotFound(i64),

    /// Son aktif yönetici korunmalıdır.
    #[error("Son aktif yönetici silinemez, pasife alınamaz veya rolü düşürülemez.")]
    LastAdminProtected,

    /// Kullanıcı kendi hesabı üzerinde yasak bir işlem denedi.
    #[error("Kendi hesabınız üzerinde bu işlemi yapamazsınız.")]
    SelfOperationProtected,

    /// İlk yönetici zaten oluşturulmuş.
    #[error("İlk yönetici zaten oluşturulmuş. Giriş ekranını kullanın.")]
    SetupAlreadyCompleted,

    /// Şifre kurallara uymuyor.
    #[error("Şifre en az 8 karakter olmalı ve en az bir harf ile bir rakam içermelidir.")]
    InvalidPassword,

    /// Argon2 şifre özeti üretilemedi veya okunamadı.
    #[error("Şifre güvenli biçimde işlenemedi: {0}")]
    PasswordHash(String),

    /// Çok fazla hatalı giriş denemesi; kısa süre beklenmeli.
    #[error("Çok fazla hatalı giriş denemesi. {0} saniye sonra tekrar deneyin.")]
    LoginThrottled(u64),

    /// Oturum açılmamış.
    #[error("Bu işlem için oturum açmalısınız.")]
    NotAuthenticated,

    /// Log/yedek dizini oluşturulamadı veya dosya işlemi başarısız.
    #[error("{0}")]
    Maintenance(String),
}

impl AppError {
    /// Bir yol ile dosya sistemi hatasını birlikte sarmalar.
    pub fn file_system(path: impl AsRef<Path>, source: std::io::Error) -> Self {
        Self::FileSystem {
            path: path.as_ref().to_path_buf(),
            source,
        }
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(sqlite_error_to_turkish(&error.to_string()))
    }
}

/// SQLite'ın İngilizce hata metnini kullanıcının anlayacağı Türkçe metne çevirir.
///
/// Tanınmayan hatalar ham metinle birlikte genel bir başlık altında döner;
/// böylece hata ayrıntısı kaybolmaz.
pub fn sqlite_error_to_turkish(raw: &str) -> String {
    let lower = raw.to_lowercase();
    if lower.contains("database or disk is full") {
        return "Disk dolu. Yer açıp tekrar deneyin.".to_string();
    }
    if lower.contains("database is locked") {
        return "Veritabanı şu an kullanımda. Birkaç saniye sonra tekrar deneyin.".to_string();
    }
    if lower.contains("disk i/o error") {
        return "Disk okuma/yazma hatası. Diskin bağlı olduğundan emin olun.".to_string();
    }
    if lower.contains("readonly database") || lower.contains("read-only") {
        return "Veritabanı salt okunur durumda. Dosya izinlerini kontrol edin.".to_string();
    }
    if lower.contains("no such table") {
        return "Veritabanı şeması eksik veya bozuk. Yedekten geri yüklemeyi deneyin.".to_string();
    }
    if lower.contains("constraint failed") {
        return "Bu kayıt kuralları ihlal ediyor (benzersizlik veya ilişki kuralı).".to_string();
    }
    if lower.contains("database disk image is malformed") {
        return "Veritabanı dosyası bozuk. Yedekten geri yükleyin.".to_string();
    }
    format!("Veritabanı hatası: {}", raw)
}

/// Uygulama genelinde kullanılan `Result` kısayolu.
pub type Result<T> = std::result::Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_system_errorinisina_yolu_yazar() {
        let error = AppError::file_system(
            "C:/veri/deneme.db",
            std::io::Error::new(std::io::ErrorKind::NotFound, "bulunamadı"),
        );

        let message = error.to_string();
        assert!(
            message.contains("C:/veri/deneme.db"),
            "hata mesajı yolu içermeli, gelen: {message}"
        );
    }

    #[test]
    fn veritabani_hatasi_donusturulebilir() {
        // rusqlite hataları `?` ile AppError'a dönüşebilmeli.
        fn fallible() -> Result<()> {
            Err(rusqlite::Error::InvalidQuery)?
        }

        assert!(matches!(fallible(), Err(AppError::Database(_))));
    }

    #[test]
    fn sqlite_disk_dolu_turkce_mesaja_cevrilir() {
        assert_eq!(
            sqlite_error_to_turkish("database or disk is full"),
            "Disk dolu. Yer açıp tekrar deneyin."
        );
    }

    #[test]
    fn sqlite_kilitli_veritabani_turkce_mesaja_cevrilir() {
        assert_eq!(
            sqlite_error_to_turkish("database is locked"),
            "Veritabanı şu an kullanımda. Birkaç saniye sonra tekrar deneyin."
        );
    }

    #[test]
    fn sqlite_disk_io_hatasi_turkce_mesaja_cevrilir() {
        assert_eq!(
            sqlite_error_to_turkish("disk I/O error"),
            "Disk okuma/yazma hatası. Diskin bağlı olduğundan emin olun."
        );
    }

    #[test]
    fn sqlite_salt_okunur_veritabani_turkce_mesaja_cevrilir() {
        assert_eq!(
            sqlite_error_to_turkish("attempt to write a readonly database"),
            "Veritabanı salt okunur durumda. Dosya izinlerini kontrol edin."
        );
    }

    #[test]
    fn sqlite_tablo_yok_hatasi_turkce_mesaja_cevrilir() {
        assert_eq!(
            sqlite_error_to_turkish("no such table: customers"),
            "Veritabanı şeması eksik veya bozuk. Yedekten geri yüklemeyi deneyin."
        );
    }

    #[test]
    fn sqlite_kisit_ihlali_turkce_mesaja_cevrilir() {
        assert_eq!(
            sqlite_error_to_turkish("UNIQUE constraint failed: customers.code"),
            "Bu kayıt kuralları ihlal ediyor (benzersizlik veya ilişki kuralı)."
        );
    }

    #[test]
    fn sqlite_bozuk_veritabani_turkce_mesaja_cevrilir() {
        assert_eq!(
            sqlite_error_to_turkish("database disk image is malformed"),
            "Veritabanı dosyası bozuk. Yedekten geri yükleyin."
        );
    }

    #[test]
    fn sqlite_bilinmeyen_hata_ham_metni_korur() {
        assert_eq!(
            sqlite_error_to_turkish("some unexpected sqlite failure"),
            "Veritabanı hatası: some unexpected sqlite failure"
        );
    }
}
