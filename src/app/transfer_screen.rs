// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Veri aktarımı: CSV içe/dışa aktarma, yedekleme ve geri yükleme akışları.
//!
//! Bu modül arayüzün çağırdığı komutları ve bunların durumunu tutar. CSV
//! biçimlendirme/okuma işi [`crate::csv`] modülünde, SQL işlemleri ise
//! [`crate::db`] katmanındadır. Arayüz yalnızca dosya seçicileri açar ve bu
//! modüldeki metotları çağırır.
//!
//! ## Yedek geri yükleme güvenliği
//!
//! Mevcut veritabanı önce `on_muhasebe.db.pre-restore-{zaman}` adıyla
//! kopyalanır. Yeni dosya önce geçici bir kopyaya yazılır; ancak bu kopya
//! doğrulandıktan ve bağlantı kapatıldıktan sonra asıl dosyanın yerine geçer.
//! Açma veya migration sırasında bir hata olursa güvenlik yedeği geri
//! kopyalanır; böylece kullanıcı verisi kaybolmaz.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};

use chrono::Local;

use crate::csv;
use crate::customer::{CustomerDraft, CustomerType, ValidatedCustomer, normalize_for_search};
use crate::db::{BackupInfo, Database, ImportOutcome, ImportSkip};
use crate::error::{AppError, Result};
use crate::product::{ProductDraft, Unit, ValidatedProduct};
use crate::restore_journal::{RestoreJournal, RestoreStep};

use super::{
    CashState, CustomerScreen, CustomerState, IncomeExpenseState, InvoiceState, OnMuhasebeApp,
    ProductScreen, ProductState, ReportsState, View,
};

/// Cari içe aktarma şablonunun sütunları.
pub const CUSTOMER_TEMPLATE_HEADERS: [&str; 12] = [
    "code",
    "name",
    "kind",
    "phone",
    "email",
    "address",
    "tax_office",
    "tax_number",
    "national_id",
    "contact_person",
    "notes",
    "active",
];

/// Ürün içe aktarma şablonunun sütunları.
pub const PRODUCT_TEMPLATE_HEADERS: [&str; 11] = [
    "code",
    "barcode",
    "name",
    "unit",
    "category",
    "purchase_price",
    "sale_price",
    "vat_rate",
    "min_stock",
    "notes",
    "active",
];

/// İçe aktarma türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportKind {
    /// Cari hesaplar.
    Customers,
    /// Ürünler.
    Products,
}

impl ImportKind {
    /// Pencere başlığı.
    pub fn title(self) -> &'static str {
        match self {
            Self::Customers => "Cari hesapları içe aktar",
            Self::Products => "Ürünleri içe aktar",
        }
    }

    /// Şablon dosyası için varsayılan ad.
    pub fn template_file_name(self) -> &'static str {
        match self {
            Self::Customers => "cari_sablon.csv",
            Self::Products => "urun_sablon.csv",
        }
    }

    /// Şablon sütunları.
    pub fn template_headers(self) -> &'static [&'static str] {
        match self {
            Self::Customers => &CUSTOMER_TEMPLATE_HEADERS,
            Self::Products => &PRODUCT_TEMPLATE_HEADERS,
        }
    }
}

/// Önizlemedeki tek satırın durumu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportRowStatus {
    /// Satır doğrulandı ve aktarılabilir.
    Ready,
    /// Satır aktarılmayacak; sebep kullanıcıya gösterilir.
    Error(String),
}

impl ImportRowStatus {
    /// Satır hazır mı?
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }

    /// Hata sebebi (hazırsa `None`).
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Ready => None,
            Self::Error(reason) => Some(reason),
        }
    }
}

/// Önizlemede taşınan doğrulanmış kayıt.
#[derive(Debug, Clone)]
pub enum ImportRecord {
    /// Cari hesap.
    Customer(ValidatedCustomer),
    /// Ürün.
    Product(ValidatedProduct),
}

/// İçe aktarma önizlemesindeki tek satır.
#[derive(Debug, Clone)]
pub struct ImportPreviewRow {
    /// CSV dosyasındaki satır numarası (başlık 1'dir).
    pub line: usize,
    /// Satırdaki kod (boşsa `(kod yok)`).
    pub code: String,
    /// Satır durumu.
    pub status: ImportRowStatus,
    /// Hazırsa doğrulanmış kayıt.
    pub record: Option<ImportRecord>,
}

/// Dosya aktarılmadan önce gösterilen önizleme.
#[derive(Debug, Clone)]
pub struct ImportPreview {
    /// İçe aktarma türü.
    pub kind: ImportKind,
    /// Kaynak dosyanın adı.
    pub file_name: String,
    /// Satırlar.
    pub rows: Vec<ImportPreviewRow>,
}

impl ImportPreview {
    /// Aktarılabilir satır sayısı.
    pub fn ready_count(&self) -> usize {
        self.rows.iter().filter(|row| row.status.is_ready()).count()
    }

    /// Hatalı satır sayısı.
    pub fn error_count(&self) -> usize {
        self.rows.len() - self.ready_count()
    }
}

/// İçe aktarma tamamlandığında gösterilen sonuç.
#[derive(Debug, Clone)]
pub struct ImportResultView {
    /// İçe aktarma türü.
    pub kind: ImportKind,
    /// Eklenen kayıt sayısı.
    pub inserted: usize,
    /// Atlanan satırlar.
    pub skipped: Vec<ImportSkip>,
}

/// Bilgilendirme penceresinin türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeKind {
    /// İşlem başarılı.
    Success,
    /// İşlem başarısız.
    Error,
}

/// Dosya işlemlerinden sonra gösterilen bilgilendirme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferNotice {
    /// Başarı/başarısızlık.
    pub kind: NoticeKind,
    /// Kısa başlık.
    pub title: String,
    /// Ayrıntılı mesaj.
    pub message: String,
    /// "Klasörü aç" için dosya yolu.
    pub path: Option<PathBuf>,
    /// Başarılı geri yükleme sonrası uygulamayı kapatma düğmesi gösterilsin mi?
    pub restore_success: bool,
}

