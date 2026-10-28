// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! CSV dışa aktarma içerikleri.
//!
//! Her rapor ve liste için sütun başlıkları burada tanımlanır. Biçimlendirme
//! [`crate::csv`] modülüne, dosya seçimi arayüze aittir. Bu modül SQL yazmaz;
//! yalnızca uygulama durumundaki hazır verileri ve veritabanının hazır okuma
//! metotlarını kullanır.

use std::path::Path;

use crate::csv;
use crate::error::{AppError, Result};
use crate::income_expense::EntryFilter;
use crate::money::Money;
use crate::report::{
    CustomerBalanceReport, CustomerStatement, IncomeExpenseReport, SalesPurchaseBreakdown,
    SalesPurchaseReport, StockReport, percentage_label,
};

use super::OnMuhasebeApp;

impl OnMuhasebeApp {
    /// Aktif raporu CSV olarak yazar.
    pub fn export_report_csv(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let outcome = self.report_csv_rows().and_then(|rows| {
            csv::write_rows(path, &rows)?;
            Ok(())
        });
        self.finish_csv_export(outcome, path)
    }

    /// Cari hesap listesini CSV olarak yazar.
    pub fn export_customer_list_csv(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let rows = customer_list_rows(self);
        let outcome = csv::write_rows(path, &rows);
        self.finish_csv_export(outcome, path)
    }

    /// Ürün listesini CSV olarak yazar.
    pub fn export_product_list_csv(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let rows = product_list_rows(self);
        let outcome = csv::write_rows(path, &rows);
        self.finish_csv_export(outcome, path)
    }

    /// Aktif fatura listesini CSV olarak yazar.
    pub fn export_invoice_list_csv(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let rows = invoice_list_rows(self);
        let outcome = csv::write_rows(path, &rows);
        self.finish_csv_export(outcome, path)
    }

    /// Tahsilat/ödeme hareket listesini CSV olarak yazar.
    pub fn export_payment_list_csv(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let outcome = self.payment_list_rows().and_then(|rows| {
            csv::write_rows(path, &rows)?;
            Ok(())
        });
        self.finish_csv_export(outcome, path)
    }

    /// Gelir/gider hareket listesini CSV olarak yazar.
    pub fn export_income_expense_list_csv(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let outcome = self.income_expense_list_rows().and_then(|rows| {
            csv::write_rows(path, &rows)?;
            Ok(())
        });
        self.finish_csv_export(outcome, path)
    }

    /// Kasa/banka hesap listesini CSV olarak yazar.
    pub fn export_cash_account_list_csv(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let rows = cash_account_list_rows(self);
        let outcome = csv::write_rows(path, &rows);
        self.finish_csv_export(outcome, path)
    }

    fn finish_csv_export(&mut self, outcome: Result<()>, path: &Path) -> Result<()> {
        self.record_csv_write(outcome, path, "CSV kaydedildi")
    }

    fn report_csv_rows(&self) -> Result<Vec<Vec<String>>> {
        let state = &self.reports;
        match state.tab {
            crate::report::ReportKind::CustomerStatement => {
                let statement = state.customer_statement.as_ref().ok_or_else(|| {
                    AppError::Csv("Cari ekstre için önce bir cari hesap seçin.".to_owned())
                })?;
                Ok(customer_statement_rows(statement))
            }
            crate::report::ReportKind::CustomerBalances => {
                Ok(customer_balance_rows(&state.customer_balance_report))
            }
            crate::report::ReportKind::StockStatus => Ok(stock_rows(&state.stock_report)),
            crate::report::ReportKind::SalesPurchase => {
                Ok(sales_purchase_rows(&state.sales_report))
            }
            crate::report::ReportKind::CashSummary => {
                let report = state.cash_report.as_ref().ok_or_else(|| {
                    AppError::Csv("Kasa/banka raporu henüz oluşturulmadı.".to_owned())
                })?;
                Ok(cash_report_rows(report))
            }
            crate::report::ReportKind::IncomeExpense => {
                let report = state.income_expense_report.as_ref().ok_or_else(|| {
                    AppError::Csv("Gelir/gider raporu henüz oluşturulmadı.".to_owned())
                })?;
                Ok(income_expense_report_rows(report))
            }
        }
    }

