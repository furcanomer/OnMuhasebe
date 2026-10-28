// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Gelir/gider ekranlarının durumu ve komutları.
//!
//! Arayüz katmanı gelir/gider işlemleri için **hiçbir SQL çalıştırmaz**.
//! Ekranın gösterdiği tüm veri [`IncomeExpenseState`] içinde tutulur ve tüm
//! değişiklikler bu modüldeki komut metotlarıyla yapılır.
//!
//! ## Sekmeler
//!
//! "Gelir ve Gider" bölümü üç sekmeden oluşur: **Hareketler** (varsayılan),
//! **Kategoriler** ve **Özet**. Alt ekranlar [`IncomeExpenseScreen`] ile
//! ayrılır.
//!
//! ## Kaydetme akışı
//!
//! ```text
//! Form (EntryDraft) --validate--> ValidatedEntry --(onay)--> [tek transaction]
//!        ^                              |                        |
//!        | hata: FieldErrors (girdi korunur) |              hareket + kasa bakiyesi
//!        +------------------------------+
//! ```
//!
//! ## İptal akışı
//!
//! İptal, tek bir transaction içinde kasa/banka bakiyesine olan etkiyi geri
//! alır ve durumu `iptal` yapar. Kayıt silinmez.

use crate::cash::{CashAccount, CashAccountFilter};
use crate::error::AppError;
use crate::income_expense::{
    CategoryDraft, CategoryField, CategoryFieldErrors, CategoryFilter, EntryDraft, EntryField,
    EntryFieldErrors, EntryFilter, EntrySummary, EntryType, ExpenseCategory, IncomeExpenseEntry,
    PeriodSummary, ValidatedEntry, current_month_range, validate_cancel_reason,
};
use crate::money::Money;
use crate::payment::PaymentSummary;

use super::{CashScreen, CashTab, FocusTarget, MovementTab, OnMuhasebeApp, View};

/// "Gelir ve Gider" bölümünün sekmeleri.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IncomeExpenseTab {
    /// Hareket listesi (varsayılan).
    #[default]
    Entries,
    /// Kategori listesi.
    Categories,
    /// Dönem özeti.
    Summary,
}

impl IncomeExpenseTab {
    /// Sekme sırası.
    pub const ALL: [Self; 3] = [Self::Entries, Self::Categories, Self::Summary];

    /// Arayüzde gösterilen sekme adı.
    pub fn label(self) -> &'static str {
        match self {
            Self::Entries => "Hareketler",
            Self::Categories => "Kategoriler",
            Self::Summary => "Özet",
        }
    }
}

/// Gelir/gider bölümündeki alt ekranlar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IncomeExpenseScreen {
    /// Hareket listesi (varsayılan).
    #[default]
    EntryList,
    /// Yeni hareket formu.
    EntryForm,
    /// Hareket ayrıntısı.
    EntryDetail,
    /// Kategori listesi.
    CategoryList,
    /// Kategori formu (yeni/düzenle).
    CategoryForm,
}

/// Gelir/gider ayrıntısının nereden açıldığı (dönüş yeri).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EntryOrigin {
    /// Gelir/gider hareket listesinden.
    #[default]
    Entries,
    /// Kasa/banka hareketleri ("Tümü") listesinden.
    CashMovements,
}

/// Gelir/gider bölümünün tüm durumu.
#[derive(Debug)]
pub struct IncomeExpenseState {
    /// Aktif alt ekran.
    pub screen: IncomeExpenseScreen,
    /// Aktif sekme.
    pub tab: IncomeExpenseTab,
    /// Bilgilendirme mesajı.
    pub notice: Option<String>,
    /// Genel hata mesajı.
    pub general_error: Option<String>,

    // ----- Hareketler -----
    /// Hareket listesi ölçütleri.
    pub entry_filter: EntryFilter,
    /// Listelenen hareketler.
    pub entry_list: Vec<IncomeExpenseEntry>,
    /// Liste okunurken oluşan hata.
    pub entry_list_error: Option<String>,
    /// Liste alt toplamları (iptaller hariç).
    pub entry_summary: EntrySummary,
    /// Yeni hareket formunun ham verisi.
    pub entry_draft: EntryDraft,
    /// Hareket formu açıldığındaki başlangıç değerleri.
    pub entry_original: EntryDraft,
    /// Hareket formunun alan hataları.
    pub entry_errors: EntryFieldErrors,
    /// Kaydetme onayı bekleyen doğrulanmış kayıt.
    pub entry_pending: Option<ValidatedEntry>,
    /// Ayrıntısı gösterilen kaydın kimliği.
    pub entry_detail_id: Option<i64>,
    /// Ayrıntısı okunan kayıt.
    pub entry_detail: Option<IncomeExpenseEntry>,
    /// Ayrıntının açıldığı yer.
    pub entry_origin: EntryOrigin,
    /// İptal onay penceresi açık mı?
    pub cancelling: bool,
    /// İptal nedeni girdisi.
    pub cancel_reason: String,
    /// İptal nedeni hatası.
    pub cancel_error: Option<String>,

