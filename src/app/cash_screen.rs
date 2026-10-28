// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Kasa/banka ve tahsilat/ödeme ekranlarının durumu ve komutları.
//!
//! Arayüz katmanı kasa/banka ve tahsilat/ödeme işlemleri için **hiçbir SQL
//! çalıştırmaz**. Ekranın gösterdiği tüm veri [`CashState`] içinde tutulur ve
//! tüm değişiklikler bu modüldeki komut metotlarıyla yapılır.
//!
//! ## Neden tek görünüm, iki sekme?
//!
//! "Kasa ve Banka" bölümü, Hesaplar ve Hareketler olmak üzere iki sekmeden
//! oluşur. Sekmeler [`CashState::tab`] ile yönetilir; böylece sol menüye yeni
//! bir bölüm eklemek gerekmez ve mevcut [`crate::app::View`] yapısı **en az**
//! ölçüde değişir. Alt ekranlar [`CashScreen`] ile ayrılır.
//!
//! ## Kaydetme akışı (tahsilat/ödeme)
//!
//! ```text
//! Form (PaymentDraft) --validate--> ValidatedPayment --(onay)--> [tek transaction]
//!        ^                                  |                        |
//!        |   hata: FieldErrors (girdi korunur) |                ödeme + dağıtımlar
//!        +----------------------------------+                 + fatura paid_amount
//!                                                             + kasa bakiyesi
//! ```
//!
//! ## Kayıt silinmez
//!
//! Kasa/banka hesabı **pasife alınır** ([`OnMuhasebeApp::toggle_cash_account_active`]);
//! tahsilat/ödeme ise **iptal edilir** ([`OnMuhasebeApp::submit_payment_cancel`])
//! ve etkisi geri alınır. Düzeltme, iptal + yeni kayıt ile yapılır.

use crate::cash::{
    AccountType, CashAccount, CashAccountDraft, CashAccountFilter, FieldErrors as CashFieldErrors,
};
use crate::customer::{Customer, CustomerFilter};
use crate::error::AppError;
use crate::income_expense::{
    EntryFilter, EntryStatus, EntrySummary, EntryType, IncomeExpenseEntry,
};
use crate::invoice::{InvoiceFilter, InvoiceType};
use crate::money::Money;
use crate::payment::{
    AllocationDraft, FieldErrors as PaymentFieldErrors, Payment, PaymentAllocation, PaymentDraft,
    PaymentFilter, PaymentSummary, PaymentType, ValidatedPayment, validate_cancel_reason,
};

use super::{FocusTarget, OnMuhasebeApp};

/// "Kasa ve Banka" bölümünün sekmeleri.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CashTab {
    /// Kasa/banka hesapları (varsayılan).
    #[default]
    Accounts,
    /// Tahsilat/ödeme hareketleri.
    Movements,
}

impl CashTab {
    /// Sekme sırası.
    pub const ALL: [Self; 2] = [Self::Accounts, Self::Movements];

    /// Arayüzde gösterilen sekme adı.
    pub fn label(self) -> &'static str {
        match self {
            Self::Accounts => "Hesaplar",
            Self::Movements => "Hareketler",
        }
    }
}

/// Kasa/banka bölümündeki alt ekranlar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CashScreen {
    /// Hesap listesi (varsayılan).
    #[default]
    AccountList,
    /// Hesap formu (yeni/düzenle).
    AccountForm,
    /// Hareket listesi.
    MovementList,
    /// Tahsilat/ödeme formu.
    MovementForm,
    /// Tahsilat/ödeme ayrıntısı.
    MovementDetail,
}

/// Hareket listesindeki alt sekmeler (tür filtresi).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MovementTab {
    /// Tümü: tahsilat, ödeme, gelir ve gider.
    #[default]
    All,
    /// Yalnızca tahsilatlar.
    Collections,
    /// Yalnızca ödemeler.
    Payments,
    /// Yalnızca gelirler.
    Incomes,
    /// Yalnızca giderler.
    Expenses,
}

impl MovementTab {
    /// Alt sekme sırası.
    pub const ALL: [Self; 5] = [
        Self::All,
        Self::Collections,
        Self::Payments,
        Self::Incomes,
        Self::Expenses,
    ];

    /// Arayüzde gösterilen ad.
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "Tümü",
            Self::Collections => "Tahsilatlar",
            Self::Payments => "Ödemeler",
            Self::Incomes => "Gelirler",
            Self::Expenses => "Giderler",
        }
    }

    /// Bu sekmenin uyguladığı tahsilat/ödeme türü (`None` = ölçüt yok).
    pub fn payment_type(self) -> Option<PaymentType> {
        match self {
            Self::Collections => Some(PaymentType::Tahsilat),
            Self::Payments => Some(PaymentType::Odeme),
            Self::All | Self::Incomes | Self::Expenses => None,
        }
    }

    /// Bu sekmenin uyguladığı gelir/gider türü (`None` = ölçüt yok).
    pub fn entry_type(self) -> Option<EntryType> {
        match self {
            Self::Incomes => Some(EntryType::Gelir),
            Self::Expenses => Some(EntryType::Gider),
            Self::All | Self::Collections | Self::Payments => None,
        }
    }

    /// Tahsilat/ödeme kayıtları bu sekmede listelenir mi?
    pub fn shows_payments(self) -> bool {
        matches!(self, Self::All | Self::Collections | Self::Payments)
    }

    /// Gelir/gider kayıtları bu sekmede listelenir mi?
    pub fn shows_entries(self) -> bool {
        matches!(self, Self::All | Self::Incomes | Self::Expenses)
    }
}