impl TransferNotice {
    /// Başarı bilgilendirmesi.
    pub fn success(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind: NoticeKind::Success,
            title: title.into(),
            message: message.into(),
            path: None,
            restore_success: false,
        }
    }

    /// Hata bilgilendirmesi.
    pub fn error(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind: NoticeKind::Error,
            title: title.into(),
            message: message.into(),
            path: None,
            restore_success: false,
        }
    }

    /// "Klasörü aç" düğmesi gösterilsin mi?
    pub fn can_open_folder(&self) -> bool {
        self.kind == NoticeKind::Success && self.path.is_some()
    }
}

/// Geri yükleme onayı bekleyen yedek.
#[derive(Debug, Clone)]
pub struct PendingRestore {
    /// Seçilen yedek dosyası.
    pub path: PathBuf,
    /// Doğrulanmış yedek bilgisi.
    pub info: BackupInfo,
}

/// Aktarım ekranlarının durumu.
#[derive(Default)]
pub struct TransferState {
    /// CSV önizlemesi (varsa).
    pub import_preview: Option<ImportPreview>,
    /// Başarılı içe aktarma sonucu.
    pub import_result: Option<ImportResultView>,
    /// İçe aktarma sırasında oluşan hata.
    pub import_error: Option<String>,
    /// Dosya işlemi bilgilendirmesi.
    pub notice: Option<TransferNotice>,
    /// Geri yükleme onayı bekleyen dosya.
    pub pending_restore: Option<PendingRestore>,
    /// Arka planda çalışan işlemin açıklaması.
    pub busy_message: Option<String>,
    /// Başarılı geri yükleme sonrası uygulama kapatılmalı mı?
    pub restore_success: bool,
    backup_job: Option<Receiver<std::result::Result<(), String>>>,
    backup_target: Option<PathBuf>,
}

impl OnMuhasebeApp {
    /// Aktarım durumuna salt okunur erişim.
    pub fn transfer_state(&self) -> &TransferState {
        &self.transfer
    }

    /// Aktarım durumuna yazma erişimi (yalnızca pencere kapatma eylemleri).
    pub fn transfer_state_mut(&mut self) -> &mut TransferState {
        &mut self.transfer
    }

    /// Bilgilendirme penceresini kapatır.
    pub fn dismiss_transfer_notice(&mut self) {
        self.transfer.notice = None;
    }

    /// İçe aktarma önizlemesini iptal eder.
    pub fn cancel_import_preview(&mut self) {
        self.transfer.import_preview = None;
        self.transfer.import_error = None;
    }

    /// İçe aktarma sonuç penceresini kapatır.
    pub fn dismiss_import_result(&mut self) {
        self.transfer.import_result = None;
    }

    /// Geri yükleme onayını iptal eder.
    pub fn cancel_restore(&mut self) {
        self.transfer.pending_restore = None;
    }

    /// Yarıda kalan geri yüklemede "Güvenlik yedeğine dön" seçeneğini uygular.
    pub fn restore_recovery_with_safety_backup(&mut self) -> Result<()> {
        let Some(journal) = self.restore_recovery.clone() else {
            return Ok(());
        };

        // Dosyanın üzerine yazmadan önce açık bağlantı kapatılır.
        self.database = None;
        if let Err(error) = crate::restore_journal::apply_safety_backup(&journal) {
            self.restore_recovery_error = Some(error.to_string());
            if let Ok(database) = Database::open(&journal.target_db) {
                self.database = Some(database);
                self.refresh_after_restore();
            }
            return Err(error);
        }

        match Database::open(&journal.target_db) {
            Ok(database) => {
                self.database = Some(database);
                RestoreJournal::remove(&RestoreJournal::path(self.config.data_dir()))?;
                self.restore_recovery = None;
                self.restore_recovery_error = None;
                self.refresh_after_restore();
                Ok(())
            }
            Err(error) => {
                self.restore_recovery_error = Some(error.to_string());
                Err(error)
            }
        }
    }

    /// Yarıda kalan geri yüklemede "Mevcut veritabanıyla devam et" seçeneği.
    pub fn restore_recovery_continue(&mut self) -> Result<()> {
        RestoreJournal::remove(&RestoreJournal::path(self.config.data_dir()))?;
        self.restore_recovery = None;
        self.restore_recovery_error = None;
        Ok(())
    }

    /// Bugünün `YYYYMMDD` biçimli tarihi.
    fn today_compact() -> String {
        Local::now().format("%Y%m%d").to_string()
    }

    /// Yedek dosyası için varsayılan ad.
    pub fn default_backup_name() -> String {
        format!(
            "OnMuhasebe-yedek-{}.onm-backup",
            Local::now().format("%Y%m%d-%H%M%S")
        )
    }

    /// Seçilen yolu `.csv` uzantısıyla tamamlar.
    pub fn csv_path(path: PathBuf) -> PathBuf {
        if path.extension().is_some() {
            path
        } else {
            path.with_extension("csv")
        }
    }