    // ----- Kategoriler -----
    /// Kategori listesi ölçütleri.
    pub category_filter: CategoryFilter,
    /// Listelenen kategoriler.
    pub category_list: Vec<ExpenseCategory>,
    /// Liste okunurken oluşan hata.
    pub category_list_error: Option<String>,
    /// Düzenlenen kategorinin kimliği (`None` = yeni kayıt).
    pub category_form_id: Option<i64>,
    /// Kategori formunun ham verisi.
    pub category_draft: CategoryDraft,
    /// Kategori formu açıldığındaki başlangıç değerleri.
    pub category_original: CategoryDraft,
    /// Kategori formunun alan hataları.
    pub category_errors: CategoryFieldErrors,

    // ----- Özet -----
    /// Özet dönemi başlangıcı (`YYYY-MM-DD`).
    pub summary_date_from: String,
    /// Özet dönemi bitişi (`YYYY-MM-DD`).
    pub summary_date_to: String,
    /// Hesaplanan dönem özeti.
    pub summary: PeriodSummary,
    /// Özet okunurken oluşan hata.
    pub summary_error: Option<String>,

    // ----- Seçim listeleri -----
    /// Tüm kategoriler (pasifler dahil).
    pub categories: Vec<ExpenseCategory>,
    /// Tüm kasa/banka hesapları (pasifler dahil).
    pub accounts: Vec<CashAccount>,
}

impl Default for IncomeExpenseState {
    fn default() -> Self {
        let (from, to) = current_month_range();
        Self {
            screen: IncomeExpenseScreen::default(),
            tab: IncomeExpenseTab::default(),
            notice: None,
            general_error: None,
            entry_filter: EntryFilter::default(),
            entry_list: Vec::new(),
            entry_list_error: None,
            entry_summary: EntrySummary::default(),
            entry_draft: EntryDraft::default(),
            entry_original: EntryDraft::default(),
            entry_errors: EntryFieldErrors::default(),
            entry_pending: None,
            entry_detail_id: None,
            entry_detail: None,
            entry_origin: EntryOrigin::default(),
            cancelling: false,
            cancel_reason: String::new(),
            cancel_error: None,
            category_filter: CategoryFilter::default(),
            category_list: Vec::new(),
            category_list_error: None,
            category_form_id: None,
            category_draft: CategoryDraft::default(),
            category_original: CategoryDraft::default(),
            category_errors: CategoryFieldErrors::default(),
            summary: PeriodSummary {
                date_from: from.clone(),
                date_to: to.clone(),
                ..PeriodSummary::default()
            },
            summary_date_from: from,
            summary_date_to: to,
            summary_error: None,
            categories: Vec::new(),
            accounts: Vec::new(),
        }
    }
}

impl OnMuhasebeApp {
    /// Gelir/gider bölümünün durumunu okur.
    pub fn income_expense_state(&self) -> &IncomeExpenseState {
        &self.income_expense
    }

    /// Gelir/gider bölümünün durumunu değiştirilebilir olarak verir.
    pub fn income_expense_state_mut(&mut self) -> &mut IncomeExpenseState {
        &mut self.income_expense
    }

    /// Gelir/gider hareket formunda kaydedilmemiş değişiklik var mı?
    pub fn income_entry_form_dirty(&self) -> bool {
        self.income_expense.screen == IncomeExpenseScreen::EntryForm
            && self.income_expense.entry_draft != self.income_expense.entry_original
    }

    /// Kategori formunda kaydedilmemiş değişiklik var mı?
    pub fn category_form_dirty(&self) -> bool {
        self.income_expense.screen == IncomeExpenseScreen::CategoryForm
            && self.income_expense.category_draft != self.income_expense.category_original
    }

    /// Bölüme girildiğinde durumu hazırlar ve listeleri yeniler.
    pub fn enter_income_expense_section(&mut self) {
        self.income_expense.screen = IncomeExpenseScreen::EntryList;
        self.income_expense.tab = IncomeExpenseTab::Entries;
        self.income_expense.entry_errors = EntryFieldErrors::default();
        self.income_expense.category_errors = CategoryFieldErrors::default();
        self.income_expense.entry_pending = None;
        self.income_expense.cancelling = false;
        self.income_expense.cancel_reason.clear();
        self.income_expense.cancel_error = None;
        self.income_expense.general_error = None;

        let (from, to) = current_month_range();
        self.income_expense.summary_date_from = from;
        self.income_expense.summary_date_to = to;

        self.reload_income_expense_options();
        self.refresh_income_expense_list();
        self.refresh_income_expense_category_list();
        self.refresh_income_expense_summary();
    }