/// Tahsilat/ödeme formunun nereden açıldığı (kayıt sonrası dönüş yeri).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaymentOrigin {
    /// Hareket listesinden (kayıt sonrası ayrıntıya gidilir).
    #[default]
    Movements,
    /// Bir faturanın ayrıntısından (kayıt sonrası faturaya dönülür).
    Invoice(i64),
    /// Bir cari hesabın ayrıntısından (kayıt sonrası ayrıntıya gidilir).
    Customer(i64),
}

/// Kasa/banka hareket listesinde birleştirilen kayıt türü.
///
/// "Tümü" sekmesi tahsilat/ödemeler ile gelir/giderleri tarihe göre birlikte
/// gösterir. Kaynak etiketi (`Tahsilat` / `Ödeme` / `Gelir` / `Gider`) liste
/// satırında ayrı bir sütun olarak sunulur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CashMovement {
    /// Cariye bağlı tahsilat veya ödeme.
    Payment(Payment),
    /// Cariye bağlı olmayan gelir veya gider.
    Entry(IncomeExpenseEntry),
}

impl CashMovement {
    /// Kaydın birincil anahtarı.
    pub fn record_id(&self) -> i64 {
        match self {
            Self::Payment(payment) => payment.id,
            Self::Entry(entry) => entry.id,
        }
    }

    /// Kayıt tarihi (`YYYY-MM-DD`).
    pub fn date(&self) -> &str {
        match self {
            Self::Payment(payment) => &payment.payment_date,
            Self::Entry(entry) => &entry.entry_date,
        }
    }

    /// Kayıt tutarı (her zaman pozitif).
    pub fn amount(&self) -> Money {
        match self {
            Self::Payment(payment) => payment.amount,
            Self::Entry(entry) => entry.amount,
        }
    }

    /// Kaynak etiketi: `Tahsilat` / `Ödeme` / `Gelir` / `Gider`.
    pub fn source_label(&self) -> &'static str {
        match self {
            Self::Payment(payment) => payment.payment_type.label(),
            Self::Entry(entry) => entry.entry_type.label(),
        }
    }

    /// Kaynak sıralaması (aynı tarih ve kimlik için deterministik sıra).
    fn source_rank(&self) -> u8 {
        match self {
            Self::Payment(_) => 0,
            Self::Entry(_) => 1,
        }
    }

    /// Kayıt iptal edilmiş mi?
    pub fn is_cancelled(&self) -> bool {
        match self {
            Self::Payment(payment) => payment.is_cancelled(),
            Self::Entry(entry) => entry.is_cancelled(),
        }
    }

    /// Kasa/banka hesabı adı.
    pub fn account_name(&self) -> &str {
        match self {
            Self::Payment(payment) => &payment.cash_account_name,
            Self::Entry(entry) => &entry.cash_account_name,
        }
    }

    /// Karşı taraf: tahsilat/ödemede cari, gelir/giderde kategori.
    pub fn counterparty(&self) -> &str {
        match self {
            Self::Payment(payment) => &payment.customer_name,
            Self::Entry(entry) => &entry.category_name,
        }
    }

    /// Açıklama.
    pub fn description(&self) -> Option<&str> {
        match self {
            Self::Payment(payment) => payment.description.as_deref(),
            Self::Entry(entry) => entry.description.as_deref(),
        }
    }
}

/// Kasa/banka ve tahsilat/ödeme bölümünün tüm durumu.
#[derive(Debug, Default)]
pub struct CashState {
    /// Aktif alt ekran.
    pub screen: CashScreen,
    /// Aktif sekme.
    pub tab: CashTab,
    /// Bilgilendirme mesajı.
    pub notice: Option<String>,
    /// Genel hata mesajı.
    pub general_error: Option<String>,

    // ----- Hesaplar -----
    /// Listelenen kasa/banka hesapları.
    pub account_list: Vec<CashAccount>,
    /// Liste okunurken oluşan hata.
    pub account_list_error: Option<String>,
    /// Hesap listesi ölçütleri.
    pub account_filter: CashAccountFilter,
    /// Düzenlenen hesabın kimliği (`None` = yeni kayıt).
    pub account_form_id: Option<i64>,
    /// Hesap formunun ham verisi.
    pub account_draft: CashAccountDraft,
    /// Hesap formu açıldığındaki başlangıç değerleri.
    pub account_original: CashAccountDraft,
    /// Hesap formunun alan hataları.
    pub account_errors: CashFieldErrors,
    /// Düzenlenen hesapta açılış bakiyesi düzenlenebilir mi?
    pub account_opening_editable: bool,
    /// Başka bir ekrandan gelindiğinde listede vurgulanacak hesap.
    pub highlight_account_id: Option<i64>,

    // ----- Hareketler -----
    /// Hareket listesi alt sekmesi (tür filtresi).
    pub movement_tab: MovementTab,
    /// Hareket listesi ölçütleri.
    pub movement_filter: PaymentFilter,
    /// Listelenen tahsilat/ödemeler.
    pub movement_list: Vec<Payment>,
    /// Listelenen gelir/gider hareketleri.
    pub movement_entries: Vec<IncomeExpenseEntry>,
    /// Gelir/gider listesi ölçütlerinde seçili kategori.
    pub movement_category_id: Option<i64>,
    /// Liste okunurken oluşan hata.
    pub movement_list_error: Option<String>,
    /// Tahsilat/ödeme alt toplamları (iptaller hariç).
    pub movement_summary: PaymentSummary,
    /// Gelir/gider alt toplamları (iptaller hariç).
    pub movement_entry_summary: EntrySummary,