    fn payment_list_rows(&self) -> Result<Vec<Vec<String>>> {
        let Some(database) = self.database.as_ref() else {
            return Err(AppError::Csv("Veritabanı açık değil.".to_owned()));
        };
        let mut filter = self.cash.movement_filter.clone();
        filter.payment_type = self.cash.movement_tab.payment_type();
        let payments = database.payment_list(&filter)?;
        Ok(payment_rows(&payments))
    }

    fn income_expense_list_rows(&self) -> Result<Vec<Vec<String>>> {
        let Some(database) = self.database.as_ref() else {
            return Err(AppError::Csv("Veritabanı açık değil.".to_owned()));
        };
        let tab = self.cash.movement_tab;
        let filter = EntryFilter {
            search: self.cash.movement_filter.search.clone(),
            entry_type: tab.entry_type(),
            category_id: if matches!(
                tab,
                super::MovementTab::Incomes | super::MovementTab::Expenses
            ) {
                self.cash.movement_category_id
            } else {
                None
            },
            cash_account_id: self.cash.movement_filter.cash_account_id,
            status: self.cash.movement_filter.status.map(|status| match status {
                crate::payment::PaymentStatus::Kesin => crate::income_expense::EntryStatus::Kesin,
                crate::payment::PaymentStatus::Iptal => crate::income_expense::EntryStatus::Iptal,
            }),
            date_from: self.cash.movement_filter.date_from.clone(),
            date_to: self.cash.movement_filter.date_to.clone(),
        };
        let entries = database.income_expense_list(&filter)?;
        Ok(income_expense_entry_rows(&entries))
    }
}