    /// Seçilen yolu `.onm-backup` uzantısıyla tamamlar.
    pub fn backup_path(path: PathBuf) -> PathBuf {
        if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("onm-backup"))
        {
            path
        } else {
            let mut name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "OnMuhasebe-yedek".to_owned());
            name.push_str(".onm-backup");
            path.with_file_name(name)
        }
    }

    /// Cari hesap CSV şablonunu yazar.
    pub fn write_customer_template(&mut self, path: impl AsRef<Path>) -> Result<()> {
        self.write_template(ImportKind::Customers, path)
    }

    /// Ürün CSV şablonunu yazar.
    pub fn write_product_template(&mut self, path: impl AsRef<Path>) -> Result<()> {
        self.write_template(ImportKind::Products, path)
    }

    fn write_template(&mut self, kind: ImportKind, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let rows = vec![
            kind.template_headers()
                .iter()
                .map(|header| (*header).to_owned())
                .collect(),
        ];
        let outcome = csv::write_rows(path, &rows);
        self.record_csv_write(outcome, path, "Şablon kaydedildi")
    }

    /// Cari hesap CSV dosyasını okuyup önizleme oluşturur.
    pub fn preview_customer_import(&mut self, path: impl AsRef<Path>) -> Result<()> {
        self.preview_import(ImportKind::Customers, path)
    }

    /// Ürün CSV dosyasını okuyup önizleme oluşturur.
    pub fn preview_product_import(&mut self, path: impl AsRef<Path>) -> Result<()> {
        self.preview_import(ImportKind::Products, path)
    }

    fn preview_import(&mut self, kind: ImportKind, path: impl AsRef<Path>) -> Result<()> {
        if !self.can_administer() {
            tracing::warn!("yetki reddedildi: CSV içe aktarma");
            let error = AppError::PermissionDenied {
                operation: "CSV içe aktarma".to_owned(),
            };
            self.transfer.notice = Some(TransferNotice::error(
                "Yetki yok",
                "Bu işlem için yönetici yetkisi gerekir.",
            ));
            return Err(error);
        }
        let path = path.as_ref();
        let outcome = self.build_import_preview(kind, path);
        match outcome {
            Ok(preview) => {
                self.transfer.import_error = None;
                self.transfer.import_result = None;
                self.transfer.import_preview = Some(preview);
                Ok(())
            }
            Err(error) => {
                let message = error.to_string();
                self.transfer.import_error = Some(message.clone());
                self.transfer.import_preview = None;
                self.transfer.notice = Some(TransferNotice::error(
                    "CSV okunamadı",
                    format!("CSV okunamadı: {message}"),
                ));
                Err(error)
            }
        }
    }

    fn build_import_preview(&self, kind: ImportKind, path: &Path) -> Result<ImportPreview> {
        let database = self
            .database
            .as_ref()
            .ok_or_else(|| AppError::Csv("Veritabanı açık değil.".to_owned()))?;
        let bytes = std::fs::read(path).map_err(|error| AppError::file_system(path, error))?;
        let text = String::from_utf8(bytes)
            .map_err(|_| AppError::Csv("CSV dosyası UTF-8 kodlamasında olmalıdır.".to_owned()))?;
        let table = csv::parse(&text).map_err(AppError::Csv)?;

        let header_row = table
            .rows
            .first()
            .ok_or_else(|| AppError::Csv("CSV dosyasında başlık satırı yok.".to_owned()))?;
        let mut index = HashMap::new();
        for (position, header) in header_row.iter().enumerate() {
            index.insert(normalize_for_search(header.trim()), position);
        }
        for required in kind.template_headers() {
            if !index.contains_key(&normalize_for_search(required)) {
                return Err(AppError::Csv(format!(
                    "CSV başlığında '{required}' sütunu eksik."
                )));
            }
        }

        if table.rows.len() < 2 {
            return Err(AppError::Csv(
                "CSV dosyasında veri satırı bulunmuyor.".to_owned(),
            ));
        }

        let mut rows = Vec::new();
        let mut seen_codes = HashSet::new();
        let mut seen_barcodes = HashSet::new();
        for (offset, row) in table.rows.iter().enumerate().skip(1) {
            let line = offset + 1;
            let prepared = match kind {
                ImportKind::Customers => {
                    prepare_customer_row(row, &index, line, &mut seen_codes, database)
                }
                ImportKind::Products => prepare_product_row(
                    row,
                    &index,
                    line,
                    &mut seen_codes,
                    &mut seen_barcodes,
                    database,
                ),
            };
            rows.push(prepared);
        }

        Ok(ImportPreview {
            kind,
            file_name: path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string()),
            rows,
        })
    }

    /// Önizlemedeki hazır satırları tek işlemde aktarır.
    pub fn confirm_import(&mut self) -> Result<()> {
        if !self.can_administer() {
            tracing::warn!("yetki reddedildi: CSV içe aktarma onayı");
            let error = AppError::PermissionDenied {
                operation: "CSV içe aktarma".to_owned(),
            };
            self.transfer.notice = Some(TransferNotice::error(
                "Yetki yok",
                "Bu işlem için yönetici yetkisi gerekir.",
            ));
            return Err(error);
        }
        let Some(preview) = self.transfer.import_preview.clone() else {
            return Ok(());
        };
        let customers: Vec<ValidatedCustomer> = preview
            .rows
            .iter()
            .filter_map(|row| match &row.record {
                Some(ImportRecord::Customer(customer)) => Some(customer.clone()),
                _ => None,
            })
            .collect();
        let products: Vec<ValidatedProduct> = preview
            .rows
            .iter()
            .filter_map(|row| match &row.record {
                Some(ImportRecord::Product(product)) => Some(product.clone()),
                _ => None,
            })
            .collect();

        if customers.is_empty() && products.is_empty() {
            let error = AppError::Csv("Aktarılacak hazır satır yok.".to_owned());
            self.transfer.import_error = Some(error.to_string());
            return Err(error);
        }

        let Some(database) = self.database.as_ref() else {
            let error = AppError::Csv("Veritabanı açık değil.".to_owned());
            self.transfer.import_error = Some(error.to_string());
            return Err(error);
        };

        let outcome = match preview.kind {
            ImportKind::Customers => database.import_customers(&customers),
            ImportKind::Products => database.import_products(&products),
        };

        match outcome {
            Ok(ImportOutcome { inserted, skipped }) => {
                let mut all_skipped: Vec<ImportSkip> = preview
                    .rows
                    .iter()
                    .filter_map(|row| {
                        row.status.reason().map(|reason| ImportSkip {
                            code: row.code.clone(),
                            reason: reason.to_owned(),
                        })
                    })
                    .collect();
                all_skipped.extend(skipped);

                self.transfer.import_result = Some(ImportResultView {
                    kind: preview.kind,
                    inserted,
                    skipped: all_skipped,
                });
                self.transfer.import_preview = None;
                self.transfer.import_error = None;

                match preview.kind {
                    ImportKind::Customers => {
                        self.customer.screen = CustomerScreen::List;
                        self.refresh_customer_list();
                    }
                    ImportKind::Products => {
                        self.product.screen = ProductScreen::List;
                        self.refresh_product_list();
                    }
                }
                Ok(())
            }
            Err(error) => {
                self.transfer.import_error = Some(error.to_string());
                Err(error)
            }
        }
    }

    /// Rapor ekranındaki aktif rapor için varsayılan CSV adı.
    pub fn report_csv_default_name(&self) -> String {
        let date = Self::today_compact();
        let state = &self.reports;
        match state.tab {
            crate::report::ReportKind::CustomerStatement => {
                let code = state
                    .customer_id
                    .and_then(|id| state.customers.iter().find(|customer| customer.id == id))
                    .map(|customer| sanitize_component(&customer.code))
                    .unwrap_or_else(|| "cari".to_owned());
                format!("cari_ekstre_{code}_{date}.csv")
            }
            crate::report::ReportKind::CustomerBalances => {
                format!("cari_bakiye_ozeti_{date}.csv")
            }
            crate::report::ReportKind::StockStatus => format!("stok_durumu_{date}.csv"),
            crate::report::ReportKind::SalesPurchase => {
                let grouping = match state.sales_grouping {
                    crate::report::SalesPurchaseGrouping::Customer => "cari",
                    crate::report::SalesPurchaseGrouping::Product => "urun",
                };
                format!("satis_alis_{grouping}_{date}.csv")
            }
            crate::report::ReportKind::CashSummary => format!("kasa_banka_{date}.csv"),
            crate::report::ReportKind::IncomeExpense => format!("gelir_gider_{date}.csv"),
        }
    }

    /// Cari hesap listesi için varsayılan CSV adı.
    pub fn customer_list_csv_name() -> String {
        format!("cari_hesaplar_{}.csv", Self::today_compact())
    }

    /// Ürün listesi için varsayılan CSV adı.
    pub fn product_list_csv_name() -> String {
        format!("urunler_{}.csv", Self::today_compact())
    }

    /// Fatura listesi için varsayılan CSV adı.
    pub fn invoice_list_csv_name(&self) -> String {
        let kind = match self.invoice.invoice_type {
            crate::invoice::InvoiceType::Sales => "satis",
            crate::invoice::InvoiceType::Purchase => "alis",
        };
        format!("faturalar_{kind}_{}.csv", Self::today_compact())
    }

    /// Tahsilat/ödeme listesi için varsayılan CSV adı.
    pub fn payment_list_csv_name() -> String {
        format!("tahsilat_odemeler_{}.csv", Self::today_compact())
    }

    /// Gelir/gider hareket listesi için varsayılan CSV adı.
    pub fn income_expense_list_csv_name() -> String {
        format!("gelir_gider_hareketleri_{}.csv", Self::today_compact())
    }

    /// Kasa/banka hesap listesi için varsayılan CSV adı.
    pub fn cash_account_list_csv_name() -> String {
        format!("kasa_banka_hesaplari_{}.csv", Self::today_compact())
    }

    /// CSV başarı/hata bilgilendirmesini kaydeder.
    pub(super) fn record_csv_write(
        &mut self,
        outcome: Result<()>,
        path: &Path,
        success_title: &str,
    ) -> Result<()> {
        match outcome {
            Ok(()) => {
                self.transfer.notice = Some(TransferNotice {
                    kind: NoticeKind::Success,
                    title: success_title.to_owned(),
                    message: format!("CSV kaydedildi: {}", path.display()),
                    path: Some(path.to_path_buf()),
                    restore_success: false,
                });
                Ok(())
            }
            Err(error) => {
                let message = error.to_string();
                self.transfer.notice = Some(TransferNotice::error(
                    "CSV yazılamadı",
                    format!("CSV yazılamadı: {message}"),
                ));
                Err(error)
            }
        }
    }

    /// Yedeği arka planda dışa aktarır.
    pub fn start_backup_export(&mut self, path: impl AsRef<Path>) -> Result<()> {
        if self.transfer.backup_job.is_some() {
            let error = AppError::Csv("Yedek zaten hazırlanıyor.".to_owned());
            self.transfer.notice =
                Some(TransferNotice::error("Yedek yazılamadı", error.to_string()));
            return Err(error);
        }

        let path = path.as_ref().to_path_buf();
        let Some(database) = self.database.as_ref() else {
            let error = AppError::Csv("Veritabanı açık değil.".to_owned());
            self.transfer.notice = Some(TransferNotice::error(
                "Yedek yazılamadı",
                format!("Yedek yazılamadı: {error}"),
            ));
            return Err(error);
        };
        let Some(source) = database.path().map(Path::to_path_buf) else {
            let error = AppError::Csv("Veritabanı dosyası bulunamadı.".to_owned());
            self.transfer.notice = Some(TransferNotice::error(
                "Yedek yazılamadı",
                format!("Yedek yazılamadı: {error}"),
            ));
            return Err(error);
        };

        if paths_equal(&source, &path) {
            let error = AppError::Csv("Yedek hedefi veritabanı dosyası olamaz.".to_owned());
            self.transfer.notice = Some(TransferNotice::error(
                "Yedek yazılamadı",
                format!("Yedek yazılamadı: {error}"),
            ));
            return Err(error);
        }

        if let Err(error) = database.checkpoint() {
            let message = format!("Yedek yazılamadı: {error}");
            self.transfer.notice = Some(TransferNotice::error("Yedek yazılamadı", message));
            return Err(error);
        }

        let (sender, receiver) = mpsc::channel();
        self.transfer.backup_job = Some(receiver);
        self.transfer.backup_target = Some(path.clone());
        self.transfer.busy_message = Some("Yedek dışa aktarılıyor...".to_owned());

        std::thread::spawn(move || {
            let result = std::fs::copy(&source, &path)
                .map(|_| ())
                .map_err(|error| format!("Yedek yazılamadı: {error}"));
            let _ = sender.send(result);
        });

        Ok(())
    }

    /// Yedeği beklemeden (testler ve komut satırı için) dışa aktarır.
    pub fn export_backup_now(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let Some(database) = self.database.as_ref() else {
            let error = AppError::Csv("Veritabanı açık değil.".to_owned());
            self.transfer.notice = Some(TransferNotice::error(
                "Yedek yazılamadı",
                format!("Yedek yazılamadı: {error}"),
            ));
            return Err(error);
        };
        let Some(source) = database.path().map(Path::to_path_buf) else {
            let error = AppError::Csv("Veritabanı dosyası bulunamadı.".to_owned());
            self.transfer.notice = Some(TransferNotice::error(
                "Yedek yazılamadı",
                format!("Yedek yazılamadı: {error}"),
            ));
            return Err(error);
        };

        if paths_equal(&source, path) {
            let error = AppError::Csv("Yedek hedefi veritabanı dosyası olamaz.".to_owned());
            self.transfer.notice = Some(TransferNotice::error(
                "Yedek yazılamadı",
                format!("Yedek yazılamadı: {error}"),
            ));
            return Err(error);
        }

        let outcome = database.checkpoint().and_then(|_| {
            if let Some(parent) = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
            {
                std::fs::create_dir_all(parent)
                    .map_err(|error| AppError::file_system(parent, error))?;
            }
            std::fs::copy(&source, path)
                .map(|_| ())
                .map_err(|error| AppError::file_system(path, error))
        });

        match outcome {
            Ok(()) => {
                self.transfer.notice = Some(TransferNotice {
                    kind: NoticeKind::Success,
                    title: "Yedek kaydedildi".to_owned(),
                    message: format!("Yedek kaydedildi: {}", path.display()),
                    path: Some(path.to_path_buf()),
                    restore_success: false,
                });
                Ok(())
            }
            Err(error) => {
                let message = format!("Yedek yazılamadı: {error}");
                self.transfer.notice = Some(TransferNotice::error("Yedek yazılamadı", message));
                Err(error)
            }
        }
    }

    /// Arka plan yedek işini kontrol eder; her karede çağrılır.
    pub fn poll_transfer(&mut self) {
        let Some(receiver) = &self.transfer.backup_job else {
            return;
        };
        match receiver.try_recv() {
            Ok(result) => {
                self.transfer.backup_job = None;
                self.transfer.busy_message = None;
                let target = self.transfer.backup_target.take();
                match result {
                    Ok(()) => {
                        let path = target.unwrap_or_default();
                        self.transfer.notice = Some(TransferNotice {
                            kind: NoticeKind::Success,
                            title: "Yedek kaydedildi".to_owned(),
                            message: format!("Yedek kaydedildi: {}", path.display()),
                            path: Some(path),
                            restore_success: false,
                        });
                    }
                    Err(message) => {
                        self.transfer.notice =
                            Some(TransferNotice::error("Yedek yazılamadı", message));
                    }
                }
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                self.transfer.backup_job = None;
                self.transfer.busy_message = None;
                self.transfer.backup_target = None;
                self.transfer.notice = Some(TransferNotice::error(
                    "Yedek yazılamadı",
                    "Yedek yazılamadı: arka plan işlemi beklenmedik şekilde sonlandı.",
                ));
            }
        }
    }

    /// Yedek dosyasını doğrular ve geri yükleme onayı için hazırlar.
    pub fn prepare_restore(&mut self, path: impl AsRef<Path>) -> Result<()> {
        if !self.can_administer() {
            tracing::warn!("yetki reddedildi: yedek geri yükleme");
            let error = AppError::PermissionDenied {
                operation: "yedek geri yükleme".to_owned(),
            };
            self.transfer.pending_restore = None;
            self.transfer.notice = Some(TransferNotice::error(
                "Yetki yok",
                "Yedek geri yükleme için yönetici yetkisi gerekir.",
            ));
            return Err(error);
        }
        let path = path.as_ref().to_path_buf();
        if !path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("onm-backup"))
        {
            let error = AppError::BackupInvalid(
                "Yalnızca .onm-backup uzantılı dosyalar geri yüklenebilir.".to_owned(),
            );
            self.transfer.pending_restore = None;
            self.transfer.notice = Some(TransferNotice::error(
                "Yedek doğrulanamadı",
                error.to_string(),
            ));
            return Err(error);
        }
        match Database::validate_backup(&path) {
            Ok(info) => {
                self.transfer.pending_restore = Some(PendingRestore { path, info });
                Ok(())
            }
            Err(error) => {
                self.transfer.pending_restore = None;
                self.transfer.notice = Some(TransferNotice::error(
                    "Yedek doğrulanamadı",
                    error.to_string(),
                ));
                Err(error)
            }
        }
    }

    /// Onaylanan yedeği geri yükler.
    pub fn confirm_restore(&mut self) -> Result<()> {
        if !self.can_administer() {
            tracing::warn!("yetki reddedildi: yedek geri yükleme onayı");
            let error = AppError::PermissionDenied {
                operation: "yedek geri yükleme".to_owned(),
            };
            self.transfer.notice = Some(TransferNotice::error(
                "Yetki yok",
                "Yedek geri yükleme için yönetici yetkisi gerekir.",
            ));
            return Err(error);
        }
        let Some(pending) = self.transfer.pending_restore.take() else {
            return Ok(());
        };
        let database_path = self.config.database_path().to_path_buf();
        let outcome = self.restore_from(&pending.path, &database_path);
        match outcome {
            Ok(()) => {
                tracing::info!(path = %pending.path.display(), "yedek geri yüklendi");
                self.transfer.notice = Some(TransferNotice {
                    kind: NoticeKind::Success,
                    title: "Yedek geri yüklendi".to_owned(),
                    message: "Yedek geri yüklendi. Uygulama yeniden başlatılacak.".to_owned(),
                    path: None,
                    restore_success: true,
                });
                self.transfer.restore_success = true;
                Ok(())
            }
            Err(error) => {
                tracing::error!(path = %pending.path.display(), error = %error, "yedek geri yüklenemedi");
                self.transfer.pending_restore = None;
                self.transfer.notice = Some(TransferNotice::error(
                    "Yedek geri yüklenemedi",
                    error.to_string(),
                ));
                Err(error)
            }
        }
    }

    fn restore_from(&mut self, backup: &Path, database_path: &Path) -> Result<()> {
        Database::validate_backup(backup)?;

        let database = self
            .database
            .as_ref()
            .ok_or_else(|| AppError::Csv("Veritabanı açık değil.".to_owned()))?;
        database.checkpoint()?;

        if !database_path.exists() {
            return Err(AppError::Csv(
                "Mevcut veritabanı dosyası bulunamadı.".to_owned(),
            ));
        }
        if paths_equal(backup, database_path) {
            return Err(AppError::Csv(
                "Seçilen dosya zaten açık veritabanıdır.".to_owned(),
            ));
        }

        let stamp = Local::now().format("%Y%m%d-%H%M%S").to_string();
        let safety = unique_path(&database_path.with_file_name(format!(
            "{}.pre-restore-{stamp}",
            database_path
                .file_name()
                .map(|name| name.to_string_lossy())
                .unwrap_or_default()
        )));
        std::fs::copy(database_path, &safety)
            .map_err(|error| AppError::file_system(&safety, error))?;

        // Güvenlik yedeği alındıktan sonra günlük yazılır. Süreç bundan sonra
        // kesilirse bir sonraki açılışta yarıda kalan işlem kullanıcıya bildirilir.
        let journal_path = RestoreJournal::path(self.config.data_dir());
        let mut journal = RestoreJournal::new(
            backup.to_path_buf(),
            safety.clone(),
            database_path.to_path_buf(),
        );
        journal.write(&journal_path)?;

        let temp = unique_path(&database_path.with_file_name(format!(
            "{}.restore-tmp",
            database_path
                .file_name()
                .map(|name| name.to_string_lossy())
                .unwrap_or_default()
        )));
        if let Err(error) = std::fs::copy(backup, &temp) {
            let _ = std::fs::remove_file(&temp);
            let _ = RestoreJournal::remove(&journal_path);
            return Err(AppError::file_system(&temp, error));
        }

        journal.update_step(&journal_path, RestoreStep::Validating)?;
        if let Err(error) = Database::validate_backup(&temp) {
            let _ = std::fs::remove_file(&temp);
            let _ = RestoreJournal::remove(&journal_path);
            return Err(error);
        }

        journal.update_step(&journal_path, RestoreStep::Reopening)?;
        self.database = None;
        remove_sidecars(database_path);

        // Dosyayı doğrudan kopyalamak Windows'ta `rename` ile mevcut dosyanın
        // üzerine yazma davranışına bağlı kalmaz. Kopyalama yarıda kalırsa
        // güvenlik yedeği hemen geri yüklenir.
        let replace = std::fs::copy(&temp, database_path);
        let _ = std::fs::remove_file(&temp);
        if let Err(error) = replace {
            if let Err(rollback_error) = std::fs::copy(&safety, database_path) {
                self.startup_error = Some(format!(
                    "Yedek kopyalanamadı ve güvenlik yedeği geri yüklenemedi: \
                     {rollback_error}. Güvenlik yedeği: {}",
                    safety.display()
                ));
                return Err(AppError::Csv(format!(
                    "Yedek geri yüklenemedi: {error}. Veritabanı yeniden açılamadı. \
                     Güvenlik yedeği: {}",
                    safety.display()
                )));
            }
            remove_sidecars(database_path);
            let _ = RestoreJournal::remove(&journal_path);
            if let Ok(database) = Database::open(database_path) {
                self.database = Some(database);
                self.refresh_after_restore();
            }
            return Err(AppError::file_system(database_path, error));
        }

        match Database::open(database_path) {
            Ok(database) => {
                self.database = Some(database);
                if let Err(error) = journal.update_step(&journal_path, RestoreStep::Done) {
                    tracing::warn!(error = %error, "geri yükleme günlüğü done adımına alınamadı");
                }
                if let Err(error) = RestoreJournal::remove(&journal_path) {
                    tracing::warn!(error = %error, "geri yükleme günlüğü silinemedi");
                }
                self.refresh_after_restore();
                Ok(())
            }
            Err(error) => {
                remove_sidecars(database_path);
                let rollback = std::fs::copy(&safety, database_path).and_then(|_| {
                    remove_sidecars(database_path);
                    std::fs::metadata(database_path).map(|_| ())
                });
                if let Err(rollback_error) = rollback {
                    self.startup_error = Some(format!(
                        "Yedek geri yüklenemedi ve güvenlik yedeği kopyalanamadı: \
                         {rollback_error}. Güvenlik yedeği: {}",
                        safety.display()
                    ));
                    return Err(AppError::Csv(format!(
                        "Yedek geri yüklenemedi: {error}. Güvenlik yedeği: {}",
                        safety.display()
                    )));
                }
                let _ = RestoreJournal::remove(&journal_path);

                match Database::open(database_path) {
                    Ok(database) => {
                        self.database = Some(database);
                        self.refresh_after_restore();
                        Err(AppError::Csv(format!(
                            "Yedek geri yüklenemedi: {error}. Mevcut verileriniz korundu. \
                             Güvenlik yedeği: {}",
                            safety.display()
                        )))
                    }
                    Err(reopen_error) => {
                        self.startup_error = Some(format!(
                            "Yedek geri yüklenemedi ve mevcut veritabanı yeniden açılamadı: \
                             {reopen_error}. Güvenlik yedeği: {}",
                            safety.display()
                        ));
                        Err(AppError::Csv(format!(
                            "Yedek geri yüklenemedi: {error}. Mevcut veritabanı yeniden \
                             açılamadı: {reopen_error}. Güvenlik yedeği: {}",
                            safety.display()
                        )))
                    }
                }
            }
        }
    }

    fn refresh_after_restore(&mut self) {
        self.startup_error = None;
        self.status = None;
        self.status_error = None;
        self.about_open = false;
        self.scroll_reset = true;
        self.customer = CustomerState::default();
        self.product = ProductState::default();
        self.invoice = InvoiceState::default();
        self.cash = CashState::default();
        self.income_expense = IncomeExpenseState::default();
        self.reports = ReportsState::default();
        self.transfer.import_preview = None;
        self.transfer.import_result = None;
        self.transfer.import_error = None;
        self.transfer.pending_restore = None;
        self.view = View::Overview;

        self.refresh_status();
        self.refresh_customer_list();
        self.refresh_product_list();
        self.set_invoice_section(crate::invoice::InvoiceType::Purchase);
        self.enter_income_expense_section();
    }

    /// Bilgilendirmedeki dosyanın bulunduğu klasörü açar.
    pub fn open_transfer_folder(&self) {
        let Some(path) = self
            .transfer
            .notice
            .as_ref()
            .and_then(|notice| notice.path.as_ref())
        else {
            return;
        };
        open_in_explorer(path);
    }
}

