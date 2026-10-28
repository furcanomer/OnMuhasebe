// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Uygulama durumu: yapılandırma, veritabanı ve arayüzü bir araya getirir.
//!
//! [`OnMuhasebeApp`] arayüz katmanının tek veri kaynağıdır. Arayüz modülleri
//! veritabanına doğrudan erişmez; yalnızca bu tipin sunduğu okuma metotlarını
//! ve komutlarını kullanır.

use eframe::CreationContext;

use crate::backup::{self, BackupKind};
use crate::config::AppConfig;
use crate::customer::FieldErrors;
use crate::db::{Database, DatabaseStatus};
use crate::error::{AppError, Result};
use crate::invoice::InvoiceType;
use crate::product::FieldErrors as ProductFieldErrors;
use crate::restore_journal::RestoreJournal;
use crate::settings::AppSettings;
use crate::ui;

mod auth_screen;
mod cash_screen;
mod customer_screen;
mod export_screen;
mod income_expense_screen;
mod invoice_screen;
mod navigation_links;
mod onboarding_screen;
mod product_screen;
mod report_screen;
mod search_screen;
mod settings_screen;
mod shortcuts;
mod transfer_screen;

#[cfg(test)]
mod stage9_tests;

pub use auth_screen::{AuthScreen, AuthState, Session};
pub use cash_screen::{CashMovement, CashScreen, CashState, CashTab, MovementTab, PaymentOrigin};
pub use customer_screen::{CustomerScreen, CustomerState};
pub use income_expense_screen::{
    EntryOrigin, IncomeExpenseScreen, IncomeExpenseState, IncomeExpenseTab,
};
pub use invoice_screen::{InvoiceScreen, InvoiceState};
pub use onboarding_screen::OnboardingState;
pub use product_screen::{OverviewMetrics, ProductScreen, ProductState};
pub use report_screen::ReportsState;
pub use search_screen::SearchPaletteState;
pub use settings_screen::{PasswordFormState, SettingsState, SettingsTab, UserFormState};
pub use shortcuts::{SHORTCUTS, ShortcutAction, ShortcutDef};
pub use transfer_screen::{
    CUSTOMER_TEMPLATE_HEADERS, ImportKind, ImportPreview, ImportPreviewRow, ImportRecord,
    ImportResultView, ImportRowStatus, NoticeKind, PRODUCT_TEMPLATE_HEADERS, PendingRestore,
    TransferNotice, TransferState,
};

/// Arayüzde gösterilebilen ana görünümler (sol menüdeki bölümler).
///
/// [`View`], gezinme menüsünün, üst bilgi çubuğunun ve içerik yönlendirmesinin
/// **tek kaynağıdır**. Yeni bir bölüm geliştirildiğinde yalnızca buraya bir
/// varyant eklenir; menü ve başlık kendiliğinden güncellenir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum View {
    /// Genel bakış / karşılama ekranı.
    Overview,
    /// Cari hesaplar.
    Customers,
    /// Ürün ve stok.
    Inventory,
    /// Alış faturaları.
    PurchaseInvoices,
    /// Satış faturaları.
    SalesInvoices,
    /// Kasa ve banka.
    CashAndBank,
    /// Gelir ve gider.
    IncomeExpense,
    /// Raporlar.
    Reports,
    /// Ayarlar.
    Settings,
}

impl View {
    /// Menüde gösterilecek sıraya göre tüm görünümler.
    pub const ALL: [Self; 9] = [
        Self::Overview,
        Self::Customers,
        Self::Inventory,
        Self::PurchaseInvoices,
        Self::SalesInvoices,
        Self::CashAndBank,
        Self::IncomeExpense,
        Self::Reports,
        Self::Settings,
    ];