fn header(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn empty() -> Vec<String> {
    Vec::new()
}

fn customer_statement_rows(statement: &CustomerStatement) -> Vec<Vec<String>> {
    let mut rows = vec![header(&[
        "Cari Kodu",
        "Cari Adı",
        "Tarih",
        "Tür",
        "Belge No",
        "Açıklama",
        "Borç",
        "Alacak",
        "Bakiye",
    ])];
    rows.push(vec![
        statement.customer_code.clone(),
        statement.customer_name.clone(),
        String::new(),
        "Dönem Başı".to_owned(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        csv::money(statement.opening_balance),
    ]);
    for line in &statement.rows {
        rows.push(vec![
            statement.customer_code.clone(),
            statement.customer_name.clone(),
            line.date.clone(),
            line.kind.label().to_owned(),
            csv::optional(line.document_no.as_deref()),
            csv::optional(line.description.as_deref()),
            csv::money(line.debit),
            csv::money(line.credit),
            csv::money(line.balance),
        ]);
    }
    rows.push(vec![
        statement.customer_code.clone(),
        statement.customer_name.clone(),
        String::new(),
        "Dönem Toplamı".to_owned(),
        String::new(),
        String::new(),
        csv::money(statement.total_debit),
        csv::money(statement.total_credit),
        csv::money(statement.closing_balance),
    ]);
    rows
}

fn customer_balance_rows(report: &CustomerBalanceReport) -> Vec<Vec<String>> {
    let mut rows = vec![header(&[
        "Cari Kodu",
        "Cari Adı",
        "Tür",
        "Satış",
        "Alış",
        "Tahsilat",
        "Ödeme",
        "Bakiye",
        "Durum",
    ])];
    for line in &report.rows {
        rows.push(vec![
            line.code.clone(),
            line.name.clone(),
            line.kind.label().to_owned(),
            csv::money(line.sales_total),
            csv::money(line.purchase_total),
            csv::money(line.collection_total),
            csv::money(line.payment_total),
            csv::money(line.balance),
            line.status().label().to_owned(),
        ]);
    }
    rows.push(empty());
    rows.push(vec![
        "Toplam Alacak".to_owned(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        csv::money(report.total_receivable),
        String::new(),
    ]);
    rows.push(vec![
        "Toplam Borç".to_owned(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        csv::money(report.total_payable),
        String::new(),
    ]);
    rows.push(vec![
        "Net Bakiye".to_owned(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        csv::money(report.net_balance),
        String::new(),
    ]);
    rows
}

fn stock_rows(report: &StockReport) -> Vec<Vec<String>> {
    let mut rows = vec![header(&[
        "Ürün Kodu",
        "Ürün Adı",
        "Birim",
        "Güncel Stok",
        "Minimum Stok",
        "Durum",
        "Alış Değeri",
        "Satış Değeri",
    ])];
    for line in &report.rows {
        rows.push(vec![
            line.code.clone(),
            line.name.clone(),
            line.unit.label().to_owned(),
            csv::quantity(line.current_stock),
            csv::quantity(line.min_stock),
            line.status.label().to_owned(),
            csv::money(line.purchase_value),
            csv::money(line.sale_value),
        ]);
    }
    rows.push(empty());
    rows.push(vec![
        "Özet".to_owned(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
    ]);
    rows.push(vec![
        "Aktif ürün".to_owned(),
        report.stats.active_products.to_string(),
        String::new(),
        String::new(),
        String::new(),
        "Düşük stok".to_owned(),
        report.stats.low_stock.to_string(),
        String::new(),
    ]);
    rows.push(vec![
        "Sıfır stok".to_owned(),
        report.stats.zero_stock.to_string(),
        String::new(),
        String::new(),
        String::new(),
        "Eksi stok".to_owned(),
        report.stats.negative_stock.to_string(),
        String::new(),
    ]);
    rows.push(vec![
        "Toplam alış değeri".to_owned(),
        csv::money(report.stats.purchase_value),
        String::new(),
        String::new(),
        String::new(),
        "Toplam satış değeri".to_owned(),
        csv::money(report.stats.sale_value),
        String::new(),
    ]);
    rows
}

fn sales_purchase_rows(report: &SalesPurchaseReport) -> Vec<Vec<String>> {
    let mut rows;
    match &report.breakdown {
        SalesPurchaseBreakdown::Customer(lines) => {
            rows = vec![header(&[
                "Cari Kodu",
                "Cari Adı",
                "Satış Adedi",
                "Satış Toplamı",
                "Alış Adedi",
                "Alış Toplamı",
                "Net",
            ])];
            for line in lines {
                rows.push(vec![
                    line.code.clone(),
                    line.name.clone(),
                    line.sales_count.to_string(),
                    csv::money(line.sales_total),
                    line.purchase_count.to_string(),
                    csv::money(line.purchase_total),
                    csv::money(Money::from_kurus(
                        line.sales_total.kurus() - line.purchase_total.kurus(),
                    )),
                ]);
            }
        }
        SalesPurchaseBreakdown::Product(lines) => {
            rows = vec![header(&[
                "Ürün Kodu",
                "Ürün Adı",
                "Birim",
                "Satılan Miktar",
                "Satış Toplamı",
                "Alınan Miktar",
                "Alış Toplamı",
                "Net",
            ])];
            for line in lines {
                rows.push(vec![
                    line.code.clone(),
                    line.name.clone(),
                    line.unit.label().to_owned(),
                    csv::quantity(line.sold_quantity),
                    csv::money(line.sales_total),
                    csv::quantity(line.purchased_quantity),
                    csv::money(line.purchase_total),
                    csv::money(Money::from_kurus(
                        line.sales_total.kurus() - line.purchase_total.kurus(),
                    )),
                ]);
            }
        }
    }

    rows.push(empty());
    rows.push(vec![
        "Genel Toplam".to_owned(),
        String::new(),
        format!("{} satış", report.sales_count),
        csv::money(report.sales_total),
        format!("{} alış", report.purchase_count),
        csv::money(report.purchase_total),
        csv::money(report.net()),
    ]);
    rows
}

fn cash_report_rows(report: &crate::report::CashMovementReport) -> Vec<Vec<String>> {
    let mut rows = vec![vec!["Kasa/Banka Hesap Dökümü".to_owned()]];
    rows.push(header(&[
        "Hesap",
        "Tür",
        "Açılış",
        "Tahsilat",
        "Ödeme",
        "Gelir",
        "Gider",
        "Net",
        "Kapanış",
    ]));
    for line in &report.accounts {
        rows.push(vec![
            line.account_name.clone(),
            line.account_type.label().to_owned(),
            csv::money(line.opening_balance),
            csv::money(line.collections),
            csv::money(line.payments),
            csv::money(line.incomes),
            csv::money(line.expenses),
            csv::money(line.net()),
            csv::money(line.closing_balance),
        ]);
    }
    rows.push(vec![
        "Genel Toplam".to_owned(),
        String::new(),
        csv::money(report.opening_balance),
        csv::money(report.collections),
        csv::money(report.payments),
        csv::money(report.incomes),
        csv::money(report.expenses),
        csv::money(Money::from_kurus(
            report
                .closing_balance
                .kurus()
                .saturating_sub(report.opening_balance.kurus()),
        )),
        csv::money(report.closing_balance),
    ]);

    rows.push(empty());
    rows.push(vec!["Hareketler".to_owned()]);
    rows.push(header(&[
        "Tarih",
        "Tür",
        "Hesap",
        "Cari/Kategori",
        "Açıklama",
        "Tutar",
    ]));
    for movement in &report.movements {
        rows.push(vec![
            movement.date.clone(),
            movement.kind.label().to_owned(),
            movement.account_name.clone(),
            movement.party.clone(),
            csv::optional(movement.description.as_deref()),
            csv::money(movement.amount),
        ]);
    }
    rows
}

fn income_expense_report_rows(report: &IncomeExpenseReport) -> Vec<Vec<String>> {
    let summary = &report.summary;
    let mut rows = vec![vec!["Gelir/Gider Özeti".to_owned()]];
    rows.push(header(&[
        "Dönem Başlangıcı",
        "Dönem Bitişi",
        "Gelir",
        "Gider",
        "Net",
    ]));
    rows.push(vec![
        summary.date_from.clone(),
        summary.date_to.clone(),
        csv::money(summary.income_total),
        csv::money(summary.expense_total),
        csv::money(Money::from_kurus(
            summary.income_total.kurus() - summary.expense_total.kurus(),
        )),
    ]);

    rows.push(empty());
    rows.push(vec!["Kategoriler".to_owned()]);
    rows.push(header(&["Tür", "Kategori", "Tutar", "Yüzde"]));
    for category in &summary.categories {
        rows.push(vec![
            category.entry_type.label().to_owned(),
            category.category_name.clone(),
            csv::money(category.total),
            percentage_label(category.percentage),
        ]);
    }

    rows.push(empty());
    rows.push(vec!["Hesaplar".to_owned()]);
    rows.push(header(&["Kasa/Banka", "Gelir", "Gider", "Net"]));
    for account in &summary.accounts {
        rows.push(vec![
            account.cash_account_name.clone(),
            csv::money(account.income_total),
            csv::money(account.expense_total),
            csv::money(account.net()),
        ]);
    }

    rows.push(empty());
    rows.push(vec!["Aylık Trend".to_owned()]);
    rows.push(header(&["Ay", "Gelir", "Gider", "Net"]));
    for month in &report.trend {
        rows.push(vec![
            month.label(),
            csv::money(month.income_total),
            csv::money(month.expense_total),
            csv::money(month.net()),
        ]);
    }
    rows
}

fn customer_list_rows(app: &OnMuhasebeApp) -> Vec<Vec<String>> {
    let mut rows = vec![header(&[
        "Cari Kodu",
        "Cari Adı",
        "Cari Türü",
        "Telefon",
        "E-posta",
        "Vergi Dairesi",
        "Vergi No",
        "TCKN",
        "Yetkili",
        "Adres",
        "Notlar",
        "Durum",
        "Bakiye",
    ])];
    for customer in &app.customer.list {
        rows.push(vec![
            customer.code.clone(),
            customer.name.clone(),
            customer.kind.label().to_owned(),
            csv::optional(customer.phone.as_deref()),
            csv::optional(customer.email.as_deref()),
            csv::optional(customer.tax_office.as_deref()),
            csv::optional(customer.tax_number.as_deref()),
            csv::optional(customer.national_id.as_deref()),
            csv::optional(customer.contact_person.as_deref()),
            csv::optional(customer.address.as_deref()),
            csv::optional(customer.notes.as_deref()),
            customer.status_label().to_owned(),
            csv::money(customer.balance()),
        ]);
    }
    rows
}

fn product_list_rows(app: &OnMuhasebeApp) -> Vec<Vec<String>> {
    let mut rows = vec![header(&[
        "Ürün Kodu",
        "Barkod",
        "Ürün Adı",
        "Birim",
        "Kategori",
        "Alış Fiyatı",
        "Satış Fiyatı",
        "KDV Oranı",
        "Minimum Stok",
        "Güncel Stok",
        "Notlar",
        "Durum",
    ])];
    for product in &app.product.list {
        rows.push(vec![
            product.code.clone(),
            csv::optional(product.barcode.as_deref()),
            product.name.clone(),
            product.unit.label().to_owned(),
            csv::optional(product.category.as_deref()),
            csv::money(product.purchase_price),
            csv::money(product.sale_price),
            product.vat_label(),
            csv::quantity(product.min_stock),
            csv::quantity(product.current_stock),
            csv::optional(product.notes.as_deref()),
            product.status_label().to_owned(),
        ]);
    }
    rows
}

fn invoice_list_rows(app: &OnMuhasebeApp) -> Vec<Vec<String>> {
    let mut rows = vec![header(&[
        "Fatura No",
        "Tür",
        "Tarih",
        "Vade",
        "Cari Kodu",
        "Cari Adı",
        "Ara Toplam",
        "İskonto",
        "KDV",
        "Genel Toplam",
        "Ödenen",
        "Kalan",
        "Durum",
    ])];
    for invoice in &app.invoice.list {
        rows.push(vec![
            invoice.number.clone(),
            invoice.invoice_type.label().to_owned(),
            invoice.invoice_date.clone(),
            csv::optional(invoice.due_date.as_deref()),
            invoice.customer_code.clone(),
            invoice.customer_name.clone(),
            csv::money(invoice.totals.subtotal),
            csv::money(invoice.totals.discount_total),
            csv::money(invoice.totals.vat_total),
            csv::money(invoice.totals.grand_total),
            csv::money(invoice.paid_amount),
            csv::money(invoice.remaining()),
            invoice.status.label().to_owned(),
        ]);
    }
    rows
}

fn payment_rows(payments: &[crate::payment::Payment]) -> Vec<Vec<String>> {
    let mut rows = vec![header(&[
        "Tür",
        "Tarih",
        "Cari Kodu",
        "Cari Adı",
        "Kasa/Banka",
        "Tutar",
        "Dağıtılan",
        "Avans",
        "Açıklama",
        "Durum",
    ])];
    for payment in payments {
        rows.push(vec![
            payment.payment_type.label().to_owned(),
            payment.payment_date.clone(),
            payment.customer_code.clone(),
            payment.customer_name.clone(),
            payment.cash_account_name.clone(),
            csv::money(payment.amount),
            csv::money(payment.allocated_total),
            csv::money(payment.remainder()),
            csv::optional(payment.description.as_deref()),
            payment.status_label().to_owned(),
        ]);
    }
    rows
}

fn income_expense_entry_rows(
    entries: &[crate::income_expense::IncomeExpenseEntry],
) -> Vec<Vec<String>> {
    let mut rows = vec![header(&[
        "Tür",
        "Tarih",
        "Kategori",
        "Kasa/Banka",
        "Tutar",
        "Açıklama",
        "Belge No",
        "Durum",
    ])];
    for entry in entries {
        rows.push(vec![
            entry.entry_type.label().to_owned(),
            entry.entry_date.clone(),
            entry.category_name.clone(),
            entry.cash_account_name.clone(),
            csv::money(entry.amount),
            csv::optional(entry.description.as_deref()),
            csv::optional(entry.document_ref.as_deref()),
            entry.status_label().to_owned(),
        ]);
    }
    rows
}

fn cash_account_list_rows(app: &OnMuhasebeApp) -> Vec<Vec<String>> {
    let mut rows = vec![header(&[
        "Hesap Adı",
        "Tür",
        "Banka",
        "IBAN",
        "Açılış Bakiyesi",
        "Güncel Bakiye",
        "Notlar",
        "Durum",
    ])];
    for account in &app.cash.account_list {
        rows.push(vec![
            account.name.clone(),
            account.account_type.label().to_owned(),
            csv::optional(account.bank_name.as_deref()),
            csv::optional(account.iban.as_deref()),
            csv::money(account.opening_balance),
            csv::money(account.current_balance),
            csv::optional(account.notes.as_deref()),
            if account.active { "Aktif" } else { "Pasif" }.to_owned(),
        ]);
    }
    rows
}