/// Cari CSV satırını doğrular.
fn prepare_customer_row(
    row: &[String],
    index: &HashMap<String, usize>,
    line: usize,
    seen_codes: &mut HashSet<String>,
    database: &Database,
) -> ImportPreviewRow {
    let code = cell(row, index, "code").trim().to_owned();
    let display_code = if code.is_empty() {
        "(kod yok)".to_owned()
    } else {
        code.clone()
    };

    let mut draft = CustomerDraft {
        code,
        name: cell(row, index, "name").trim().to_owned(),
        phone: cell(row, index, "phone").trim().to_owned(),
        email: cell(row, index, "email").trim().to_owned(),
        address: cell(row, index, "address").trim().to_owned(),
        tax_office: cell(row, index, "tax_office").trim().to_owned(),
        tax_number: cell(row, index, "tax_number").trim().to_owned(),
        national_id: cell(row, index, "national_id").trim().to_owned(),
        contact_person: cell(row, index, "contact_person").trim().to_owned(),
        notes: cell(row, index, "notes").trim().to_owned(),
        ..CustomerDraft::default()
    };

    if let Err(reason) = parse_customer_kind(cell(row, index, "kind"), &mut draft.kind) {
        return error_row(line, display_code, reason);
    }
    if let Err(reason) = parse_active(cell(row, index, "active"), &mut draft.active) {
        return error_row(line, display_code, reason);
    }

    let validated = match draft.validate() {
        Ok(customer) => customer,
        Err(errors) => {
            return error_row(
                line,
                display_code,
                errors
                    .first_message()
                    .unwrap_or("Cari bilgileri geçersiz.")
                    .to_owned(),
            );
        }
    };

    let key = normalize_for_search(&validated.code);
    if !seen_codes.insert(key) {
        return error_row(line, display_code, "kod dosyada tekrar ediyor");
    }
    match database.customer_code_exists(&validated.code, None) {
        Ok(true) => return error_row(line, display_code, "kod zaten mevcut"),
        Ok(false) => {}
        Err(error) => {
            return error_row(
                line,
                display_code,
                format!("veritabanı kontrol edilemedi: {error}"),
            );
        }
    }

    ImportPreviewRow {
        line,
        code: display_code,
        status: ImportRowStatus::Ready,
        record: Some(ImportRecord::Customer(validated)),
    }
}