    /// Menüde ve üst bilgi çubuğunda gösterilen başlık.
    pub fn title(self) -> &'static str {
        match self {
            Self::Overview => "Genel Bakış",
            Self::Customers => "Cari Hesaplar",
            Self::Inventory => "Ürün ve Stok",
            Self::PurchaseInvoices => "Alış Faturaları",
            Self::SalesInvoices => "Satış Faturaları",
            Self::CashAndBank => "Kasa ve Banka",
            Self::IncomeExpense => "Gelir ve Gider",
            Self::Reports => "Raporlar",
            Self::Settings => "Ayarlar",
        }
    }

    /// Bir cümlelik bölüm açıklaması (kısayol ipucu ve bilgilendirme ekranı).
    pub fn description(self) -> &'static str {
        match self {
            Self::Overview => "Programın genel durumu, modül kısayolları ve veritabanı bilgileri.",
            Self::Customers => {
                "Müşteri ve tedarikçi kartları: iletişim ve vergi bilgileri, durum takibi."
            }
            Self::Inventory => "Ürün ve hizmet kartları, stok miktarları, giriş/çıkış hareketleri.",
            Self::PurchaseInvoices => "Tedarikçilerden alınan faturalar ve fatura kalemleri.",
            Self::SalesInvoices => "Müşterilere kesilen faturalar ve fatura kalemleri.",
            Self::CashAndBank => "Kasa ve banka hesapları, tahsilat ve ödeme hareketleri.",
            Self::IncomeExpense => "Gelir ve gider kayıtları, gider kategorileri.",
            Self::Reports => "Cari, stok, satış/alış, kasa/banka ve gelir/gider raporları.",
            Self::Settings => "Şirket bilgileri, varsayılan değerler ve yedekleme ayarları.",
        }
    }

    /// Bu bölümün ekranı geliştirildi mi?
    ///
    /// Henüz geliştirilmemiş bölümler için [`crate::ui::placeholder`] ekranı
    /// gösterilir; menüde yine de tıklanabilir kalırlar.
    pub fn is_ready(self) -> bool {
        matches!(
            self,
            Self::Overview
                | Self::Customers
                | Self::Inventory
                | Self::PurchaseInvoices
                | Self::SalesInvoices
                | Self::CashAndBank
                | Self::IncomeExpense
                | Self::Reports
                | Self::Settings
        )
    }
}

/// Form alanlarına programatik odaklanma hedefi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    /// Cari kodu.
    CustomerCode,
    /// Cari adı.
    CustomerName,
    /// Ürün kodu.
    ProductCode,
    /// Ürün adı.
    ProductName,
    /// Kasa/banka hesap adı.
    CashName,
    /// Kategori adı.
    CategoryName,
    /// Fatura numarası.
    InvoiceNumber,
    /// Fatura carisi.
    InvoiceCustomer,
    /// Tahsilat/ödeme carisi.
    PaymentCustomer,
    /// Tahsilat/ödeme tutarı.
    PaymentAmount,
    /// Gelir/gider kategorisi.
    EntryCategory,
    /// Gelir/gider tutarı.
    EntryAmount,
    /// Stok hareketi miktarı.
    MovementQuantity,
    /// Global arama alanı.
    SearchQuery,
    /// Sihirbaz şirket unvanı.
    OnboardingCompanyName,
    /// Sihirbaz hesap adı.
    OnboardingCashName,
}

/// Değişiklik onayı bekleyen form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelTarget {
    /// Cari formu.
    Customer,
    /// Ürün formu.
    Product,
    /// Stok hareketi formu.
    Movement,
    /// Fatura formu.
    Invoice,
    /// Kasa/banka hesap formu.
    CashAccount,
    /// Tahsilat/ödeme formu.
    Payment,
    /// Gelir/gider hareket formu.
    IncomeEntry,
    /// Kategori formu.
    Category,
}

