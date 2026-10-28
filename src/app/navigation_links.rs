// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Ekranlar arası bağlantılar.
//!
//! Ayrıntı ekranlarındaki cari, ürün, fatura ve kasa/banka bağlantıları bu
//! modüldeki metotlarla hedef ekranı hazırlar. Bağlantılar tarayıcı geçmişi
//! tutmaz; mevcut `Listeye dön` / `Geri dön` davranışı korunur.

use crate::cash::CashAccountFilter;
use crate::error::AppError;
use crate::invoice::{InvoiceFilter, InvoiceType};
use crate::report::ReportKind;

use super::{CashScreen, CashTab, OnMuhasebeApp, View};

impl OnMuhasebeApp {
    /// Cari hesap ayrıntısını açar.
    pub fn show_customer_detail(&mut self, customer_id: i64) {
        self.set_view(View::Customers);
        self.open_customer_detail(customer_id);
    }

    /// Ürün ayrıntısını açar.
    pub fn show_product_detail(&mut self, product_id: i64) {
        self.set_view(View::Inventory);
        self.open_product_detail(product_id);
    }

    /// Cari hesabın satış faturalarını, yalnızca o cariye filtrelenmiş olarak açar.
    pub fn show_customer_sales_invoices(&mut self, customer_id: i64) {
        self.set_view(View::SalesInvoices);
        self.invoice.filter = InvoiceFilter {
            customer_id: Some(customer_id),
            ..InvoiceFilter::default()
        };
        self.invoice.screen = super::InvoiceScreen::List;
        self.invoice.errors = crate::invoice::FieldErrors::default();
        self.invoice.general_error = None;
        self.refresh_invoice_list();
        self.request_scroll_reset();
    }

    /// Cari ekstre raporunu seçili cariyle açar.
    pub fn show_customer_statement(&mut self, customer_id: i64) {
        self.set_view(View::Reports);
        if self.reports.tab != ReportKind::CustomerStatement {
            self.set_report_tab(ReportKind::CustomerStatement);
        }
        self.set_report_customer(Some(customer_id));
        self.request_scroll_reset();
    }

    /// Faturayı türüne göre doğru bölümde açar.
    pub fn show_invoice_detail(&mut self, invoice_id: i64) {
        let invoice_type = match &self.database {
            Some(database) => match database.invoice_get(invoice_id) {
                Ok(Some(invoice)) => invoice.invoice_type,
                Ok(None) => {
                    self.set_view(View::SalesInvoices);
                    self.invoice.general_error =
                        Some(format!("Fatura bulunamadı (kayıt no: {invoice_id})."));
                    return;
                }
                Err(error) => {
                    self.set_view(View::SalesInvoices);
                    self.invoice.general_error = Some(error.to_string());
                    return;
                }
            },
            None => {
                self.set_view(View::SalesInvoices);
                self.invoice.general_error =
                    Some(AppError::Csv("Veritabanı açık değil.".to_owned()).to_string());
                return;
            }
        };

        let view = match invoice_type {
            InvoiceType::Sales => View::SalesInvoices,
            InvoiceType::Purchase => View::PurchaseInvoices,
        };
        self.set_view(view);
        self.open_invoice_detail(invoice_id);
    }

    /// Kasa/banka hesabını hesaplar listesinde vurgular.
    pub fn show_cash_account_highlight(&mut self, account_id: i64) {
        self.set_view(View::CashAndBank);
        self.cash.tab = CashTab::Accounts;
        self.cash.screen = CashScreen::AccountList;
        self.cash.account_filter = CashAccountFilter::default();
        self.cash.general_error = None;
        self.cash.highlight_account_id = Some(account_id);
        self.refresh_cash_account_list();
        self.request_scroll_reset();
    }
}