    /// Sekmeyi değiştirir ve ilgili listeyi yeniler.
    pub fn set_income_expense_tab(&mut self, tab: IncomeExpenseTab) {
        if self.income_expense.tab == tab {
            return;
        }
        self.income_expense.tab = tab;
        self.income_expense.screen = match tab {
            IncomeExpenseTab::Entries => IncomeExpenseScreen::EntryList,
            IncomeExpenseTab::Categories => IncomeExpenseScreen::CategoryList,
            IncomeExpenseTab::Summary => IncomeExpenseScreen::EntryList,
        };
        self.income_expense.general_error = None;
        match tab {
            IncomeExpenseTab::Entries => self.refresh_income_expense_list(),
            IncomeExpenseTab::Categories => self.refresh_income_expense_category_list(),
            IncomeExpenseTab::Summary => self.refresh_income_expense_summary(),
        }
        self.request_scroll_reset();
    }

    /// Bilgilendirme mesajını kapatır.
    pub fn dismiss_income_expense_notice(&mut self) {
        self.income_expense.notice = None;
    }

    /// Seçim listeleri için kategori ve kasa/banka kayıtlarını yeniden okur.
    pub fn reload_income_expense_options(&mut self) {
        let Some(database) = &self.database else {
            self.income_expense.categories = Vec::new();
            self.income_expense.accounts = Vec::new();
            return;
        };

        match database.expense_category_list(&CategoryFilter::default()) {
            Ok(categories) => self.income_expense.categories = categories,
            Err(error) => self.income_expense.general_error = Some(error.to_string()),
        }
        match database.cash_account_list(&CashAccountFilter::default()) {
            Ok(accounts) => self.income_expense.accounts = accounts,
            Err(error) => self.income_expense.general_error = Some(error.to_string()),
        }
    }

    // -----------------------------------------------------------------------
    // Hareketler
    // -----------------------------------------------------------------------

    /// Hareket listesini geçerli ölçütlere göre yeniden okur.
    pub fn refresh_income_expense_list(&mut self) {
        self.income_expense.entry_list_error = None;
        let filter = self.income_expense.entry_filter.clone();

        let Some(database) = &self.database else {
            self.income_expense.entry_list = Vec::new();
            self.income_expense.entry_summary = EntrySummary::default();
            self.income_expense.entry_list_error =
                Some("Veritabanı bağlantısı kurulamadığı için kayıtlar yüklenemedi.".to_owned());
            return;
        };

        match database.income_expense_list(&filter) {
            Ok(entries) => {
                self.income_expense.entry_summary = EntrySummary::from_entries(entries.iter());
                self.income_expense.entry_list = entries;
            }
            Err(error) => {
                self.income_expense.entry_list = Vec::new();
                self.income_expense.entry_summary = EntrySummary::default();
                self.income_expense.entry_list_error = Some(error.to_string());
            }
        }
    }

    /// Hareket filtrelerini temizler ve listeyi yeniler.
    pub fn reset_income_expense_filter(&mut self) {
        self.income_expense.entry_filter = EntryFilter::default();
        self.refresh_income_expense_list();
    }

    /// Yeni gelir/gider formunu açar.
    pub fn open_income_expense_entry_form(&mut self, entry_type: EntryType) {
        self.set_view(View::IncomeExpense);
        self.income_expense.tab = IncomeExpenseTab::Entries;
        self.income_expense.screen = IncomeExpenseScreen::EntryForm;
        self.income_expense.entry_origin = EntryOrigin::Entries;
        self.reset_income_expense_entry_draft(entry_type);
        self.request_scroll_reset();
    }

    /// Formu belirli türde sıfırlar; ilk aktif kasa/banka hesabını seçer.
    fn reset_income_expense_entry_draft(&mut self, entry_type: EntryType) {
        self.reload_income_expense_options();
        let mut draft = EntryDraft::with_type(entry_type);
        draft.cash_account_id = self
            .income_expense
            .accounts
            .iter()
            .find(|account| account.active)
            .map(|account| account.id);
        self.income_expense.entry_draft = draft;
        self.income_expense.entry_original = self.income_expense.entry_draft.clone();
        self.income_expense.entry_errors = EntryFieldErrors::default();
        self.income_expense.entry_pending = None;
        self.income_expense.general_error = None;
        self.income_expense.cancelling = false;
        self.request_focus(FocusTarget::EntryCategory);
    }

