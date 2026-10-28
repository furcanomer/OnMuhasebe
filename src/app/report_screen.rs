// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Rapor ekranlarının durumu ve komutları.
//!
//! Arayüz katmanı raporlar için **hiçbir SQL çalıştırmaz**. Tüm rapor verisi
//! bu modüldeki yenileme metotlarıyla [`Database`] üzerinden okunur ve
//! [`ReportsState`] içinde tutulur. Raporlar salt okunurdur; bu modül hiçbir
//! kaydı değiştirmez.
//!
//! ## Filtreler
//!
//! Her raporun kendi filtreleri vardır. "Sıfırla" aktif raporu varsayılan
//! durumuna döndürür: cari ekstre/bakiye için boş tarih aralığı, dönem
//! raporları için içinde bulunulan ay, stok için yalnızca aktif ürünler.

use crate::cash::{CashAccount, CashAccountFilter};
use crate::customer::{Customer, CustomerFilter, CustomerType};
use crate::income_expense::current_month_range;
use crate::report::{
    CashMovementReport, CustomerBalanceReport, CustomerBalanceReportFilter,
    CustomerBalanceSortColumn, CustomerStatement, IncomeExpenseReport, ReportKind,
    SalesPurchaseGrouping, SalesPurchaseReport, StockReport, StockReportFilter, StockStatus,
    validate_date_range,
};

use super::OnMuhasebeApp;

/// Rapor ekranının tüm filtrelere ve yüklenmiş raporlara ilişkin durumu.
#[derive(Debug, Clone)]
pub struct ReportsState {
    /// Aktif rapor sekmesi.
    pub tab: ReportKind,
    /// Başlangıç tarihi (`YYYY-MM-DD`, boş = sınır yok).
    pub date_from: String,
    /// Bitiş tarihi (`YYYY-MM-DD`, boş = sınır yok).
    pub date_to: String,
    /// Seçili cari (cari ekstre).
    pub customer_id: Option<i64>,
    /// Açılır seçim için tüm cariler.
    pub customers: Vec<Customer>,
    /// Açılır seçim için tüm kasa/banka hesapları.
    pub accounts: Vec<CashAccount>,
    /// Cari ekstre raporu.
    pub customer_statement: Option<CustomerStatement>,
    /// Cari bakiye özeti filtreleri.
    pub customer_balance_filter: CustomerBalanceReportFilter,
    /// Cari bakiye sıralama kolonu.
    pub customer_sort_column: CustomerBalanceSortColumn,
    /// Cari bakiye sıralaması artan mı?
    pub customer_sort_ascending: bool,
    /// Cari bakiye özeti raporu.
    pub customer_balance_report: CustomerBalanceReport,
    /// Stok durumu filtreleri.
    pub stock_filter: StockReportFilter,
    /// Stok durumu raporu.
    pub stock_report: StockReport,
    /// Satış/alış gruplaması.
    pub sales_grouping: SalesPurchaseGrouping,
    /// Satış/alış raporu.
    pub sales_report: SalesPurchaseReport,
    /// Seçili kasa/banka hesabı (`None` = tümü).
    pub cash_account_id: Option<i64>,
    /// Kasa/banka hareket raporu.
    pub cash_report: Option<CashMovementReport>,
    /// Gelir/gider raporu.
    pub income_expense_report: Option<IncomeExpenseReport>,
    /// Son işlem hatası (varsa).
    pub error: Option<String>,
}

impl Default for ReportsState {
    fn default() -> Self {
        Self {
            tab: ReportKind::CustomerStatement,
            date_from: String::new(),
            date_to: String::new(),
            customer_id: None,
            customers: Vec::new(),
            accounts: Vec::new(),
            customer_statement: None,
            customer_balance_filter: CustomerBalanceReportFilter::default(),
            customer_sort_column: CustomerBalanceSortColumn::Balance,
            customer_sort_ascending: false,
            customer_balance_report: CustomerBalanceReport::default(),
            stock_filter: StockReportFilter::default(),
            stock_report: StockReport::default(),
            sales_grouping: SalesPurchaseGrouping::default(),
            sales_report: SalesPurchaseReport::default(),
            cash_account_id: None,
            cash_report: None,
            income_expense_report: None,
            error: None,
        }
    }
}