impl CancelTarget {
    /// Onay penceresinde gösterilen form adı.
    pub fn label(self) -> &'static str {
        match self {
            Self::Customer => "Cari hesap",
            Self::Product => "Ürün",
            Self::Movement => "Stok hareketi",
            Self::Invoice => "Fatura",
            Self::CashAccount => "Kasa/banka hesabı",
            Self::Payment => "Tahsilat/ödeme",
            Self::IncomeEntry => "Gelir/gider",
            Self::Category => "Kategori",
        }
    }
}

/// Uygulamanın çalışma zamanı durumu.
pub struct OnMuhasebeApp {
    config: AppConfig,
    database: Option<Database>,
    startup_error: Option<String>,
    status: Option<DatabaseStatus>,
    status_error: Option<String>,
    view: View,
    about_open: bool,
    scroll_reset: bool,
    auth: AuthState,
    settings: AppSettings,
    settings_screen: SettingsState,
    startup_backup_at: Option<std::time::Instant>,
    startup_backup_warning: Option<String>,
    customer: CustomerState,
    product: ProductState,
    invoice: InvoiceState,
    cash: CashState,
    income_expense: IncomeExpenseState,
    reports: ReportsState,
    transfer: TransferState,
    onboarding: OnboardingState,
    search: SearchPaletteState,
    restore_recovery: Option<RestoreJournal>,
    restore_recovery_error: Option<String>,
    pub(crate) shortcuts_open: bool,
    pub(crate) logout_confirm: bool,
    pub(crate) cancel_confirm: Option<CancelTarget>,
    pub(crate) error_details_open: bool,
    focus_target: Option<FocusTarget>,
    last_window_title: Option<String>,
    notice_started: Option<(String, std::time::Instant)>,
}

impl OnMuhasebeApp {
    /// Arayüz ilk çizilmeden önce çağrılır: tema uygulanır ve veritabanı açılır.
    ///
    /// Veritabanı açılamazsa uygulama **yine de** başlatılır; hata, karşılama
    /// ekranında kullanıcıya açıkça gösterilir. Böylece ekransız (console
    /// penceresi olmayan) bir derlemede de sorun sessizce kaybolmaz.
    pub fn new(context: &CreationContext<'_>, config: AppConfig) -> Self {
        let mut app = Self::with_config(&context.egui_ctx, config);
        // Otomatik yedek, ilk karede "Yedek alınıyor..." mesajı görüldükten
        // sonra (bkz. `run_startup_backup`) çalışır.
        // İlk kurulumda veritabanı henüz boştur; anlamsız bir boş yedek alıp
        // 24 saatlik sayacı başlatmak yerine ilk yönetici oluşturulduktan
        // sonraki açılışta otomatik yedeğe başlanır.
        if app.database.is_some()
            && app.auth.screen != AuthScreen::FirstAdmin
            && app
                .settings
                .should_auto_backup(chrono::Local::now().naive_local())
        {
            app.startup_backup_at = Some(std::time::Instant::now());
            context
                .egui_ctx
                .request_repaint_after(std::time::Duration::from_millis(500));
        }
        app
    }