    /// Formdaki hareket türünü değiştirir; türle uyuşmayan kategori seçimini temizler.
    pub fn set_income_expense_entry_type(&mut self, entry_type: EntryType) {
        if self.income_expense.entry_draft.entry_type == entry_type {
            return;
        }
        self.income_expense.entry_draft.entry_type = entry_type;
        let selected = self.income_expense.entry_draft.category_id;
        if let Some(category_id) = selected {
            let matches = self.income_expense.categories.iter().any(|category| {
                category.id == category_id
                    && category.active
                    && category.category_type == entry_type
            });
            if !matches {
                self.income_expense.entry_draft.category_id = None;
            }
        }
        self.income_expense.entry_errors.clear(EntryField::Category);
        self.income_expense.entry_errors.clear(EntryField::Type);
    }

    /// Kategori seçimini değiştirir.
    pub fn set_income_expense_category(&mut self, category_id: Option<i64>) {
        self.income_expense.entry_draft.category_id = category_id;
        self.income_expense.entry_errors.clear(EntryField::Category);
    }

    /// Kasa/banka hesabı seçimini değiştirir.
    pub fn set_income_expense_cash_account(&mut self, account_id: Option<i64>) {
        self.income_expense.entry_draft.cash_account_id = account_id;
        self.income_expense
            .entry_errors
            .clear(EntryField::CashAccount);
    }

    /// Aktif ve türü uyan kategori seçenekleri.
    pub fn income_expense_entry_category_options(&self) -> Vec<(i64, String)> {
        self.income_expense
            .categories
            .iter()
            .filter(|category| {
                category.active
                    && category.category_type == self.income_expense.entry_draft.entry_type
            })
            .map(|category| (category.id, category.name.clone()))
            .collect()
    }

    /// Filtre için tüm kategori seçenekleri (pasifler dahil).
    pub fn income_expense_filter_category_options(&self) -> Vec<(i64, String)> {
        self.income_expense
            .categories
            .iter()
            .map(|category| {
                (
                    category.id,
                    format!("{} · {}", category.name, category.category_type.label()),
                )
            })
            .collect()
    }

    /// Yeni hareket için aktif kasa/banka hesabı seçenekleri.
    pub fn income_expense_entry_account_options(&self) -> Vec<(i64, String)> {
        self.income_expense
            .accounts
            .iter()
            .filter(|account| account.active)
            .map(|account| (account.id, account.name.clone()))
            .collect()
    }

    /// Filtre için tüm kasa/banka hesabı seçenekleri (pasifler dahil).
    pub fn income_expense_filter_account_options(&self) -> Vec<(i64, String)> {
        self.income_expense
            .accounts
            .iter()
            .map(|account| {
                (
                    account.id,
                    format!("{} · {}", account.name, account.account_type.label()),
                )
            })
            .collect()
    }

    /// Kategori kimliğinden görünen adı bulur.
    pub fn income_expense_category_label(&self, category_id: i64) -> Option<String> {
        self.income_expense
            .categories
            .iter()
            .find(|category| category.id == category_id)
            .map(|category| category.name.clone())
    }

    /// Kasa/banka kimliğinden görünen adı bulur.
    pub fn income_expense_account_label(&self, account_id: i64) -> Option<String> {
        self.income_expense
            .accounts
            .iter()
            .find(|account| account.id == account_id)
            .map(|account| account.name.clone())
    }

    /// Formdaki tutar ve seçili hesaba göre tahmini yeni bakiyeyi verir.
    pub fn income_expense_preview_balance(&self) -> Option<Money> {
        let amount = self.income_expense.entry_draft.input_amount().ok()?;
        if amount.is_zero() {
            return None;
        }
        let account_id = self.income_expense.entry_draft.cash_account_id?;
        let account = self
            .income_expense
            .accounts
            .iter()
            .find(|account| account.id == account_id)?;
        let delta = amount
            .kurus()
            .saturating_mul(self.income_expense.entry_draft.entry_type.cash_sign());
        Some(Money::from_kurus(
            account.current_balance.kurus().saturating_add(delta),
        ))
    }

    /// Formu doğrular; başarılıysa onay bekleyen kaydı hazırlar.
    pub fn submit_income_expense_entry_form(&mut self) {
        match self.income_expense.entry_draft.validate() {
            Ok(entry) => {
                self.income_expense.entry_errors = EntryFieldErrors::default();
                self.income_expense.entry_pending = Some(entry);
                self.income_expense.general_error = None;
            }
            Err(errors) => {
                let focus = if errors.get(EntryField::Category).is_some() {
                    FocusTarget::EntryCategory
                } else {
                    FocusTarget::EntryAmount
                };
                self.income_expense.entry_errors = errors;
                self.income_expense.entry_pending = None;
                self.request_focus(focus);
            }
        }
    }

    /// Kaydetme onayından vazgeçer; form ve girdi korunur.
    pub fn discard_income_expense_entry_confirmation(&mut self) {
        self.income_expense.entry_pending = None;
    }