impl ReportsState {
    /// Cari seçim kutusu seçenekleri.
    pub fn customer_options(&self) -> Vec<(i64, String)> {
        self.customers
            .iter()
            .map(|customer| {
                (
                    customer.id,
                    format!("{} · {}", customer.code, customer.name),
                )
            })
            .collect()
    }

    /// Kasa/banka seçim kutusu seçenekleri.
    pub fn account_options(&self) -> Vec<(i64, String)> {
        self.accounts
            .iter()
            .map(|account| {
                (
                    account.id,
                    format!("{} · {}", account.name, account.account_type.label()),
                )
            })
            .collect()
    }

    /// Seçili carinin gösterim metni.
    pub fn selected_customer_text(&self) -> String {
        self.customer_id
            .and_then(|id| self.customers.iter().find(|customer| customer.id == id))
            .map(|customer| format!("{} · {}", customer.code, customer.name))
            .unwrap_or_else(|| "Cari seçin".to_owned())
    }

    /// Seçili kasa/banka hesabının gösterim metni.
    pub fn selected_account_text(&self) -> String {
        self.cash_account_id
            .and_then(|id| self.accounts.iter().find(|account| account.id == id))
            .map(|account| format!("{} · {}", account.name, account.account_type.label()))
            .unwrap_or_else(|| "Tüm hesaplar".to_owned())
    }
}

impl OnMuhasebeApp {
    /// Rapor durumuna salt okunur erişim.
    pub fn reports_state(&self) -> &ReportsState {
        &self.reports
    }

    /// Rapor durumuna yazma erişimi (yalnızca filtre alanları için).
    pub fn reports_state_mut(&mut self) -> &mut ReportsState {
        &mut self.reports
    }

    /// Rapor bölümüne girer: seçenekleri ve varsayılan filtreleri yeniler.
    ///
    /// Raporlar salt okunur olduğu için bölüme her girişte veriler yeniden
    /// okunur. Aktif sekme cari ekstreye döner.
    pub fn enter_reports_section(&mut self) {
        self.reports = ReportsState::default();
        self.load_report_options();
        self.reset_reports_filters();
    }

    /// Rapor filtrelerinde kullanılan cari ve kasa listelerini yeniler.
    pub fn load_report_options(&mut self) {
        self.reports.customers.clear();
        self.reports.accounts.clear();

        let Some(database) = &self.database else {
            self.reports.error = Some("Veritabanı açık değil; raporlar okunamıyor.".to_owned());
            return;
        };

        match database.customer_list(&CustomerFilter::default()) {
            Ok(customers) => self.reports.customers = customers,
            Err(error) => {
                self.reports.error = Some(error.to_string());
                return;
            }
        }
        match database.cash_account_list(&CashAccountFilter::default()) {
            Ok(accounts) => self.reports.accounts = accounts,
            Err(error) => self.reports.error = Some(error.to_string()),
        }
    }

    /// Aktif rapor sekmesini değiştirir ve varsayılan filtrelerle yeniler.
    pub fn set_report_tab(&mut self, tab: ReportKind) {
        if self.reports.tab == tab {
            return;
        }
        self.reports.tab = tab;
        self.load_report_options();
        self.reset_reports_filters();
    }

    /// Aktif raporun filtrelerini varsayılana döndürür ve raporu yeniler.
    pub fn reset_reports_filters(&mut self) {
        self.reports.error = None;
        self.reports.customer_id = None;
        self.reports.customer_balance_filter = CustomerBalanceReportFilter::default();
        self.reports.customer_sort_column = CustomerBalanceSortColumn::Balance;
        self.reports.customer_sort_ascending = false;
        self.reports.stock_filter = StockReportFilter::default();
        self.reports.sales_grouping = SalesPurchaseGrouping::default();
        self.reports.cash_account_id = None;

        self.reports.customer_statement = None;
        self.reports.customer_balance_report = CustomerBalanceReport::default();
        self.reports.stock_report = StockReport::default();
        self.reports.sales_report = SalesPurchaseReport::default();
        self.reports.cash_report = None;
        self.reports.income_expense_report = None;

        if self.reports.tab.defaults_to_current_month() {
            let (from, to) = current_month_range();
            self.reports.date_from = from;
            self.reports.date_to = to;
        } else {
            self.reports.date_from.clear();
            self.reports.date_to.clear();
        }

        self.refresh_current_report();
    }