/// Ürün CSV satırını doğrular.
fn prepare_product_row(
    row: &[String],
    index: &HashMap<String, usize>,
    line: usize,
    seen_codes: &mut HashSet<String>,
    seen_barcodes: &mut HashSet<String>,
    database: &Database,
) -> ImportPreviewRow {
    let code = cell(row, index, "code").trim().to_owned();
    let display_code = if code.is_empty() {
        "(kod yok)".to_owned()
    } else {
        code.clone()
    };
    let barcode = cell(row, index, "barcode").trim().to_owned();

    let unit = match parse_unit(cell(row, index, "unit")) {
        Ok(unit) => unit,
        Err(reason) => return error_row(line, display_code, reason),
    };

    let purchase_price = match parse_money_field(cell(row, index, "purchase_price")) {
        Ok(value) => value,
        Err(reason) => return error_row(line, display_code, reason),
    };
    let sale_price = match parse_money_field(cell(row, index, "sale_price")) {
        Ok(value) => value,
        Err(reason) => return error_row(line, display_code, reason),
    };
    let min_stock = match csv::parse_quantity_import(cell(row, index, "min_stock")) {
        Ok(value) if value.is_negative() => {
            return error_row(line, display_code, "minimum stok negatif olamaz");
        }
        Ok(value) => value,
        Err(reason) => return error_row(line, display_code, reason),
    };
    let vat_rate = match parse_vat(cell(row, index, "vat_rate")) {
        Ok(value) => value,
        Err(reason) => return error_row(line, display_code, reason),
    };
    let mut active = true;
    if let Err(reason) = parse_active(cell(row, index, "active"), &mut active) {
        return error_row(line, display_code, reason);
    }

    let draft = ProductDraft {
        code,
        barcode,
        name: cell(row, index, "name").trim().to_owned(),
        unit,
        category: cell(row, index, "category").trim().to_owned(),
        purchase_price: purchase_price.format_turkish(),
        sale_price: sale_price.format_turkish(),
        vat_rate: vat_rate.to_string(),
        min_stock: min_stock.format_turkish(),
        notes: cell(row, index, "notes").trim().to_owned(),
        active,
        starting_stock: "0".to_owned(),
    };

    let validated = match draft.validate() {
        Ok(product) => product,
        Err(errors) => {
            return error_row(
                line,
                display_code,
                errors
                    .first_message()
                    .unwrap_or("Ürün bilgileri geçersiz.")
                    .to_owned(),
            );
        }
    };

    let key = normalize_for_search(&validated.code);
    if !seen_codes.insert(key) {
        return error_row(line, display_code, "kod dosyada tekrar ediyor");
    }
    if let Some(barcode) = validated
        .barcode
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        let key = normalize_for_search(barcode);
        if !seen_barcodes.insert(key) {
            return error_row(line, display_code, "barkod dosyada tekrar ediyor");
        }
    }
    match database.product_code_exists(&validated.code, None) {
        Ok(true) => return error_row(line, display_code, "kod zaten mevcut"),
        Ok(false) => {}
        Err(error) => {
            return error_row(
                line,
                display_code,
                format!("veritabanı kontrol edilemedi: {error}"),
            );
        }
    }
    if let Some(barcode) = validated
        .barcode
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        match database.product_barcode_exists(barcode, None) {
            Ok(true) => return error_row(line, display_code, "barkod zaten mevcut"),
            Ok(false) => {}
            Err(error) => {
                return error_row(
                    line,
                    display_code,
                    format!("veritabanı kontrol edilemedi: {error}"),
                );
            }
        }
    }

    ImportPreviewRow {
        line,
        code: display_code,
        status: ImportRowStatus::Ready,
        record: Some(ImportRecord::Product(validated)),
    }
}