    /// Tema uygulanmış bir egui bağlamıyla uygulama durumunu oluşturur.
    ///
    /// [`Self::new`] yalnızca bu fonksiyona devreder; böylece uygulama durumu
    /// pencere oluşturmadan (örneğin testlerde) da kurulabilir.
    pub fn with_config(egui_context: &egui::Context, config: AppConfig) -> Self {
        ui::theme::apply(egui_context);

        // Yarıda kalmış geri yükleme varsa yalnızca tespit edilir; hiçbir
        // seçenek otomatik uygulanmaz. Karar kullanıcıya aittir.
        let restore_recovery = match crate::restore_journal::detect(config.data_dir()) {
            Ok(journal) => journal,
            Err(error) => {
                tracing::warn!(error = %error, "geri yükleme günlüğü okunamadı");
                None
            }
        };

        let (database, startup_error) = match Database::open(config.database_path()) {
            Ok(database) => (Some(database), None),
            Err(error) => (None, Some(error.to_string())),
        };

        let mut app = Self {
            config,
            database,
            startup_error,
            status: None,
            status_error: None,
            view: View::Overview,
            about_open: false,
            scroll_reset: false,
            auth: AuthState::default(),
            settings: AppSettings::default(),
            settings_screen: SettingsState::default(),
            startup_backup_at: None,
            startup_backup_warning: None,
            customer: CustomerState::default(),
            product: ProductState::default(),
            invoice: InvoiceState::default(),
            cash: CashState::default(),
            income_expense: IncomeExpenseState::default(),
            reports: ReportsState::default(),
            transfer: TransferState::default(),
            onboarding: OnboardingState::default(),
            search: SearchPaletteState::default(),
            restore_recovery,
            restore_recovery_error: None,
            shortcuts_open: false,
            logout_confirm: false,
            cancel_confirm: None,
            error_details_open: false,
            focus_target: None,
            last_window_title: None,
            notice_started: None,
        };
        app.auth.screen = match app.database.as_ref() {
            Some(database) => match database.user_count() {
                Ok(0) => AuthScreen::FirstAdmin,
                Ok(_) => AuthScreen::Login,
                Err(error) => {
                    app.startup_error = Some(error.to_string());
                    AuthScreen::Login
                }
            },
            None => AuthScreen::Login,
        };
        app.load_settings();
        if app
            .settings
            .clock_skew_detected(chrono::Local::now().naive_local())
        {
            tracing::warn!(
                last_auto_backup_at = ?app.settings.last_auto_backup_at,
                "son otomatik yedek zamanı gelecekte; saat kayması tespit edildi, \
                 otomatik yedek atlandı"
            );
        }
        app.refresh_status();
        app.refresh_customer_list();
        app.refresh_product_list();
        app.set_invoice_section(InvoiceType::Purchase);
        app.enter_income_expense_section();
        app
    }

    /// Veritabanı durum özetini yeniden okur.
    pub fn refresh_status(&mut self) {
        self.status_error = None;

        let Some(database) = &self.database else {
            self.status = None;
            return;
        };

        match database.status() {
            Ok(status) => self.status = Some(status),
            Err(error) => self.status_error = Some(error.to_string()),
        }
    }

    /// Uygulama yapılandırması.
    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    /// Günlük dosyalarının klasörü.
    pub fn logs_dir(&self) -> std::path::PathBuf {
        self.config.logs_dir()
    }

    /// Son günlük dosyasının yolu.
    pub fn latest_log_file(&self) -> Option<std::path::PathBuf> {
        crate::logging::latest_log_file(&self.config.logs_dir())
    }

    /// Log klasörünü işletim sistemi dosya yöneticisinde açar.
    pub fn open_logs_folder(&self) {
        let directory = self.config.logs_dir();
        if let Err(error) = std::fs::create_dir_all(&directory) {
            tracing::error!(error = %error, "log klasörü oluşturulamadı");
            return;
        }
        #[cfg(target_os = "windows")]
        let result = std::process::Command::new("explorer")
            .arg(&directory)
            .spawn();
        #[cfg(not(target_os = "windows"))]
        let result = std::process::Command::new("xdg-open")
            .arg(&directory)
            .spawn();
        if let Err(error) = result {
            tracing::error!(error = %error, "log klasörü açılamadı");
        }
    }

    /// Açık veritabanı bağlantısı (açılamadıysa `None`).
    pub fn database(&self) -> Option<&Database> {
        self.database.as_ref()
    }

    /// Veritabanı açılırken oluşan hata mesajı (varsa).
    pub fn startup_error(&self) -> Option<&str> {
        self.startup_error.as_deref()
    }

    /// Yarıda kalmış geri yükleme bilgisi (varsa). Karar kullanıcıya aittir.
    pub fn restore_recovery(&self) -> Option<&RestoreJournal> {
        self.restore_recovery.as_ref()
    }