    /// "Uygula" düğmesi: tarihleri doğrular ve aktif raporu yeniler.
    pub fn apply_report_filters(&mut self) {
        self.refresh_current_report();
    }

    /// Cari ekstre için cari seçer ve raporu yeniler.
    pub fn set_report_customer(&mut self, customer_id: Option<i64>) {
        self.reports.customer_id = customer_id;
        self.refresh_customer_statement();
    }

    /// Satış/alış gruplamasını değiştirir ve raporu yeniler.
    pub fn set_sales_grouping(&mut self, grouping: SalesPurchaseGrouping) {
        if self.reports.sales_grouping == grouping {
            return;
        }
        self.reports.sales_grouping = grouping;
        self.refresh_sales_report();
    }

    /// Kasa/banka hesabı filtresini değiştirir ve raporu yeniler.
    pub fn set_cash_report_account(&mut self, account_id: Option<i64>) {
        self.reports.cash_account_id = account_id;
        self.refresh_cash_report();
    }

    /// Cari bakiye tablosunda bir kolon başlığına tıklar.
    pub fn toggle_customer_balance_sort(&mut self, column: CustomerBalanceSortColumn) {
        if self.reports.customer_sort_column == column {
            self.reports.customer_sort_ascending = !self.reports.customer_sort_ascending;
        } else {
            self.reports.customer_sort_column = column;
            // Bakiye için ilk tıklamada da mutlak değere göre azalan sıra korunur.
            self.reports.customer_sort_ascending = column != CustomerBalanceSortColumn::Balance;
        }
        self.reports.customer_balance_report.sort_by(
            self.reports.customer_sort_column,
            self.reports.customer_sort_ascending,
        );
    }

    /// Stok durum filtresini değiştirir (yalnızca "Uygula" ile rapor yenilenir).
    pub fn set_stock_status_filter(&mut self, status: Option<StockStatus>) {
        self.reports.stock_filter.status = status;
    }

    /// Aktif raporu yeniler.
    pub fn refresh_current_report(&mut self) {
        match self.reports.tab {
            ReportKind::CustomerStatement => self.refresh_customer_statement(),
            ReportKind::CustomerBalances => self.refresh_customer_balance_report(),
            ReportKind::StockStatus => self.refresh_stock_report(),
            ReportKind::SalesPurchase => self.refresh_sales_report(),
            ReportKind::CashSummary => self.refresh_cash_report(),
            ReportKind::IncomeExpense => self.refresh_income_expense_report(),
        }
    }

    /// Geçerli tarih aralığını döndürür; hata varsa `None`.
    fn report_range(&mut self) -> Option<(String, String)> {
        let from = self.reports.date_from.trim().to_owned();
        let to = self.reports.date_to.trim().to_owned();
        if self.reports.tab.uses_date_range()
            && let Err(message) = validate_date_range(&from, &to)
        {
            self.reports.error = Some(message);
            return None;
        }
        Some((from, to))
    }
    /// Cari ekstreyi yeniler.
    pub fn refresh_customer_statement(&mut self) {
        self.reports.error = None;
        let Some((from, to)) = self.report_range() else {
            self.reports.customer_statement = None;
            return;
        };
        let Some(customer_id) = self.reports.customer_id else {
            self.reports.customer_statement = None;
            return;
        };
        let Some(database) = &self.database else {
            self.reports.error = Some("Veritabanı açık değil; rapor okunamıyor.".to_owned());
            self.reports.customer_statement = None;
            return;
        };

        match database.customer_statement(customer_id, &from, &to) {
            Ok(statement) => self.reports.customer_statement = statement,
            Err(error) => {
                self.reports.error = Some(error.to_string());
                self.reports.customer_statement = None;
            }
        }
    }