    /// Ayrıntısı gösterilen kaydın kimliği.
    pub detail_id: Option<i64>,
    /// Ayrıntısı okunan kayıt.
    pub detail: Option<Payment>,
    /// Ayrıntıdaki dağıtımlar.
    pub detail_allocations: Vec<PaymentAllocation>,
    /// İptal onay penceresi açık mı?
    pub cancelling: bool,
    /// İptal nedeni girdisi.
    pub cancel_reason: String,
    /// İptal nedeni hatası.
    pub cancel_error: Option<String>,

    /// Tahsilat/ödeme formunun ham verisi.
    pub payment_draft: PaymentDraft,
    /// Tahsilat/ödeme formu açıldığındaki başlangıç değerleri.
    pub payment_original: PaymentDraft,
    /// Tahsilat/ödeme formunun alan hataları.
    pub payment_errors: PaymentFieldErrors,
    /// Kaydetme onayı bekleyen doğrulanmış kayıt.
    pub payment_pending: Option<ValidatedPayment>,
    /// Formun açıldığı yer.
    pub payment_origin: PaymentOrigin,

    /// Seçim listeleri için okunmuş cariler.
    pub customers: Vec<Customer>,
    /// Seçim listeleri için okunmuş kasa/banka hesapları (tümü).
    pub accounts: Vec<CashAccount>,
}

impl OnMuhasebeApp {
    /// Kasa/banka bölümünün durumunu okur.
    pub fn cash_state(&self) -> &CashState {
        &self.cash
    }

    /// Kasa/banka bölümünün durumunu değiştirilebilir olarak verir (form alanları).
    pub fn cash_state_mut(&mut self) -> &mut CashState {
        &mut self.cash
    }

    /// Kasa/banka hesap formunda kaydedilmemiş değişiklik var mı?
    pub fn cash_account_form_dirty(&self) -> bool {
        self.cash.screen == CashScreen::AccountForm
            && self.cash.account_draft != self.cash.account_original
    }

    /// Tahsilat/ödeme formunda kaydedilmemiş değişiklik var mı?
    pub fn payment_form_dirty(&self) -> bool {
        self.cash.screen == CashScreen::MovementForm
            && self.cash.payment_draft != self.cash.payment_original
    }

    /// Bölüme girildiğinde durumu hazırlar ve listeleri yeniler.
    pub fn enter_cash_section(&mut self) {
        self.cash.screen = CashScreen::AccountList;
        self.cash.tab = CashTab::Accounts;
        self.cash.account_errors = CashFieldErrors::default();
        self.cash.payment_errors = PaymentFieldErrors::default();
        self.cash.general_error = None;
        self.cash.payment_pending = None;
        self.cash.highlight_account_id = None;
        self.reload_cash_options();
        self.refresh_cash_account_list();
        self.refresh_payment_list();
    }

    /// Sekmeyi değiştirir (Hesaplar / Hareketler).
    pub fn set_cash_tab(&mut self, tab: CashTab) {
        if self.cash.tab == tab {
            return;
        }
        self.cash.tab = tab;
        self.cash.screen = match tab {
            CashTab::Accounts => CashScreen::AccountList,
            CashTab::Movements => CashScreen::MovementList,
        };
        self.cash.general_error = None;
        self.request_scroll_reset();
    }

    /// Bilgilendirme mesajını kapatır.
    pub fn dismiss_cash_notice(&mut self) {
        self.cash.notice = None;
    }

    // -----------------------------------------------------------------------
    // Hesaplar
    // -----------------------------------------------------------------------

    /// Hesap listesini geçerli ölçütlere göre yeniden okur.
    pub fn refresh_cash_account_list(&mut self) {
        self.cash.account_list_error = None;

        let filter = self.cash.account_filter.clone();
        let Some(database) = &self.database else {
            self.cash.account_list = Vec::new();
            self.cash.account_list_error =
                Some("Veritabanı bağlantısı kurulamadığı için hesaplar yüklenemedi.".to_owned());
            return;
        };

        match database.cash_account_list(&filter) {
            Ok(accounts) => self.cash.account_list = accounts,
            Err(error) => {
                self.cash.account_list = Vec::new();
                self.cash.account_list_error = Some(error.to_string());
            }
        }
    }

    /// Hesap listesi filtrelerini temizler ve listeyi yeniler.
    pub fn reset_cash_account_filter(&mut self) {
        self.cash.account_filter = CashAccountFilter::default();
        self.refresh_cash_account_list();
    }

    /// Yeni hesap formunu açar.
    pub fn open_cash_account_new_form(&mut self) {
        self.cash.account_form_id = None;
        self.cash.account_draft = CashAccountDraft::default();
        self.cash.account_original = self.cash.account_draft.clone();
        self.cash.account_errors = CashFieldErrors::default();
        self.cash.account_opening_editable = true;
        self.cash.general_error = None;
        self.cash.screen = CashScreen::AccountForm;
        self.request_scroll_reset();
        self.request_focus(FocusTarget::CashName);
    }