fn error_row(line: usize, code: String, reason: impl Into<String>) -> ImportPreviewRow {
    ImportPreviewRow {
        line,
        code,
        status: ImportRowStatus::Error(reason.into()),
        record: None,
    }
}

fn cell<'a>(row: &'a [String], index: &HashMap<String, usize>, name: &str) -> &'a str {
    index
        .get(&normalize_for_search(name))
        .and_then(|position| row.get(*position))
        .map(String::as_str)
        .unwrap_or_default()
}

fn parse_customer_kind(value: &str, target: &mut CustomerType) -> std::result::Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    let normalized = normalize_for_search(trimmed);
    let kind = CustomerType::ALL.into_iter().find(|kind| {
        normalized == normalize_for_search(kind.label())
            || normalized == normalize_for_search(kind.as_db())
    });
    match kind {
        Some(kind) => {
            *target = kind;
            Ok(())
        }
        None => Err("cari türü geçersiz (Müşteri, Tedarikçi veya Her ikisi olmalı)".to_owned()),
    }
}

fn parse_active(value: &str, target: &mut bool) -> std::result::Result<(), String> {
    let normalized = normalize_for_search(value.trim());
    match normalized.as_str() {
        "" | "evet" | "true" | "1" | "aktif" => {
            *target = true;
            Ok(())
        }
        "hayir" | "false" | "0" | "pasif" => {
            *target = false;
            Ok(())
        }
        _ => Err("aktif değeri Evet veya Hayır olmalı".to_owned()),
    }
}