    /// Cari bakiye özetini yeniler.
    pub fn refresh_customer_balance_report(&mut self) {
        self.reports.error = None;
        let Some((from, to)) = self.report_range() else {
            self.reports.customer_balance_report = CustomerBalanceReport::default();
            return;
        };
        let Some(database) = &self.database else {
            self.reports.error = Some("Veritabanı açık değil; rapor okunamıyor.".to_owned());
            return;
        };

        match database.customer_balance_report(&from, &to, &self.reports.customer_balance_filter) {
            Ok(mut report) => {
                report.sort_by(
                    self.reports.customer_sort_column,
                    self.reports.customer_sort_ascending,
                );
                self.reports.customer_balance_report = report;
            }
            Err(error) => {
                self.reports.error = Some(error.to_string());
                self.reports.customer_balance_report = CustomerBalanceReport::default();
            }
        }
    }

    /// Stok durumu raporunu yeniler.
    pub fn refresh_stock_report(&mut self) {
        self.reports.error = None;
        let Some(database) = &self.database else {
            self.reports.error = Some("Veritabanı açık değil; rapor okunamıyor.".to_owned());
            self.reports.stock_report = StockReport::default();
            return;
        };

        match database.stock_status_report(&self.reports.stock_filter) {
            Ok(report) => self.reports.stock_report = report,
            Err(error) => {
                self.reports.error = Some(error.to_string());
                self.reports.stock_report = StockReport::default();
            }
        }
    }

    /// Satış/alış özetini yeniler.
    pub fn refresh_sales_report(&mut self) {
        self.reports.error = None;
        let Some((from, to)) = self.report_range() else {
            self.reports.sales_report = SalesPurchaseReport::default();
            return;
        };
        let Some(database) = &self.database else {
            self.reports.error = Some("Veritabanı açık değil; rapor okunamıyor.".to_owned());
            return;
        };

        match database.sales_purchase_report(&from, &to, self.reports.sales_grouping) {
            Ok(report) => self.reports.sales_report = report,
            Err(error) => {
                self.reports.error = Some(error.to_string());
                self.reports.sales_report = SalesPurchaseReport::default();
            }
        }
    }

    /// Kasa/banka hareket özetini yeniler.
    pub fn refresh_cash_report(&mut self) {
        self.reports.error = None;
        let Some((from, to)) = self.report_range() else {
            self.reports.cash_report = None;
            return;
        };
        let Some(database) = &self.database else {
            self.reports.error = Some("Veritabanı açık değil; rapor okunamıyor.".to_owned());
            return;
        };

        match database.cash_movement_report(&from, &to, self.reports.cash_account_id) {
            Ok(Some(report)) => self.reports.cash_report = Some(report),
            Ok(None) => {
                self.reports.cash_report = None;
                self.reports.error = Some("Seçili kasa/banka hesabı bulunamadı.".to_owned());
            }
            Err(error) => {
                self.reports.error = Some(error.to_string());
                self.reports.cash_report = None;
            }
        }
    }

    /// Gelir/gider özetini yeniler.
    pub fn refresh_income_expense_report(&mut self) {
        self.reports.error = None;
        let Some((from, to)) = self.report_range() else {
            self.reports.income_expense_report = None;
            return;
        };
        let Some(database) = &self.database else {
            self.reports.error = Some("Veritabanı açık değil; rapor okunamıyor.".to_owned());
            return;
        };

        match database.income_expense_report(&from, &to) {
            Ok(report) => self.reports.income_expense_report = Some(report),
            Err(error) => {
                self.reports.error = Some(error.to_string());
                self.reports.income_expense_report = None;
            }
        }
    }

    /// Cari bakiye filtresinde tür seçimini uygular (rapor "Uygula" ile yenilenir).
    pub fn set_customer_balance_kind(&mut self, kind: Option<CustomerType>) {
        self.reports.customer_balance_filter.kind = kind;
    }
}