    /// Var olan hesabın düzenleme formunu açar.
    pub fn open_cash_account_edit_form(&mut self, id: i64) {
        let account = match &self.database {
            Some(database) => match database.cash_account_get(id) {
                Ok(Some(account)) => account,
                Ok(None) => {
                    self.cash.general_error = Some(format!("Hesap bulunamadı (kayıt no: {id})."));
                    self.refresh_cash_account_list();
                    return;
                }
                Err(error) => {
                    self.cash.general_error = Some(error.to_string());
                    return;
                }
            },
            None => {
                self.cash.general_error =
                    Some("Veritabanı bağlantısı yok; hesap okunamadı.".to_owned());
                return;
            }
        };

        self.cash.account_opening_editable = !account.has_movements();
        self.cash.account_draft = CashAccountDraft::from_account(&account);
        self.cash.account_original = self.cash.account_draft.clone();
        self.cash.account_form_id = Some(id);
        self.cash.account_errors = CashFieldErrors::default();
        self.cash.general_error = None;
        self.cash.screen = CashScreen::AccountForm;
        self.request_scroll_reset();
        self.request_focus(FocusTarget::CashName);
    }

    /// Hesap formunu kaydetmeden kapatır.
    pub fn cancel_cash_account_form(&mut self) {
        self.cash.account_errors = CashFieldErrors::default();
        self.cash.general_error = None;
        self.cash.screen = CashScreen::AccountList;
        self.cash.tab = CashTab::Accounts;
        self.request_scroll_reset();
        self.refresh_cash_account_list();
    }

    /// Hesap formunu doğrular ve kaydeder.
    pub fn submit_cash_account_form(&mut self) {
        if self.cash.account_form_id.is_some() && !self.cash_account_form_dirty() {
            self.cash.notice = Some("Kaydedilecek değişiklik yok.".to_owned());
            return;
        }

        let validated = match self.cash.account_draft.validate() {
            Ok(validated) => validated,
            Err(errors) => {
                self.cash.account_errors = errors;
                self.cash.general_error = None;
                self.request_focus(FocusTarget::CashName);
                return;
            }
        };

        self.cash.account_errors = CashFieldErrors::default();
        self.cash.general_error = None;

        let form_id = self.cash.account_form_id;
        let result = match &self.database {
            Some(database) => {
                if database
                    .cash_account_name_exists(&validated.name, form_id)
                    .unwrap_or(false)
                {
                    self.cash
                        .account_errors
                        .push(crate::cash::Field::Name, "Bu ad zaten kullanılıyor.");
                    self.request_focus(FocusTarget::CashName);
                    return;
                }
                match form_id {
                    Some(id) => database.cash_account_update(id, &validated).map(|()| id),
                    None => database.cash_account_insert(&validated),
                }
            }
            None => {
                self.cash.general_error =
                    Some("Veritabanı bağlantısı yok; hesap kaydedilemedi.".to_owned());
                return;
            }
        };

        match result {
            Ok(_) => {
                let creating = form_id.is_none();
                self.cash.notice = Some(if creating {
                    "Kasa/banka hesabı oluşturuldu.".to_owned()
                } else {
                    "Kasa/banka hesabı güncellendi.".to_owned()
                });
                self.cash.account_form_id = None;
                self.cash.screen = CashScreen::AccountList;
                self.cash.tab = CashTab::Accounts;
                self.request_scroll_reset();
                self.reload_cash_options();
                self.refresh_cash_account_list();
            }
            Err(AppError::CashAccountNameTaken(name)) => {
                self.cash.account_errors.push(
                    crate::cash::Field::Name,
                    format!("'{name}' adı zaten kullanılıyor."),
                );
                self.request_focus(FocusTarget::CashName);
            }
            Err(error) => {
                self.cash.general_error = Some(error.to_string());
            }
        }
    }

    /// Hesabı pasife alır veya yeniden aktife alır.
    pub fn toggle_cash_account_active(&mut self, id: i64) {
        let current = self
            .cash
            .account_list
            .iter()
            .find(|account| account.id == id)
            .map(|account| account.active);

        let Some(active) = current else {
            self.cash.general_error = Some(format!("Hesap bulunamadı (kayıt no: {id})."));
            return;
        };

        let result = match &self.database {
            Some(database) => database.cash_account_set_active(id, !active),
            None => {
                self.cash.general_error =
                    Some("Veritabanı bağlantısı yok; hesap güncellenemedi.".to_owned());
                return;
            }
        };

        match result {
            Ok(()) => {
                self.cash.notice = Some(if active {
                    "Hesap pasife alındı.".to_owned()
                } else {
                    "Hesap yeniden aktife alındı.".to_owned()
                });
                self.reload_cash_options();
                self.refresh_cash_account_list();
            }
            Err(error) => self.cash.general_error = Some(error.to_string()),
        }
    }

    /// Kasa/banka listesi başlığı için toplamlar (aktif hesaplar, veritabanından).
    pub fn cash_totals(&self) -> crate::cash::CashTotals {
        let Some(database) = &self.database else {
            return crate::cash::CashTotals::default();
        };
        crate::cash::CashTotals {
            cash: database
                .cash_accounts_total(AccountType::Kasa)
                .unwrap_or_default(),
            bank: database
                .cash_accounts_total(AccountType::Banka)
                .unwrap_or_default(),
        }
    }

    /// Genel Bakış için son tahsilat/ödemeler (iptaller dâhil).
    pub fn recent_payments(&self, limit: usize) -> Vec<Payment> {
        match &self.database {
            Some(database) => database.recent_payments(limit).unwrap_or_default(),
            None => Vec::new(),
        }
    }

    /// Toplam kasa/banka hesabı sayısı (pasif kayıtlar dâhil).
    pub fn cash_account_count(&self) -> usize {
        match &self.database {
            Some(database) => database.cash_account_count().unwrap_or(0),
            None => 0,
        }
    }

    // -----------------------------------------------------------------------
    // Hareketler (tahsilat/ödeme)
    // -----------------------------------------------------------------------