    /// Onaylanan gelir/gider kaydını tek transaction içinde kesinleştirir.
    pub fn confirm_income_expense_entry_save(&mut self) {
        let Some(entry) = self.income_expense.entry_pending.clone() else {
            self.income_expense.general_error = Some("Kaydedilecek kayıt bulunamadı.".to_owned());
            return;
        };

        let result = match &self.database {
            Some(database) => database.income_expense_insert(&entry),
            None => {
                self.income_expense.general_error =
                    Some("Veritabanı bağlantısı yok; kayıt yapılamadı.".to_owned());
                return;
            }
        };

        match result {
            Ok(id) => {
                self.income_expense.entry_pending = None;
                self.income_expense.entry_errors = EntryFieldErrors::default();
                self.income_expense.general_error = None;
                self.reload_income_expense_options();
                self.refresh_income_expense_list();
                self.refresh_income_expense_category_list();
                self.refresh_income_expense_summary();
                self.reload_cash_options();
                self.refresh_cash_account_list();
                self.refresh_payment_list();
                self.open_income_expense_detail_inner(id, EntryOrigin::Entries);
                self.income_expense.notice = Some("Gelir/Gider kaydedildi.".to_owned());
            }
            Err(error) => {
                self.income_expense.entry_pending = None;
                self.map_entry_save_error(error);
            }
        }
    }

    /// Veritabanı kaydetme hatasını ilgili alana yönlendirir.
    fn map_entry_save_error(&mut self, error: AppError) {
        match error {
            error @ AppError::ExpenseCategoryInactive(_) => {
                self.income_expense
                    .entry_errors
                    .push(EntryField::Category, error.to_string());
            }
            error @ AppError::ExpenseCategoryTypeMismatch(_) => {
                self.income_expense
                    .entry_errors
                    .push(EntryField::Category, error.to_string());
            }
            error @ AppError::ExpenseCategoryNotFound(_) => {
                self.income_expense
                    .entry_errors
                    .push(EntryField::Category, error.to_string());
            }
            error @ AppError::CashAccountInactive(_) => {
                self.income_expense
                    .entry_errors
                    .push(EntryField::CashAccount, error.to_string());
            }
            error @ AppError::CashAccountNotFound(_) => {
                self.income_expense
                    .entry_errors
                    .push(EntryField::CashAccount, error.to_string());
            }
            error => self.income_expense.general_error = Some(error.to_string()),
        }
    }

    /// Gelir/gider ayrıntısını açar (modül listesinden).
    pub fn open_income_expense_detail(&mut self, id: i64) {
        self.set_view(View::IncomeExpense);
        self.open_income_expense_detail_inner(id, EntryOrigin::Entries);
    }

    /// Kasa/banka hareket listesinden gelir/gider ayrıntısını açar.
    pub fn open_income_expense_detail_from_cash(&mut self, id: i64) {
        self.set_view(View::IncomeExpense);
        self.open_income_expense_detail_inner(id, EntryOrigin::CashMovements);
    }

    fn open_income_expense_detail_inner(&mut self, id: i64, origin: EntryOrigin) {
        let entry = match &self.database {
            Some(database) => match database.income_expense_get(id) {
                Ok(Some(entry)) => entry,
                Ok(None) => {
                    self.income_expense.general_error =
                        Some(format!("Kayıt bulunamadı (kayıt no: {id})."));
                    self.back_from_income_expense_detail_to_origin(origin);
                    return;
                }
                Err(error) => {
                    self.income_expense.general_error = Some(error.to_string());
                    self.back_from_income_expense_detail_to_origin(origin);
                    return;
                }
            },
            None => {
                self.income_expense.general_error =
                    Some("Veritabanı bağlantısı yok; kayıt okunamadı.".to_owned());
                return;
            }
        };

        self.income_expense.entry_detail = Some(entry);
        self.income_expense.entry_detail_id = Some(id);
        self.income_expense.entry_origin = origin;
        self.income_expense.screen = IncomeExpenseScreen::EntryDetail;
        self.income_expense.tab = IncomeExpenseTab::Entries;
        self.income_expense.cancelling = false;
        self.income_expense.cancel_reason.clear();
        self.income_expense.cancel_error = None;
        self.income_expense.entry_pending = None;
        self.income_expense.entry_errors = EntryFieldErrors::default();
        self.income_expense.general_error = None;
        self.request_scroll_reset();
    }

    /// Ayrıntıdan kaydın açıldığı yere döner.
    pub fn back_from_income_expense_detail(&mut self) {
        self.back_from_income_expense_detail_to_origin(self.income_expense.entry_origin);
    }