fn parse_unit(value: &str) -> std::result::Result<Unit, String> {
    let normalized = normalize_for_search(value.trim());
    if normalized.is_empty() {
        return Ok(Unit::default());
    }
    Unit::ALL
        .into_iter()
        .find(|unit| {
            let label_without_detail = unit.label().split(" (").next().unwrap_or_default();
            normalized == normalize_for_search(unit.as_db())
                || normalized == normalize_for_search(unit.short_label())
                || normalized == normalize_for_search(unit.label())
                || normalized == normalize_for_search(label_without_detail)
        })
        .ok_or_else(|| "ölçü birimi geçersiz".to_owned())
}

fn parse_money_field(value: &str) -> std::result::Result<crate::money::Money, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(crate::money::Money::ZERO);
    }
    match crate::money::Money::parse(trimmed) {
        Ok(money) if money.kurus() < 0 => Err("tutar negatif olamaz".to_owned()),
        Ok(money) => Ok(money),
        Err(_) => Err(format!("geçersiz tutar: '{trimmed}'")),
    }
}

fn parse_vat(value: &str) -> std::result::Result<i64, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(20);
    }
    match trimmed.parse::<i64>() {
        Ok(rate) if (0..=100).contains(&rate) => Ok(rate),
        _ => Err("KDV oranı 0 ile 100 arasında tam sayı olmalı".to_owned()),
    }
}

/// Dosya adında kullanılamayan karakterleri temizler.
pub fn sanitize_component(value: &str) -> String {
    let sanitized: String = value
        .trim()
        .chars()
        .map(|character| match character {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            character if character.is_control() => '_',
            character => character,
        })
        .collect();
    if sanitized.is_empty() {
        "kayit".to_owned()
    } else {
        sanitized
    }
}

fn paths_equal(left: &Path, right: &Path) -> bool {
    match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

fn unique_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "yedek".to_owned());
    for counter in 1..10_000 {
        let candidate = parent.join(format!("{name}-{counter}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    path.to_path_buf()
}

fn remove_sidecars(path: &Path) {
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

fn open_in_explorer(path: &Path) {
    #[cfg(target_os = "windows")]
    {
        let argument = format!("/select,{}", path.display());
        let _ = std::process::Command::new("explorer.exe")
            .arg(argument)
            .spawn();
    }
    #[cfg(not(target_os = "windows"))]
    {
        let parent = path.parent().unwrap_or(path);
        let _ = std::process::Command::new("xdg-open").arg(parent).spawn();
    }
}