    /// Hareket alt sekmesini değiştirir ve listeyi yeniler.
    pub fn set_movement_tab(&mut self, tab: MovementTab) {
        if self.cash.movement_tab == tab {
            return;
        }
        self.cash.movement_tab = tab;
        if !matches!(tab, MovementTab::Incomes | MovementTab::Expenses) {
            self.cash.movement_category_id = None;
        }
        self.refresh_payment_list();
    }

    /// Hareket listesini geçerli ölçütlere göre yeniden okur.
    ///
    /// "Tümü" sekmesinde tahsilat/ödemeler ile gelir/giderler birlikte
    /// okunur; alt toplamlar ayrı ayrı tutulur. Gelir/gider sekmelerinde
    /// yalnızca ilgili kayıtlar sorgulanır.
    pub fn refresh_payment_list(&mut self) {
        self.cash.movement_list_error = None;
        self.cash.movement_list = Vec::new();
        self.cash.movement_summary = PaymentSummary::default();
        self.cash.movement_entries = Vec::new();
        self.cash.movement_entry_summary = EntrySummary::default();

        let Some(database) = &self.database else {
            self.cash.movement_list_error =
                Some("Veritabanı bağlantısı kurulamadığı için kayıtlar yüklenemedi.".to_owned());
            return;
        };

        let tab = self.cash.movement_tab;
        let mut error: Option<String> = None;

        if tab.shows_payments() {
            let mut filter = self.cash.movement_filter.clone();
            filter.payment_type = tab.payment_type();
            match database.payment_list(&filter) {
                Ok(payments) => {
                    self.cash.movement_summary = PaymentSummary::from_payments(payments.iter());
                    self.cash.movement_list = payments;
                }
                Err(payment_error) => error = Some(payment_error.to_string()),
            }
        }

        if tab.shows_entries() {
            let filter = EntryFilter {
                search: self.cash.movement_filter.search.clone(),
                entry_type: tab.entry_type(),
                category_id: if matches!(tab, MovementTab::Incomes | MovementTab::Expenses) {
                    self.cash.movement_category_id
                } else {
                    None
                },
                cash_account_id: self.cash.movement_filter.cash_account_id,
                status: self.cash.movement_filter.status.map(|status| match status {
                    crate::payment::PaymentStatus::Kesin => EntryStatus::Kesin,
                    crate::payment::PaymentStatus::Iptal => EntryStatus::Iptal,
                }),
                date_from: self.cash.movement_filter.date_from.clone(),
                date_to: self.cash.movement_filter.date_to.clone(),
            };
            match database.income_expense_list(&filter) {
                Ok(entries) => {
                    self.cash.movement_entry_summary = EntrySummary::from_entries(entries.iter());
                    self.cash.movement_entries = entries;
                }
                Err(entry_error) => {
                    if error.is_none() {
                        error = Some(entry_error.to_string());
                    }
                }
            }
        }

        self.cash.movement_list_error = error;
    }

    /// Hareket filtrelerini temizler (alt sekme "Tümü" olur) ve listeyi yeniler.
    pub fn reset_payment_filter(&mut self) {
        self.cash.movement_filter = PaymentFilter::default();
        self.cash.movement_category_id = None;
        self.cash.movement_tab = MovementTab::All;
        self.refresh_payment_list();
    }

    /// Gelir/gider hareket listesinde kategori filtresini değiştirir.
    pub fn set_movement_category_filter(&mut self, category_id: Option<i64>) {
        self.cash.movement_category_id = category_id;
        self.refresh_payment_list();
    }

    /// "Tümü" ve gelir/gider sekmeleri için birleşik hareket listesi.
    ///
    /// Kayıtlar tarihe göre **yeniden eskiye** sıralanır; aynı tarihte kimlik
    /// ve kaynak sırası kullanılır. Sonuç deterministiktir.
    pub fn cash_movement_list(&self) -> Vec<CashMovement> {
        let mut movements = Vec::new();
        if self.cash.movement_tab.shows_payments() {
            movements.extend(
                self.cash
                    .movement_list
                    .iter()
                    .cloned()
                    .map(CashMovement::Payment),
            );
        }
        if self.cash.movement_tab.shows_entries() {
            movements.extend(
                self.cash
                    .movement_entries
                    .iter()
                    .cloned()
                    .map(CashMovement::Entry),
            );
        }
        movements.sort_by(|left, right| {
            right
                .date()
                .cmp(left.date())
                .then_with(|| right.record_id().cmp(&left.record_id()))
                .then_with(|| right.source_rank().cmp(&left.source_rank()))
        });
        movements
    }

    /// Hareket listesine döner.
    pub fn open_payment_list(&mut self) {
        self.set_view(super::View::CashAndBank);
        self.cash.screen = CashScreen::MovementList;
        self.cash.tab = CashTab::Movements;
        self.cash.payment_errors = PaymentFieldErrors::default();
        self.cash.payment_pending = None;
        self.cash.general_error = None;
        self.request_scroll_reset();
        self.reload_cash_options();
        self.refresh_payment_list();
    }

    /// Yeni tahsilat/ödeme formunu açar (hareket listesinden).
    pub fn open_payment_form(&mut self, payment_type: PaymentType) {
        self.set_view(super::View::CashAndBank);
        self.reset_payment_draft(payment_type);
        self.cash.payment_origin = PaymentOrigin::Movements;
        self.cash.screen = CashScreen::MovementForm;
        self.cash.tab = CashTab::Movements;
        self.request_scroll_reset();
    }