    fn back_from_income_expense_detail_to_origin(&mut self, origin: EntryOrigin) {
        match origin {
            EntryOrigin::Entries => self.open_income_expense_list(),
            EntryOrigin::CashMovements => {
                self.set_view(View::CashAndBank);
                self.cash.screen = CashScreen::MovementList;
                self.cash.tab = CashTab::Movements;
                self.cash.movement_tab = MovementTab::All;
                self.cash.general_error = None;
                self.request_scroll_reset();
                self.reload_cash_options();
                self.refresh_payment_list();
            }
        }
    }

    /// Gelir/gider hareket listesine döner.
    pub fn open_income_expense_list(&mut self) {
        self.set_view(View::IncomeExpense);
        self.income_expense.screen = IncomeExpenseScreen::EntryList;
        self.income_expense.tab = IncomeExpenseTab::Entries;
        self.income_expense.entry_errors = EntryFieldErrors::default();
        self.income_expense.entry_pending = None;
        self.income_expense.general_error = None;
        self.request_scroll_reset();
        self.refresh_income_expense_list();
    }

    /// İptal onay penceresini açar.
    pub fn start_income_expense_cancel(&mut self) {
        self.income_expense.cancelling = true;
        self.income_expense.cancel_reason.clear();
        self.income_expense.cancel_error = None;
    }

    /// İptal onay penceresini kapatır.
    pub fn abort_income_expense_cancel(&mut self) {
        self.income_expense.cancelling = false;
        self.income_expense.cancel_reason.clear();
        self.income_expense.cancel_error = None;
    }

    /// İptal nedenini doğrular ve kaydı iptal eder.
    pub fn submit_income_expense_cancel(&mut self) {
        let reason = match validate_cancel_reason(&self.income_expense.cancel_reason) {
            Ok(reason) => reason,
            Err(message) => {
                self.income_expense.cancel_error = Some(message);
                return;
            }
        };

        let Some(id) = self.income_expense.entry_detail_id else {
            self.income_expense.general_error = Some("Kayıt bilgisi bulunamadı.".to_owned());
            return;
        };
        let origin = self.income_expense.entry_origin;

        let result = match &self.database {
            Some(database) => database.income_expense_cancel(id, &reason),
            None => {
                self.income_expense.general_error =
                    Some("Veritabanı bağlantısı yok; kayıt iptal edilemedi.".to_owned());
                return;
            }
        };

        match result {
            Ok(()) => {
                self.income_expense.cancelling = false;
                self.income_expense.cancel_reason.clear();
                self.income_expense.cancel_error = None;
                self.reload_income_expense_options();
                self.refresh_income_expense_list();
                self.refresh_income_expense_category_list();
                self.refresh_income_expense_summary();
                self.reload_cash_options();
                self.refresh_cash_account_list();
                self.refresh_payment_list();
                self.open_income_expense_detail_inner(id, origin);
                self.income_expense.notice = Some(
                    "Gelir/gider kaydı iptal edildi. Kasa/banka bakiyesi geri alındı.".to_owned(),
                );
            }
            Err(AppError::IncomeExpenseAlreadyCancelled(_)) => {
                self.income_expense.cancelling = false;
                self.income_expense.general_error =
                    Some("Bu gelir/gider kaydı zaten iptal edilmiş.".to_owned());
                self.open_income_expense_detail_inner(id, origin);
            }
            Err(error) => {
                self.income_expense.cancel_error = Some(error.to_string());
            }
        }
    }

    // -----------------------------------------------------------------------
    // Kategoriler
    // -----------------------------------------------------------------------

    /// Kategori listesini geçerli ölçütlere göre yeniden okur.
    pub fn refresh_income_expense_category_list(&mut self) {
        self.income_expense.category_list_error = None;
        let filter = self.income_expense.category_filter.clone();

        let Some(database) = &self.database else {
            self.income_expense.category_list = Vec::new();
            self.income_expense.category_list_error =
                Some("Veritabanı bağlantısı kurulamadığı için kategoriler yüklenemedi.".to_owned());
            return;
        };

        match database.expense_category_list(&filter) {
            Ok(categories) => self.income_expense.category_list = categories,
            Err(error) => {
                self.income_expense.category_list = Vec::new();
                self.income_expense.category_list_error = Some(error.to_string());
            }
        }
    }

    /// Kategori filtrelerini temizler ve listeyi yeniler.
    pub fn reset_income_expense_category_filter(&mut self) {
        self.income_expense.category_filter = CategoryFilter::default();
        self.refresh_income_expense_category_list();
    }