    /// Geri yükleme kurtarma seçeneği uygulanırken oluşan hata (varsa).
    pub fn restore_recovery_error(&self) -> Option<&str> {
        self.restore_recovery_error.as_deref()
    }

    /// Veritabanı durum özeti (okunamadıysa `None`).
    pub fn status(&self) -> Option<&DatabaseStatus> {
        self.status.as_ref()
    }

    /// Durum okunurken oluşan hata mesajı (varsa).
    pub fn status_error(&self) -> Option<&str> {
        self.status_error.as_deref()
    }

    /// Aktif görünüm.
    pub fn view(&self) -> View {
        self.view
    }

    /// Aktif görünümü değiştirir.
    ///
    /// Cari hesap bölümüne geçildiğinde liste tazelenir ve bölümün alt ekranı
    /// listeye döner; böylece kullanıcı her zaman güncel kayıtları görür.
    pub fn set_view(&mut self, view: View) {
        let changed = self.view != view;
        self.view = view;

        if changed {
            self.scroll_reset = true;

            if view == View::Customers {
                self.customer.screen = CustomerScreen::List;
                self.customer.errors = FieldErrors::default();
                self.customer.general_error = None;
                self.refresh_customer_list();
            }

            if view == View::Inventory {
                self.product.screen = ProductScreen::List;
                self.product.errors = ProductFieldErrors::default();
                self.product.general_error = None;
                self.refresh_product_list();
            }

            if view == View::CashAndBank {
                self.enter_cash_section();
            }

            if view == View::IncomeExpense {
                self.enter_income_expense_section();
            }

            if view == View::Reports {
                self.enter_reports_section();
            }

            if view == View::Settings {
                self.restrict_settings_for_operator();
                self.refresh_settings_users();
                self.refresh_backup_list();
            }

            match view {
                View::PurchaseInvoices => self.set_invoice_section(InvoiceType::Purchase),
                View::SalesInvoices => self.set_invoice_section(InvoiceType::Sales),
                _ => {}
            }
        }
    }

    /// İçerik alanının en baştan gösterilmesini ister.
    ///
    /// Ekran değişimlerinde çağrılır; aksi hâlde egui önceki kaydırma konumunu
    /// korur ve kullanıcı yeni ekranın ortasında kalır.
    pub fn request_scroll_reset(&mut self) {
        self.scroll_reset = true;
    }

    /// Bekleyen kaydırma isteğini tüketir (istek yalnızca bir kez işlenir).
    pub fn take_scroll_reset(&mut self) -> bool {
        std::mem::take(&mut self.scroll_reset)
    }

    /// "Hakkında" penceresi açık mı?
    pub fn about_open(&self) -> bool {
        self.about_open
    }

    /// "Hakkında" penceresinin görünürlüğünü ayarlar.
    pub fn set_about_open(&mut self, open: bool) {
        self.about_open = open;
    }

    /// Bir alanın odaklanma isteğini tüketir.
    pub fn take_focus(&mut self, target: FocusTarget) -> bool {
        if self.focus_target == Some(target) {
            self.focus_target = None;
            true
        } else {
            false
        }
    }

    /// Bir alana odaklanma isteği kaydeder.
    pub fn request_focus(&mut self, target: FocusTarget) {
        self.focus_target = Some(target);
    }

    /// Testler için bekleyen odak hedefi.
    pub fn pending_focus(&self) -> Option<FocusTarget> {
        self.focus_target
    }

    /// Pencere başlığı: uygulama adı, aktif kullanıcı/ekran ve sürüm.
    pub fn window_title(&self) -> String {
        let who = match self.auth.screen {
            AuthScreen::FirstAdmin => "İlk kurulum".to_owned(),
            AuthScreen::Login => "Giriş".to_owned(),
            AuthScreen::Locked => "Kilitli".to_owned(),
            AuthScreen::Authenticated => {
                let user = self
                    .current_actor()
                    .map(|actor| actor.label().to_owned())
                    .unwrap_or_else(|| "Oturum".to_owned());
                format!("{user} · {}", self.view.title())
            }
        };
        format!("{} — {} — v{}", crate::APP_NAME, who, crate::APP_VERSION)
    }