    /// Belirli bir cari için tahsilat/ödeme formunu açar.
    pub fn open_payment_form_for_customer(&mut self, customer_id: i64, payment_type: PaymentType) {
        self.set_view(super::View::CashAndBank);
        self.reset_payment_draft(payment_type);
        self.cash.payment_draft.customer_id = Some(customer_id);
        self.cash.payment_origin = PaymentOrigin::Customer(customer_id);
        self.cash.screen = CashScreen::MovementForm;
        self.cash.tab = CashTab::Movements;
        self.rebuild_payment_allocations();
        self.request_scroll_reset();
        self.request_focus(FocusTarget::PaymentAmount);
    }

    /// Bir fatura için, kalan tutarı önceden dağıtılmış bir form açar.
    pub fn open_payment_form_for_invoice(&mut self, invoice_id: i64) {
        let invoice = match &self.database {
            Some(database) => match database.invoice_get(invoice_id) {
                Ok(Some(invoice)) => invoice,
                Ok(None) => {
                    self.cash.general_error =
                        Some(format!("Fatura bulunamadı (kayıt no: {invoice_id})."));
                    return;
                }
                Err(error) => {
                    self.cash.general_error = Some(error.to_string());
                    return;
                }
            },
            None => {
                self.cash.general_error =
                    Some("Veritabanı bağlantısı yok; fatura okunamadı.".to_owned());
                return;
            }
        };

        if invoice.is_cancelled() {
            self.cash.general_error =
                Some("İptal edilmiş faturaya tahsilat/ödeme bağlanamaz.".to_owned());
            return;
        }

        let payment_type = match invoice.invoice_type {
            InvoiceType::Sales => PaymentType::Tahsilat,
            InvoiceType::Purchase => PaymentType::Odeme,
        };

        self.set_view(super::View::CashAndBank);
        self.reset_payment_draft(payment_type);
        self.cash.payment_draft.customer_id = Some(invoice.customer_id);
        self.cash.payment_origin = PaymentOrigin::Invoice(invoice_id);
        self.cash.screen = CashScreen::MovementForm;
        self.cash.tab = CashTab::Movements;
        self.rebuild_payment_allocations();

        // Yalnızca bu faturayı kalan tutarıyla önceden doldur.
        let remaining = invoice.remaining().format_turkish();
        if let Some(allocation) = self
            .cash
            .payment_draft
            .allocations
            .iter_mut()
            .find(|allocation| allocation.invoice_id == invoice_id)
        {
            allocation.amount = remaining.clone();
        }
        self.cash.payment_draft.amount = remaining;
        self.request_scroll_reset();
        self.request_focus(FocusTarget::PaymentAmount);
    }

    /// Formu (yeni kayıt için) varsayılan değerlerle hazırlar ve seçim listelerini tazeler.
    fn reset_payment_draft(&mut self, payment_type: PaymentType) {
        self.cash.payment_errors = PaymentFieldErrors::default();
        self.cash.payment_pending = None;
        self.cash.general_error = None;

        // Seçim listeleri (cari + kasa/banka) önce tazelenir; varsayılan kasa
        // hesabı ancak bu listenin okunmasından sonra seçilebilir.
        self.reload_cash_options();

        let cash_account_id = self
            .cash
            .accounts
            .iter()
            .find(|account| account.active)
            .map(|account| account.id);

        self.cash.payment_draft = PaymentDraft {
            payment_type,
            cash_account_id,
            ..PaymentDraft::default()
        };
        self.cash.payment_original = self.cash.payment_draft.clone();
        self.request_focus(FocusTarget::PaymentCustomer);
    }

    /// Formun türünü değiştirir; cari uyarısı ve fatura listesi yenilenir.
    pub fn set_payment_draft_type(&mut self, payment_type: PaymentType) {
        if self.cash.payment_draft.payment_type == payment_type {
            return;
        }
        self.cash.payment_draft.payment_type = payment_type;
        self.rebuild_payment_allocations();
    }

    /// Formda cariyi seçer; fatura listesi yeniden kurulur.
    pub fn set_payment_customer(&mut self, customer_id: Option<i64>) {
        if self.cash.payment_draft.customer_id == customer_id {
            return;
        }
        self.cash.payment_draft.customer_id = customer_id;
        self.cash
            .payment_errors
            .clear(crate::payment::Field::Customer);
        self.rebuild_payment_allocations();
    }

    /// Formda kasa/banka hesabını seçer.
    pub fn set_payment_cash_account(&mut self, account_id: Option<i64>) {
        self.cash.payment_draft.cash_account_id = account_id;
        self.cash
            .payment_errors
            .clear(crate::payment::Field::CashAccount);
    }

    /// Tüm dağıtım satırlarını fatura kalanına eşitler.
    pub fn allocate_payment_all(&mut self) {
        self.cash.payment_draft.allocate_all();
    }

    /// Formdaki veriyi doğrular; başarılıysa kaydetme onayı için bekletir.
    pub fn submit_payment_form(&mut self) {
        match self.cash.payment_draft.validate() {
            Ok(validated) => {
                self.cash.payment_errors = PaymentFieldErrors::default();
                self.cash.general_error = None;
                self.cash.payment_pending = Some(validated);
            }
            Err(errors) => {
                let focus = if errors.get(crate::payment::Field::Customer).is_some()
                    || errors.get(crate::payment::Field::CashAccount).is_some()
                {
                    FocusTarget::PaymentCustomer
                } else {
                    FocusTarget::PaymentAmount
                };
                self.cash.payment_pending = None;
                self.cash.general_error = None;
                self.cash.payment_errors = errors;
                self.request_focus(focus);
            }
        }
    }