    /// Yeni kategori formunu açar.
    pub fn open_income_expense_category_new_form(&mut self) {
        self.set_view(View::IncomeExpense);
        self.income_expense.tab = IncomeExpenseTab::Categories;
        self.income_expense.screen = IncomeExpenseScreen::CategoryForm;
        self.income_expense.category_form_id = None;
        self.income_expense.category_draft = CategoryDraft::default();
        self.income_expense.category_original = self.income_expense.category_draft.clone();
        self.income_expense.category_errors = CategoryFieldErrors::default();
        self.income_expense.general_error = None;
        self.request_scroll_reset();
        self.request_focus(FocusTarget::CategoryName);
    }

    /// Var olan kategoriyi düzenlemek için formu açar.
    pub fn open_income_expense_category_edit_form(&mut self, id: i64) {
        let category = match &self.database {
            Some(database) => match database.expense_category_get(id) {
                Ok(Some(category)) => category,
                Ok(None) => {
                    self.income_expense.general_error =
                        Some(format!("Kategori bulunamadı (kayıt no: {id})."));
                    return;
                }
                Err(error) => {
                    self.income_expense.general_error = Some(error.to_string());
                    return;
                }
            },
            None => {
                self.income_expense.general_error =
                    Some("Veritabanı bağlantısı yok; kategori okunamadı.".to_owned());
                return;
            }
        };

        self.set_view(View::IncomeExpense);
        self.income_expense.tab = IncomeExpenseTab::Categories;
        self.income_expense.screen = IncomeExpenseScreen::CategoryForm;
        self.income_expense.category_form_id = Some(id);
        self.income_expense.category_draft = CategoryDraft::from_category(&category);
        self.income_expense.category_original = self.income_expense.category_draft.clone();
        self.income_expense.category_errors = CategoryFieldErrors::default();
        self.income_expense.general_error = None;
        self.request_scroll_reset();
        self.request_focus(FocusTarget::CategoryName);
    }

    /// Kategori formundan listeye döner.
    pub fn cancel_income_expense_category_form(&mut self) {
        self.income_expense.screen = IncomeExpenseScreen::CategoryList;
        self.income_expense.tab = IncomeExpenseTab::Categories;
        self.income_expense.category_form_id = None;
        self.income_expense.category_errors = CategoryFieldErrors::default();
        self.income_expense.general_error = None;
        self.refresh_income_expense_category_list();
        self.request_scroll_reset();
    }

    /// Yeni kategoride türü değiştirir (düzenlemede tür sabittir).
    pub fn set_income_expense_category_form_type(&mut self, category_type: EntryType) {
        if self.income_expense.category_form_id.is_some() {
            return;
        }
        self.income_expense.category_draft.category_type = category_type;
        self.income_expense
            .category_errors
            .clear(CategoryField::Name);
    }

    /// Kategori formunu doğrular ve kaydeder.
    pub fn submit_income_expense_category_form(&mut self) {
        if self.income_expense.category_form_id.is_some() && !self.category_form_dirty() {
            self.income_expense.notice = Some("Kaydedilecek değişiklik yok.".to_owned());
            return;
        }

        let validated = match self.income_expense.category_draft.validate() {
            Ok(validated) => validated,
            Err(errors) => {
                self.income_expense.category_errors = errors;
                self.request_focus(FocusTarget::CategoryName);
                return;
            }
        };

        let exclude_id = self.income_expense.category_form_id;
        if let Some(database) = &self.database {
            match database.expense_category_name_exists(
                &validated.name,
                validated.category_type,
                exclude_id,
            ) {
                Ok(true) => {
                    self.income_expense.category_errors.push(
                        CategoryField::Name,
                        format!(
                            "'{}' adlı kategori bu türde zaten kullanılıyor. Lütfen başka bir ad girin.",
                            validated.name
                        ),
                    );
                    self.request_focus(FocusTarget::CategoryName);
                    return;
                }
                Ok(false) => {}
                Err(error) => {
                    self.income_expense.general_error = Some(error.to_string());
                    return;
                }
            }
        } else {
            self.income_expense.general_error =
                Some("Veritabanı bağlantısı yok; kategori kaydedilemedi.".to_owned());
            return;
        }

        let result = match (&self.database, exclude_id) {
            (Some(database), Some(id)) => database
                .expense_category_update(id, &validated)
                .map(|()| id),
            (Some(database), None) => database.expense_category_insert(&validated),
            (None, _) => unreachable!("veritabanı yukarıda kontrol edildi"),
        };

        match result {
            Ok(_) => {
                self.income_expense.category_errors = CategoryFieldErrors::default();
                self.income_expense.category_form_id = None;
                self.income_expense.screen = IncomeExpenseScreen::CategoryList;
                self.income_expense.general_error = None;
                self.reload_income_expense_options();
                self.refresh_income_expense_category_list();
                self.request_scroll_reset();
                self.income_expense.notice = Some("Kategori kaydedildi.".to_owned());
            }
            Err(AppError::ExpenseCategoryNameTaken(message)) => {
                self.income_expense.category_errors.push(
                    CategoryField::Name,
                    format!(
                        "{} Lütfen başka bir ad girin.",
                        message.trim_end_matches(" Lütfen başka bir ad girin.")
                    ),
                );
                self.request_focus(FocusTarget::CategoryName);
            }
            Err(error) => self.income_expense.general_error = Some(error.to_string()),
        }
    }

