// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Rapor sorguları.
//!
//! Bu modül yalnızca **okuma** yapar; hiçbir kayıt eklmez, değiştirmez veya
//! silmez. Tüm kullanıcı girdileri parametreli sorgularla iletilir.
//!
//! Hesaplamaların bir kısmı mevcut veri erişim metotları (`invoice_list_for_customer`,
//! `payment_list`, `income_expense_list`, `product_list`) üzerinden yapılır.
//! Böylece raporlar, ekranlarda gösterilen verilerle aynı SQL ve aynı iptal
//! kurallarını kullanır.

use chrono::Local;
use rusqlite::types::Type;
use rusqlite::{Row, params};

use crate::cash::CashAccountFilter;
use crate::customer::CustomerType;
use crate::error::{AppError, Result};
use crate::income_expense::{EntryFilter, EntryStatus};
use crate::invoice::InvoiceStatus;
use crate::payment::{PaymentFilter, PaymentStatus};
use crate::product::{ProductFilter, Unit};
use crate::quantity::Quantity;
use crate::report::{
    CashMovementReport, CashReportMovement, CustomerBalanceReport, CustomerBalanceReportFilter,
    CustomerBalanceRow, CustomerSalesRow, CustomerStatement, IncomeExpenseReport, LedgerMovement,
    ProductSalesRow, SalesPurchaseBreakdown, SalesPurchaseGrouping, SalesPurchaseReport,
    StockReport, StockReportFilter, TREND_MONTHS, ledger_kind_for_invoice, parse_iso_date,
    trend_start_date,
};

use super::Database;

impl Database {
    /// Tek bir cari için hesap ekstresi üretir.
    ///
    /// Cari yoksa `None` döner. Yalnızca kesinleşmiş faturalar ve
    /// tahsilat/ödemeler kullanılır. Dönem başı bakiyesi, `date_from`
    /// öncesindeki hareketlerden hesaplanır.
    pub fn customer_statement(
        &self,
        customer_id: i64,
        date_from: &str,
        date_to: &str,
    ) -> Result<Option<CustomerStatement>> {
        let Some(customer) = self.customer_get(customer_id)? else {
            return Ok(None);
        };
        let from = date_from.trim();
        let to = date_to.trim();

        let mut movements = Vec::new();

        for invoice in self.invoice_list_for_customer(customer_id)? {
            if invoice.status != InvoiceStatus::Confirmed
                || (!to.is_empty() && invoice.invoice_date.as_str() > to)
            {
                continue;
            }
            movements.push(LedgerMovement {
                id: invoice.id,
                date: invoice.invoice_date,
                kind: ledger_kind_for_invoice(invoice.invoice_type),
                document_no: Some(invoice.number),
                description: invoice.notes,
                amount: invoice.totals.grand_total,
            });
        }

        for payment in self.payment_list_for_customer(customer_id)? {
            if payment.status != PaymentStatus::Kesin
                || (!to.is_empty() && payment.payment_date.as_str() > to)
            {
                continue;
            }
            movements.push(LedgerMovement {
                id: payment.id,
                date: payment.payment_date,
                kind: match payment.payment_type {
                    crate::payment::PaymentType::Tahsilat => crate::report::LedgerKind::Collection,
                    crate::payment::PaymentType::Odeme => crate::report::LedgerKind::Payment,
                },
                document_no: None,
                description: payment.description,
                amount: payment.amount,
            });
        }

        Ok(Some(CustomerStatement::from_movements(
            &customer, from, to, movements,
        )))
    }