    /// Başlık değiştiyse pencereye gönderir.
    pub fn update_window_title(&mut self, context: &egui::Context) {
        let title = self.window_title();
        if self.last_window_title.as_deref() != Some(title.as_str()) {
            context.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_window_title = Some(title);
        }
    }

    /// Hata ayrıntıları penceresinde gösterilecek son hata.
    pub fn last_error_summary(&self) -> Option<String> {
        self.startup_error
            .clone()
            .or_else(|| self.status_error.clone())
            .or_else(|| self.auth.error.clone())
            .or_else(|| self.customer.general_error.clone())
            .or_else(|| self.customer.list_error.clone())
            .or_else(|| self.product.general_error.clone())
            .or_else(|| self.product.list_error.clone())
            .or_else(|| self.invoice.general_error.clone())
            .or_else(|| self.invoice.list_error.clone())
            .or_else(|| self.cash.general_error.clone())
            .or_else(|| self.cash.account_list_error.clone())
            .or_else(|| self.income_expense.general_error.clone())
            .or_else(|| self.income_expense.entry_list_error.clone())
            .or_else(|| self.income_expense.category_list_error.clone())
            .or_else(|| self.settings_screen.settings_error.clone())
            .or_else(|| self.settings_screen.backup_error.clone())
            .or_else(|| self.settings_screen.users_error.clone())
            .or_else(|| {
                self.transfer_state()
                    .notice
                    .as_ref()
                    .map(|notice| notice.message.clone())
            })
    }

    /// Başarı bildirimlerini 5 saniye sonra otomatik kapatır.
    pub fn tick_notices(&mut self) {
        let Some(text) = self.active_notice_text() else {
            self.notice_started = None;
            return;
        };

        let expired = self
            .notice_started
            .as_ref()
            .is_some_and(|(previous, started)| {
                previous == &text && started.elapsed() >= std::time::Duration::from_secs(5)
            });
        if expired {
            self.clear_notices();
            self.notice_started = None;
        } else if self
            .notice_started
            .as_ref()
            .is_none_or(|(previous, _)| previous != &text)
        {
            self.notice_started = Some((text, std::time::Instant::now()));
        }
    }

    fn active_notice_text(&self) -> Option<String> {
        self.customer
            .notice
            .clone()
            .or_else(|| self.product.notice.clone())
            .or_else(|| self.invoice.notice.clone())
            .or_else(|| self.cash.notice.clone())
            .or_else(|| self.income_expense.notice.clone())
            .or_else(|| self.settings_screen.settings_notice.clone())
            .or_else(|| self.settings_screen.users_notice.clone())
            .or_else(|| self.settings_screen.password_notice.clone())
            .or_else(|| self.settings_screen.backup_notice.clone())
            .or_else(|| self.auth.notice.clone())
            .or_else(|| {
                self.transfer
                    .notice
                    .as_ref()
                    .map(|notice| notice.message.clone())
            })
    }

    fn clear_notices(&mut self) {
        self.customer.notice = None;
        self.product.notice = None;
        self.invoice.notice = None;
        self.cash.notice = None;
        self.income_expense.notice = None;
        self.settings_screen.settings_notice = None;
        self.settings_screen.users_notice = None;
        self.settings_screen.password_notice = None;
        self.settings_screen.backup_notice = None;
        self.auth.notice = None;
        self.transfer.notice = None;
    }

    /// Programın sağlıklı çalıştığının tek satırlık özeti.
    pub fn is_healthy(&self) -> bool {
        self.database.is_some() && self.status.is_some() && self.status_error.is_none()
    }

    /// Açılış otomatik yedeği sürüyor mu / bekliyor mu?
    pub fn startup_backup_pending(&self) -> bool {
        self.startup_backup_at.is_some()
    }