    /// Kategoriyi pasife alır veya yeniden aktife alır.
    pub fn toggle_income_expense_category_active(&mut self, id: i64) {
        let Some(category) = self
            .income_expense
            .categories
            .iter()
            .find(|category| category.id == id)
            .cloned()
        else {
            self.income_expense.general_error =
                Some(format!("Kategori bulunamadı (kayıt no: {id})."));
            return;
        };
        let new_active = !category.active;

        let result = match &self.database {
            Some(database) => database.expense_category_set_active(id, new_active),
            None => {
                self.income_expense.general_error =
                    Some("Veritabanı bağlantısı yok; kategori güncellenemedi.".to_owned());
                return;
            }
        };

        match result {
            Ok(()) => {
                self.reload_income_expense_options();
                self.refresh_income_expense_category_list();
                self.income_expense.notice = Some(if new_active {
                    "Kategori yeniden aktife alındı.".to_owned()
                } else {
                    "Kategori pasife alındı.".to_owned()
                });
            }
            Err(error) => self.income_expense.general_error = Some(error.to_string()),
        }
    }

    // -----------------------------------------------------------------------
    // Özet
    // -----------------------------------------------------------------------

    /// Özeti geçerli tarih aralığı için yeniden hesaplar.
    pub fn refresh_income_expense_summary(&mut self) {
        self.income_expense.summary_error = None;
        let from = self.income_expense.summary_date_from.trim().to_owned();
        let to = self.income_expense.summary_date_to.trim().to_owned();

        if (!from.is_empty() && chrono::NaiveDate::parse_from_str(&from, "%Y-%m-%d").is_err())
            || (!to.is_empty() && chrono::NaiveDate::parse_from_str(&to, "%Y-%m-%d").is_err())
        {
            self.income_expense.summary = PeriodSummary::default();
            self.income_expense.summary_error =
                Some("Tarih aralığı geçerli değil (GG.AA.YYYY biçiminde seçin).".to_owned());
            return;
        }
        if !from.is_empty() && !to.is_empty() && from > to {
            self.income_expense.summary = PeriodSummary::default();
            self.income_expense.summary_error =
                Some("Başlangıç tarihi bitiş tarihinden sonra olamaz.".to_owned());
            return;
        }

        let filter = EntryFilter {
            date_from: from.clone(),
            date_to: to.clone(),
            ..EntryFilter::default()
        };
        let Some(database) = &self.database else {
            self.income_expense.summary = PeriodSummary::default();
            self.income_expense.summary_error =
                Some("Veritabanı bağlantısı kurulamadığı için özet hesaplanamadı.".to_owned());
            return;
        };

        match database.income_expense_list(&filter) {
            Ok(entries) => {
                self.income_expense.summary =
                    PeriodSummary::from_entries(&from, &to, entries.iter());
            }
            Err(error) => {
                self.income_expense.summary = PeriodSummary::default();
                self.income_expense.summary_error = Some(error.to_string());
            }
        }
    }

    /// Özet dönemini içinde bulunulan aya ayarlar.
    pub fn set_income_expense_summary_this_month(&mut self) {
        let (from, to) = current_month_range();
        self.income_expense.summary_date_from = from;
        self.income_expense.summary_date_to = to;
        self.refresh_income_expense_summary();
    }

    /// Genel bakış için bu ayın gelir/gider toplamları ve kayıt varlığı.
    ///
    /// Dönemde hiç kayıt yoksa ikinci değer `false` olur; böylece genel bakış
    /// uydurma sıfırlar yerine açık bir boş durum mesajı gösterebilir.
    pub fn month_income_expense(&self) -> (EntrySummary, bool) {
        let (from, to) = current_month_range();
        let Some(database) = &self.database else {
            return (EntrySummary::default(), false);
        };
        let summary = database
            .income_expense_summary(&from, &to)
            .unwrap_or_default();
        let has_entries = database
            .income_expense_has_entries(&from, &to)
            .unwrap_or(false);
        (summary, has_entries)
    }

    /// Kasa/banka "Tümü" listesi için birleşik nakit etkisi.
    pub fn cash_movement_net(&self) -> Money {
        let payments: PaymentSummary = self.cash.movement_summary;
        let entries = self.cash.movement_entry_summary;
        Money::from_kurus(payments.net().kurus().saturating_add(entries.net().kurus()))
    }
}