    /// Kaydetme onayından vazgeçer (form açık kalır, girdi korunur).
    pub fn discard_payment_confirmation(&mut self) {
        self.cash.payment_pending = None;
    }

    /// Onaylanan tahsilat/ödemeyi kesinleştirir (tek transaction).
    pub fn confirm_payment_save(&mut self) {
        let Some(validated) = self.cash.payment_pending.clone() else {
            return;
        };

        let result = match &self.database {
            Some(database) => database.payment_insert(&validated),
            None => {
                self.cash.general_error =
                    Some("Veritabanı bağlantısı yok; kayıt yapılamadı.".to_owned());
                return;
            }
        };

        let origin = self.cash.payment_origin;
        let invoice_type = validated.payment_type.invoice_type();

        match result {
            Ok(id) => {
                self.cash.payment_pending = None;
                self.refresh_payment_list();
                self.refresh_customer_list();
                self.refresh_cash_account_list();
                self.refresh_invoice_list();

                match origin {
                    PaymentOrigin::Invoice(invoice_id) => {
                        let view = match invoice_type {
                            InvoiceType::Sales => super::View::SalesInvoices,
                            InvoiceType::Purchase => super::View::PurchaseInvoices,
                        };
                        self.set_view(view);
                        self.open_invoice_detail(invoice_id);
                        self.invoice.notice = Some("Tahsilat/Ödeme kaydedildi.".to_owned());
                    }
                    PaymentOrigin::Movements | PaymentOrigin::Customer(_) => {
                        self.open_payment_detail(id);
                        self.cash.notice = Some("Tahsilat/Ödeme kaydedildi.".to_owned());
                    }
                }
            }
            Err(error) => {
                self.cash.payment_pending = None;
                self.cash.screen = CashScreen::MovementForm;
                self.cash.general_error = Some(error.to_string());
            }
        }
    }

    /// Tahsilat/ödeme ayrıntısını açar.
    pub fn open_payment_detail(&mut self, id: i64) {
        self.set_view(super::View::CashAndBank);
        let payment = match &self.database {
            Some(database) => match database.payment_get(id) {
                Ok(Some(payment)) => payment,
                Ok(None) => {
                    self.cash.general_error = Some(format!("Kayıt bulunamadı (kayıt no: {id})."));
                    self.open_payment_list();
                    return;
                }
                Err(error) => {
                    self.cash.general_error = Some(error.to_string());
                    self.open_payment_list();
                    return;
                }
            },
            None => {
                self.cash.general_error =
                    Some("Veritabanı bağlantısı yok; kayıt okunamadı.".to_owned());
                self.open_payment_list();
                return;
            }
        };

        let allocations = match &self.database {
            Some(database) => database.payment_allocations(id).unwrap_or_default(),
            None => Vec::new(),
        };

        self.cash.detail = Some(payment);
        self.cash.detail_allocations = allocations;
        self.cash.detail_id = Some(id);
        self.cash.screen = CashScreen::MovementDetail;
        self.cash.tab = CashTab::Movements;
        self.cash.cancelling = false;
        self.cash.cancel_reason.clear();
        self.cash.cancel_error = None;
        self.cash.payment_pending = None;
        self.cash.payment_errors = PaymentFieldErrors::default();
        self.cash.general_error = None;
        self.request_scroll_reset();
    }

    /// İptal onay penceresini açar.
    pub fn start_payment_cancel(&mut self) {
        self.cash.cancelling = true;
        self.cash.cancel_reason.clear();
        self.cash.cancel_error = None;
    }

    /// İptal onay penceresini kapatır.
    pub fn abort_payment_cancel(&mut self) {
        self.cash.cancelling = false;
        self.cash.cancel_reason.clear();
        self.cash.cancel_error = None;
    }

    /// İptal nedenini doğrular ve kaydı iptal eder.
    pub fn submit_payment_cancel(&mut self) {
        let reason = match validate_cancel_reason(&self.cash.cancel_reason) {
            Ok(reason) => reason,
            Err(message) => {
                self.cash.cancel_error = Some(message);
                return;
            }
        };

        let Some(id) = self.cash.detail_id else {
            self.cash.general_error = Some("Kayıt bilgisi bulunamadı.".to_owned());
            return;
        };

        let result = match &self.database {
            Some(database) => database.payment_cancel(id, &reason),
            None => {
                self.cash.general_error =
                    Some("Veritabanı bağlantısı yok; kayıt iptal edilemedi.".to_owned());
                return;
            }
        };

        match result {
            Ok(()) => {
                self.cash.cancelling = false;
                self.cash.cancel_reason.clear();
                self.cash.cancel_error = None;
                self.refresh_payment_list();
                self.refresh_customer_list();
                self.refresh_cash_account_list();
                self.refresh_invoice_list();
                self.open_payment_detail(id);
                self.cash.notice = Some(
                    "Tahsilat/ödeme iptal edildi. Kasa bakiyesi ve fatura ödemeleri geri alındı."
                        .to_owned(),
                );
            }
            Err(AppError::PaymentAlreadyCancelled(_)) => {
                self.cash.cancelling = false;
                self.cash.general_error = Some("Bu kayıt zaten iptal edilmiş.".to_owned());
                self.open_payment_detail(id);
            }
            Err(error) => {
                self.cash.cancel_error = Some(error.to_string());
            }
        }
    }