    /// Açılış otomatik yedeği başarısız olduysa kullanıcıya gösterilecek uyarı.
    pub fn startup_backup_warning(&self) -> Option<&str> {
        self.startup_backup_warning.as_deref()
    }

    /// Açılış otomatik yedeğini hemen çalıştırır (GUI doğrulaması ve testler).
    pub fn run_startup_auto_backup(&mut self) -> Result<()> {
        self.startup_backup_at = None;
        let result = self.create_auto_backup();
        match &result {
            Ok(path) => {
                tracing::info!(path = %path.display(), "otomatik yedek alındı");
                self.startup_backup_warning = None;
            }
            Err(error) => {
                tracing::error!(error = %error, "otomatik yedek alınamadı");
                self.startup_backup_warning = Some(
                    "Otomatik yedek alınamadı. Ayarlar > Yedekleme'den manuel deneyin.".to_owned(),
                );
            }
        }
        self.refresh_backup_list();
        result.map(|_| ())
    }

    fn create_auto_backup(&mut self) -> Result<std::path::PathBuf> {
        let directory = self.config.backups_dir();
        let target = backup::unique_backup_path(&directory, BackupKind::Auto);
        {
            let Some(database) = self.database.as_ref() else {
                return Err(AppError::Maintenance("Veritabanı açık değil.".to_owned()));
            };
            backup::create_backup(database, &target)?;
        }
        if let Some(database) = self.database.as_ref() {
            self.settings.mark_auto_backup_now(database)?;
        }
        let _ = backup::rotate_auto_backups(
            &directory,
            self.settings.rotation,
            chrono::Local::now().date_naive(),
        );
        Ok(target)
    }
}

impl eframe::App for OnMuhasebeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let context = ui.ctx().clone();
        let engaged = context.input(|input| {
            !input.events.is_empty() || input.pointer.any_down() || !input.keys_down.is_empty()
        });
        if engaged {
            self.touch_activity();
        }
        self.check_auto_lock();

        if let Some(started) = self.startup_backup_at {
            if started.elapsed() < std::time::Duration::from_millis(500) {
                context.request_repaint_after(std::time::Duration::from_millis(100));
            } else {
                let _ = self.run_startup_auto_backup();
            }
        }

        self.update_window_title(&context);
        self.tick_notices();
        ui::overlays::show_restore_recovery(&context, self);

        if !self.is_authenticated() {
            ui::auth::show(ui, self);
            return;
        }

        self.handle_shortcuts(&context);
        self.tick_search();
        if self.search.changed_at.is_some() {
            context.request_repaint_after(std::time::Duration::from_millis(30));
        }
        ui::layout::draw(ui, self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tum_gorunumlerin_basligi_ve_aciklamasi_vardir() {
        for view in View::ALL {
            assert!(!view.title().is_empty(), "{view:?} başlığı boş olmamalı");
            assert!(
                !view.description().is_empty(),
                "{view:?} açıklaması boş olmamalı"
            );
        }
    }

    #[test]
    fn basliklar_birbirinden_farklidir() {
        for (index, view) in View::ALL.iter().enumerate() {
            for other in &View::ALL[index + 1..] {
                assert_ne!(view.title(), other.title());
            }
        }
    }

    #[test]
    fn hazir_bolumler_dogru_bildirilir() {
        for view in View::ALL {
            let expected = matches!(
                view,
                View::Overview
                    | View::Customers
                    | View::Inventory
                    | View::PurchaseInvoices
                    | View::SalesInvoices
                    | View::CashAndBank
                    | View::IncomeExpense
                    | View::Reports
                    | View::Settings
            );
            assert_eq!(view.is_ready(), expected, "{view:?}");
        }
    }

    #[test]
    fn yeni_bir_gorunum_ilk_sirada_genel_bakis_bulunur() {
        assert_eq!(View::ALL.first(), Some(&View::Overview));
    }
}