    /// Tüm cariler için bakiye özeti üretir.
    ///
    /// Tarih aralığı boşsa bakiye, [`crate::customer::Customer::balance`] ile
    /// aynıdır. Aralık verilirse yalnızca dönemdeki kesin hareketler toplanır.
    pub fn customer_balance_report(
        &self,
        date_from: &str,
        date_to: &str,
        filter: &CustomerBalanceReportFilter,
    ) -> Result<CustomerBalanceReport> {
        // Cari başına dört bağıntılı alt sorgu yerine, tarih aralığına göre
        // bir kez toplanan hareketleri LEFT JOIN ile birleştir.
        let mut statement = self.connection.prepare(
            "SELECT c.id, c.code, c.name, c.kind,
                COALESCE(sales.sales_total, 0),
                COALESCE(purchases.purchase_total, 0),
                COALESCE(collections.collection_total, 0),
                COALESCE(payments_total.payment_total, 0)
             FROM customers c
             LEFT JOIN (
                 SELECT customer_id, SUM(grand_total) AS sales_total
                 FROM invoices
                 WHERE status = 'kesin' AND invoice_type = 'satis'
                   AND (?3 = '' OR invoice_date >= ?3)
                   AND (?4 = '' OR invoice_date <= ?4)
                 GROUP BY customer_id
             ) sales ON sales.customer_id = c.id
             LEFT JOIN (
                 SELECT customer_id, SUM(grand_total) AS purchase_total
                 FROM invoices
                 WHERE status = 'kesin' AND invoice_type = 'alis'
                   AND (?3 = '' OR invoice_date >= ?3)
                   AND (?4 = '' OR invoice_date <= ?4)
                 GROUP BY customer_id
             ) purchases ON purchases.customer_id = c.id
             LEFT JOIN (
                 SELECT customer_id, SUM(amount) AS collection_total
                 FROM payments
                 WHERE status = 'kesin' AND payment_type = 'tahsilat'
                   AND (?3 = '' OR payment_date >= ?3)
                   AND (?4 = '' OR payment_date <= ?4)
                 GROUP BY customer_id
             ) collections ON collections.customer_id = c.id
             LEFT JOIN (
                 SELECT customer_id, SUM(amount) AS payment_total
                 FROM payments
                 WHERE status = 'kesin' AND payment_type = 'odeme'
                   AND (?3 = '' OR payment_date >= ?3)
                   AND (?4 = '' OR payment_date <= ?4)
                 GROUP BY customer_id
             ) payments_total ON payments_total.customer_id = c.id
             WHERE (?1 IS NULL OR c.kind = ?1)
               AND (?2 IS NULL OR c.active = ?2)
             ORDER BY c.code COLLATE NOCASE",
        )?;

        let kind = filter.kind.map(CustomerType::as_db);
        let active = filter.active.map(i64::from);
        let from = date_from.trim();
        let to = date_to.trim();
        let rows = statement.query_map(params![kind, active, from, to], customer_balance_row)?;

        let mut report_rows = Vec::new();
        for row in rows {
            report_rows.push(row?);
        }

        Ok(CustomerBalanceReport::from_rows(report_rows, filter))
    }

    /// Ürünlerin güncel stok durumu raporunu üretir.
    ///
    /// Tüm ürünler okunur; "yalnızca aktif" ve durum/arama filtreleri rapor
    /// modelinde uygulanır. Alt bilgi sayıları her zaman tüm aktif ürünleri
    /// kapsar.
    pub fn stock_status_report(&self, filter: &StockReportFilter) -> Result<StockReport> {
        let products = self.product_list(&ProductFilter::default())?;
        StockReport::from_products(&products, filter)
    }

    /// Kesinleşmiş satış/alış faturalarının dönem özetini üretir.
    pub fn sales_purchase_report(
        &self,
        date_from: &str,
        date_to: &str,
        grouping: SalesPurchaseGrouping,
    ) -> Result<SalesPurchaseReport> {
        let from = date_from.trim();
        let to = date_to.trim();

        let (sales_count, sales_total, purchase_count, purchase_total) =
            self.connection.query_row(
                "SELECT
                    COALESCE(SUM(CASE WHEN invoice_type = 'satis' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN invoice_type = 'satis' THEN grand_total ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN invoice_type = 'alis' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN invoice_type = 'alis' THEN grand_total ELSE 0 END), 0)
                 FROM invoices
                 WHERE status = 'kesin'
                   AND (?1 = '' OR invoice_date >= ?1)
                   AND (?2 = '' OR invoice_date <= ?2)",
                params![from, to],
                |row| {
                    let sales_count: i64 = row.get(0)?;
                    let sales_total: i64 = row.get(1)?;
                    let purchase_count: i64 = row.get(2)?;
                    let purchase_total: i64 = row.get(3)?;
                    Ok((
                        sales_count.max(0) as usize,
                        crate::money::Money::from_kurus(sales_total),
                        purchase_count.max(0) as usize,
                        crate::money::Money::from_kurus(purchase_total),
                    ))
                },
            )?;

        let breakdown = match grouping {
            SalesPurchaseGrouping::Customer => {
                SalesPurchaseBreakdown::Customer(self.sales_customer_rows(from, to)?)
            }
            SalesPurchaseGrouping::Product => {
                SalesPurchaseBreakdown::Product(self.sales_product_rows(from, to)?)
            }
        };

        Ok(SalesPurchaseReport {
            sales_count,
            sales_total,
            purchase_count,
            purchase_total,
            breakdown,
        })
    }