    /// Seçim listeleri için cari ve kasa/banka kayıtlarını yeniden okur.
    pub(crate) fn reload_cash_options(&mut self) {
        let Some(database) = &self.database else {
            self.cash.customers = Vec::new();
            self.cash.accounts = Vec::new();
            return;
        };

        match database.customer_list(&CustomerFilter::default()) {
            Ok(customers) => self.cash.customers = customers,
            Err(error) => self.cash.general_error = Some(error.to_string()),
        }
        match database.cash_account_list(&CashAccountFilter::default()) {
            Ok(accounts) => self.cash.accounts = accounts,
            Err(error) => self.cash.general_error = Some(error.to_string()),
        }
    }

    /// Seçili cariye ve türe göre dağıtılabilecek faturaları hazırlar.
    fn rebuild_payment_allocations(&mut self) {
        self.cash
            .payment_errors
            .clear(crate::payment::Field::Allocations);
        self.cash.payment_draft.allocations.clear();

        let (Some(customer_id), Some(database)) =
            (self.cash.payment_draft.customer_id, &self.database)
        else {
            return;
        };

        let invoice_type = self.cash.payment_draft.payment_type.invoice_type();
        let invoices = database
            .invoice_list(invoice_type, &InvoiceFilter::default())
            .unwrap_or_default();

        let allocations = invoices
            .into_iter()
            .filter(|invoice| {
                invoice.customer_id == customer_id
                    && !invoice.is_cancelled()
                    && invoice.remaining() > Money::ZERO
            })
            .map(|invoice| AllocationDraft {
                invoice_id: invoice.id,
                invoice_number: invoice.number,
                invoice_date: invoice.invoice_date,
                grand_total: invoice.totals.grand_total,
                paid_amount: invoice.paid_amount,
                amount: String::new(),
            })
            .collect();

        self.cash.payment_draft.replace_allocations(allocations);
    }

    /// Formdaki cari seçim listesi (tür uygun olanlar önce).
    pub fn payment_customer_options(&self) -> Vec<(i64, String)> {
        let payment_type = self.cash.payment_draft.payment_type;
        let mut suitable = Vec::new();
        let mut other = Vec::new();

        for customer in &self.cash.customers {
            let ok = payment_type.is_customer_kind_suitable(customer.kind);
            let label = if ok {
                format!("{} — {}", customer.code, customer.name)
            } else {
                format!("{} — {} · tür uygun değil", customer.code, customer.name)
            };
            if ok {
                suitable.push((customer.id, label));
            } else {
                other.push((customer.id, label));
            }
        }

        suitable.extend(other);
        suitable
    }

    /// Filtre için tüm cariler.
    pub fn movement_customer_options(&self) -> Vec<(i64, String)> {
        self.cash
            .customers
            .iter()
            .map(|customer| {
                (
                    customer.id,
                    format!("{} — {}", customer.code, customer.name),
                )
            })
            .collect()
    }

    /// Form için yalnızca **aktif** kasa/banka hesapları.
    pub fn payment_cash_account_options(&self) -> Vec<(i64, String)> {
        self.cash
            .accounts
            .iter()
            .filter(|account| account.active)
            .map(|account| (account.id, account.name.clone()))
            .collect()
    }

    /// Filtre için tüm kasa/banka hesapları.
    pub fn movement_cash_account_options(&self) -> Vec<(i64, String)> {
        self.cash
            .accounts
            .iter()
            .map(|account| (account.id, account.name.clone()))
            .collect()
    }

    /// Formda seçili cariyi döndürür (varsa).
    pub fn payment_selected_customer(&self) -> Option<&Customer> {
        let id = self.cash.payment_draft.customer_id?;
        self.cash
            .customers
            .iter()
            .find(|customer| customer.id == id)
    }

    /// Formda seçili kasa/banka hesabını döndürür (varsa).
    pub fn payment_selected_account(&self) -> Option<&CashAccount> {
        let id = self.cash.payment_draft.cash_account_id?;
        self.cash.accounts.iter().find(|account| account.id == id)
    }

    /// Cari etiketini kimliğine göre verir.
    pub fn cash_customer_label(&self, customer_id: i64) -> Option<String> {
        self.cash
            .customers
            .iter()
            .find(|customer| customer.id == customer_id)
            .map(|customer| format!("{} — {}", customer.code, customer.name))
    }

    /// Kasa/banka hesabı etiketini kimliğine göre verir.
    pub fn cash_account_label(&self, account_id: i64) -> Option<String> {
        self.cash
            .accounts
            .iter()
            .find(|account| account.id == account_id)
            .map(|account| account.name.clone())
    }

    /// Bir cariye bağlı tahsilat/ödemeleri türlerine göre ayırır.
    ///
    /// Dönüş: `(tahsilatlar, ödemeler)`. İptaller de dâhildir; arayüz bunları
    /// ayrıca işaretler.
    pub fn customer_payments(&self, customer_id: i64) -> (Vec<Payment>, Vec<Payment>) {
        let payments = match &self.database {
            Some(database) => database
                .payment_list_for_customer(customer_id)
                .unwrap_or_default(),
            None => Vec::new(),
        };

        let mut collections = Vec::new();
        let mut outgoings = Vec::new();
        for payment in payments {
            match payment.payment_type {
                PaymentType::Tahsilat => collections.push(payment),
                PaymentType::Odeme => outgoings.push(payment),
            }
        }
        (collections, outgoings)
    }

    /// Bir faturaya bağlı tahsilat/ödeme bağlantılarını okur.
    pub fn invoice_payments(&self, invoice_id: i64) -> Vec<crate::payment::InvoicePaymentLink> {
        match &self.database {
            Some(database) => database
                .payment_links_for_invoice(invoice_id)
                .unwrap_or_default(),
            None => Vec::new(),
        }
    }
}