    /// Kasa/banka hareket özetini üretir.
    ///
    /// `account_id` `None` ise tüm hesaplar dahil edilir ve hesap bazlı döküm
    /// döner. Seçili hesap yoksa `None` döner.
    pub fn cash_movement_report(
        &self,
        date_from: &str,
        date_to: &str,
        account_id: Option<i64>,
    ) -> Result<Option<CashMovementReport>> {
        let accounts = match account_id {
            Some(id) => match self.cash_account_get(id)? {
                Some(account) => vec![account],
                None => return Ok(None),
            },
            None => self.cash_account_list(&CashAccountFilter::default())?,
        };

        let payments = self.payment_list(&PaymentFilter {
            cash_account_id: account_id,
            status: Some(PaymentStatus::Kesin),
            date_to: date_to.trim().to_owned(),
            ..PaymentFilter::default()
        })?;
        let entries = self.income_expense_list(&EntryFilter {
            cash_account_id: account_id,
            status: Some(EntryStatus::Kesin),
            date_to: date_to.trim().to_owned(),
            ..EntryFilter::default()
        })?;

        let mut movements = Vec::with_capacity(payments.len() + entries.len());
        movements.extend(payments.iter().map(CashReportMovement::from_payment));
        movements.extend(entries.iter().map(CashReportMovement::from_entry));

        Ok(Some(CashMovementReport::from_accounts(
            &accounts, movements, date_from, date_to,
        )))
    }

    /// Gelir/gider dönem özetini ve son altı aylık trendi üretir.
    pub fn income_expense_report(
        &self,
        date_from: &str,
        date_to: &str,
    ) -> Result<IncomeExpenseReport> {
        let period_entries = self.income_expense_list(&EntryFilter {
            date_from: date_from.trim().to_owned(),
            date_to: date_to.trim().to_owned(),
            ..EntryFilter::default()
        })?;

        let trend_end = if date_to.trim().is_empty() {
            Local::now().date_naive()
        } else {
            parse_iso_date(date_to).unwrap_or_else(|| Local::now().date_naive())
        };
        let trend_start = trend_start_date(trend_end, TREND_MONTHS);
        let trend_end_text = trend_end.format("%Y-%m-%d").to_string();
        let trend_entries = self.income_expense_list(&EntryFilter {
            date_from: trend_start,
            date_to: trend_end_text,
            ..EntryFilter::default()
        })?;

        Ok(IncomeExpenseReport::from_entries(
            date_from,
            date_to,
            &period_entries,
            &trend_entries,
            trend_end,
        ))
    }

    /// Cari bazlı satış/alış dökümü.
    fn sales_customer_rows(&self, from: &str, to: &str) -> Result<Vec<CustomerSalesRow>> {
        let mut statement = self.connection.prepare(
            "SELECT c.id, c.code, c.name,
                COALESCE(SUM(CASE WHEN i.invoice_type = 'satis' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN i.invoice_type = 'satis' THEN i.grand_total ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN i.invoice_type = 'alis' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN i.invoice_type = 'alis' THEN i.grand_total ELSE 0 END), 0)
             FROM invoices i
             JOIN customers c ON c.id = i.customer_id
             WHERE i.status = 'kesin'
               AND (?1 = '' OR i.invoice_date >= ?1)
               AND (?2 = '' OR i.invoice_date <= ?2)
             GROUP BY c.id, c.code, c.name
             ORDER BY c.code COLLATE NOCASE",
        )?;

        let rows = statement.query_map(params![from, to], |row| {
            let sales_count: i64 = row.get(3)?;
            let sales_total: i64 = row.get(4)?;
            let purchase_count: i64 = row.get(5)?;
            let purchase_total: i64 = row.get(6)?;
            Ok(CustomerSalesRow {
                customer_id: row.get(0)?,
                code: row.get(1)?,
                name: row.get(2)?,
                sales_count: sales_count.max(0) as usize,
                sales_total: crate::money::Money::from_kurus(sales_total),
                purchase_count: purchase_count.max(0) as usize,
                purchase_total: crate::money::Money::from_kurus(purchase_total),
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(AppError::from)
    }

    /// Ürün bazlı satış/alış dökümü (fatura satırlarından).
    fn sales_product_rows(&self, from: &str, to: &str) -> Result<Vec<ProductSalesRow>> {
        let mut statement = self.connection.prepare(
            "SELECT p.id, p.code, p.name, p.unit,
                COALESCE(SUM(CASE WHEN i.invoice_type = 'satis' THEN il.quantity ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN i.invoice_type = 'satis' THEN il.line_total ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN i.invoice_type = 'alis' THEN il.quantity ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN i.invoice_type = 'alis' THEN il.line_total ELSE 0 END), 0)
             FROM invoice_lines il
             JOIN invoices i ON i.id = il.invoice_id
             JOIN products p ON p.id = il.product_id
             WHERE i.status = 'kesin'
               AND (?1 = '' OR i.invoice_date >= ?1)
               AND (?2 = '' OR i.invoice_date <= ?2)
             GROUP BY p.id, p.code, p.name, p.unit
             ORDER BY p.code COLLATE NOCASE",
        )?;

        let rows = statement.query_map(params![from, to], |row| {
            let unit_value: String = row.get(3)?;
            let unit = Unit::from_db(&unit_value).ok_or_else(|| {
                rusqlite::Error::FromSqlConversionFailure(
                    3,
                    Type::Text,
                    Box::new(AppError::InvalidRecord(format!(
                        "bilinmeyen ölçü birimi: '{unit_value}'"
                    ))),
                )
            })?;
            let sold_quantity: i64 = row.get(4)?;
            let sales_total: i64 = row.get(5)?;
            let purchased_quantity: i64 = row.get(6)?;
            let purchase_total: i64 = row.get(7)?;
            Ok(ProductSalesRow {
                product_id: row.get(0)?,
                code: row.get(1)?,
                name: row.get(2)?,
                unit,
                sold_quantity: Quantity::from_milli(sold_quantity),
                sales_total: crate::money::Money::from_kurus(sales_total),
                purchased_quantity: Quantity::from_milli(purchased_quantity),
                purchase_total: crate::money::Money::from_kurus(purchase_total),
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(AppError::from)
    }
}

/// Cari bakiye satırını okur.
fn customer_balance_row(row: &Row<'_>) -> rusqlite::Result<CustomerBalanceRow> {
    let kind_value: String = row.get(3)?;
    let kind = CustomerType::from_db(&kind_value).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            3,
            Type::Text,
            Box::new(AppError::InvalidRecord(format!(
                "bilinmeyen cari türü: '{kind_value}'"
            ))),
        )
    })?;

    let sales_total: i64 = row.get(4)?;
    let purchase_total: i64 = row.get(5)?;
    let collection_total: i64 = row.get(6)?;
    let payment_total: i64 = row.get(7)?;
    let balance = crate::money::Money::from_kurus(
        sales_total
            .saturating_sub(purchase_total)
            .saturating_sub(collection_total)
            .saturating_add(payment_total),
    );

    Ok(CustomerBalanceRow {
        customer_id: row.get(0)?,
        code: row.get(1)?,
        name: row.get(2)?,
        kind,
        sales_total: crate::money::Money::from_kurus(sales_total),
        purchase_total: crate::money::Money::from_kurus(purchase_total),
        collection_total: crate::money::Money::from_kurus(collection_total),
        payment_total: crate::money::Money::from_kurus(payment_total),
        balance,
    })
}
