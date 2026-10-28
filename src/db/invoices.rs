// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Fatura veri erişimi.
//!
//! SQL ifadeleri yalnızca bu modülde bulunur; arayüz katmanı [`Database`]
//! üzerindeki metotları çağırır ve hiçbir zaman SQL yazmaz.
//!
//! ## Güvenlik ve bütünlük
//!
//! * Tüm kullanıcı girdileri **parametreli** sorgularla (`?1`, `?2`, ...)
//!   iletilir.
//! * Yazma metotları yalnızca [`ValidatedInvoice`] kabul eder; doğrulanmamış
//!   veri bu katmana gelemez.
//! * Fatura, satırları, stok hareketleri ve güncel stok **tek bir işlem
//!   (transaction) içinde** yazılır. Herhangi bir adım başarısız olursa hiçbir
//!   kayıt kalmaz.
//! * Fatura **silinmez/düzenlenmez**: iptal `status` ile işaretlenir ve stok
//!   etkisi ters yönlü `duzeltme` hareketleriyle geri alınır.

use rusqlite::types::Type;
use rusqlite::{OptionalExtension, Row, params};

use crate::error::{AppError, Result};
use crate::invoice::{
    Invoice, InvoiceFilter, InvoiceLine, InvoiceStatus, InvoiceTotals, InvoiceType, LineAmounts,
    ValidatedInvoice, suggest_next_number,
};
use crate::money::Money;
use crate::product::Unit;
use crate::quantity::Quantity;

use super::Database;

/// Fatura sorgularında kullanılan kolon listesi (sırası [`row_to_invoice`] ile aynıdır).
const INVOICE_COLUMNS: &str = "i.id, i.invoice_number, i.invoice_type, i.customer_id, c.code, \
                               c.name, i.invoice_date, i.due_date, i.notes, i.subtotal, \
                               i.discount_total, i.net_total, i.vat_total, i.grand_total, \
                               i.status, i.cancelled_at, i.cancel_reason, i.created_at, \
                               i.updated_at, i.paid_amount";

/// Fatura satırı kolon listesi (sırası [`row_to_invoice_line`] ile aynıdır).
const INVOICE_LINE_COLUMNS: &str = "id, invoice_id, product_id, line_no, product_code, \
                                    product_name, unit, description, quantity, unit_price, \
                                    discount_rate, vat_rate, line_gross, line_discount, line_net, \
                                    line_vat, line_total";

/// Fatura satırında tür kolonunun indeksi.
const INVOICE_TYPE_INDEX: usize = 2;
/// Fatura satırında durum kolonunun indeksi.
const INVOICE_STATUS_INDEX: usize = 14;
/// Fatura satırında ölçü birimi kolonunun indeksi.
const LINE_UNIT_INDEX: usize = 6;

impl Database {
    /// Ölçütlere uyan faturaları (en yeni önce) döndürür.
    ///
    /// Tür, durum, cari ve tarih ölçütleri SQL'de (parametreli) uygulanır.
    /// Serbest metin araması, Türkçe karakterlerden etkilenmemesi için Rust
    /// tarafında yapılır.
    pub fn invoice_list(
        &self,
        invoice_type: InvoiceType,
        filter: &InvoiceFilter,
    ) -> Result<Vec<Invoice>> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {INVOICE_COLUMNS}
             FROM invoices i
             JOIN customers c ON c.id = i.customer_id
             WHERE i.invoice_type = ?1
               AND (?2 IS NULL OR i.status = ?2)
               AND (?3 IS NULL OR i.customer_id = ?3)
               AND (?4 = '' OR i.invoice_date >= ?4)
               AND (?5 = '' OR i.invoice_date <= ?5)
             ORDER BY i.invoice_date DESC, i.id DESC"
        ))?;

        let status = filter.status.map(InvoiceStatus::as_db);
        let date_from = filter.date_from.trim();
        let date_to = filter.date_to.trim();

        let rows = statement.query_map(
            params![
                invoice_type.as_db(),
                status,
                filter.customer_id,
                date_from,
                date_to
            ],
            row_to_invoice,
        )?;

        let mut invoices = Vec::new();
        for row in rows {
            invoices.push(row?);
        }

        let needle = crate::customer::normalize_for_search(filter.search.trim());
        if !needle.is_empty() {
            invoices.retain(|invoice| invoice.matches_search(&needle));
        }

        Ok(invoices)
    }

    /// Kimliğine göre tek bir faturayı okur.
    pub fn invoice_get(&self, id: i64) -> Result<Option<Invoice>> {
        let invoice = self
            .connection
            .query_row(
                &format!(
                    "SELECT {INVOICE_COLUMNS}
                     FROM invoices i
                     JOIN customers c ON c.id = i.customer_id
                     WHERE i.id = ?1"
                ),
                [id],
                row_to_invoice,
            )
            .optional()?;
        Ok(invoice)
    }

    /// Bir faturanın satırlarını satır numarasına göre sıralı döndürür.
    pub fn invoice_lines(&self, invoice_id: i64) -> Result<Vec<InvoiceLine>> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {INVOICE_LINE_COLUMNS}
             FROM invoice_lines
             WHERE invoice_id = ?1
             ORDER BY line_no"
        ))?;

        let rows = statement.query_map([invoice_id], row_to_invoice_line)?;

        let mut lines = Vec::new();
        for row in rows {
            lines.push(row?);
        }
        Ok(lines)
    }

    /// Toplam fatura sayısı (iptaller dâhil).
    pub fn invoice_count(&self) -> Result<usize> {
        let count: i64 = self
            .connection
            .query_row("SELECT COUNT(*) FROM invoices", [], |row| row.get(0))?;
        Ok(count.max(0) as usize)
    }

    /// Fatura numarası kullanımda mı? (büyük/küçük harf duyarsız)
    pub fn invoice_number_exists(&self, number: &str, exclude_id: Option<i64>) -> Result<bool> {
        let exists: bool = self.connection.query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM invoices
                 WHERE invoice_number = ?1 COLLATE NOCASE AND (?2 IS NULL OR id <> ?2)
             )",
            params![number, exclude_id],
            |row| row.get(0),
        )?;
        Ok(exists)
    }

    /// Kullanılmamış bir fatura numarası önerir (`SF-2026-0001`, ...).
    pub fn invoice_next_number(&self, invoice_type: InvoiceType, year: i32) -> Result<String> {
        let mut statement = self
            .connection
            .prepare("SELECT invoice_number FROM invoices")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;

        let mut numbers = Vec::new();
        for row in rows {
            numbers.push(row?);
        }

        Ok(suggest_next_number(&numbers, invoice_type, year))
    }

    /// Yeni fatura ekler; satırları, stok hareketlerini ve güncel stoğu **aynı
    /// işlem içinde** yazar ve oluşturulan faturanın kimliğini döndürür.
    ///
    /// Satırların `line_no` değeri veriliş sırasına göre 1'den başlar. Her
    /// satır için bir stok hareketi üretilir (alış: artı, satış: eksi yön) ve
    /// ilgili ürünün `current_stock` değeri güncellenir.
    pub fn invoice_insert(&self, invoice: &ValidatedInvoice) -> Result<i64> {
        let transaction = self.connection.unchecked_transaction()?;

        transaction
            .execute(
                "INSERT INTO invoices
                     (invoice_number, invoice_type, customer_id, invoice_date, due_date, notes,
                      subtotal, discount_total, net_total, vat_total, grand_total, status)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 'kesin')",
                params![
                    invoice.number,
                    invoice.invoice_type.as_db(),
                    invoice.customer_id,
                    invoice.invoice_date,
                    invoice.due_date.as_deref(),
                    invoice.notes.as_deref(),
                    invoice.totals.subtotal.kurus(),
                    invoice.totals.discount_total.kurus(),
                    invoice.totals.net_total.kurus(),
                    invoice.totals.vat_total.kurus(),
                    invoice.totals.grand_total.kurus(),
                ],
            )
            .map_err(|error| map_invoice_write_error(error, &invoice.number))?;

        let invoice_id = transaction.last_insert_rowid();

        let movement_type = invoice.invoice_type.movement_type();
        let sign = invoice.invoice_type.stock_sign();
        let movement_description = format!(
            "{} faturası: {}",
            invoice.invoice_type.label(),
            invoice.number
        );

        for (index, line) in invoice.lines.iter().enumerate() {
            let line_no = index as i64 + 1;

            transaction.execute(
                "INSERT INTO invoice_lines
                     (invoice_id, product_id, line_no, product_code, product_name, unit,
                      description, quantity, unit_price, discount_rate, vat_rate, line_gross,
                      line_discount, line_net, line_vat, line_total)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
                params![
                    invoice_id,
                    line.product_id,
                    line_no,
                    line.product_code,
                    line.product_name,
                    line.unit.as_db(),
                    line.description.as_deref(),
                    line.quantity.milli(),
                    line.unit_price.kurus(),
                    line.discount_rate,
                    line.vat_rate,
                    line.amounts.gross.kurus(),
                    line.amounts.discount.kurus(),
                    line.amounts.net.kurus(),
                    line.amounts.vat.kurus(),
                    line.amounts.total.kurus(),
                ],
            )?;

            let delta = line
                .quantity
                .milli()
                .checked_mul(sign)
                .ok_or(AppError::QuantityOverflow)?;

            transaction.execute(
                "INSERT INTO product_stock_movements
                     (product_id, movement_type, quantity_delta, movement_date, description,
                      invoice_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    line.product_id,
                    movement_type.as_db(),
                    delta,
                    invoice.invoice_date,
                    movement_description,
                    invoice_id,
                ],
            )?;

            transaction.execute(
                "UPDATE products
                    SET current_stock = current_stock + ?2, updated_at = datetime('now')
                  WHERE id = ?1",
                params![line.product_id, delta],
            )?;
        }

        transaction.commit()?;
        Ok(invoice_id)
    }

    /// Kesinleşmiş bir faturayı iptal eder.
    ///
    /// Her satır için ters yönlü bir `duzeltme` stok hareketi üretilir, güncel
    /// stok geri alınır ve belge `iptal` olarak işaretlenir. Tüm işlemler tek
    /// bir transaction içindedir. Zaten iptal edilmiş bir fatura
    /// [`AppError::InvoiceAlreadyCancelled`] ile reddedilir. Faturaya bağlı
    /// tahsilat/ödeme varsa (`paid_amount > 0`) iptal
    /// [`AppError::InvoiceHasPayments`] ile reddedilir; önce ilgili ödemeler
    /// iptal edilmelidir.
    pub fn invoice_cancel(&self, id: i64, reason: &str) -> Result<()> {
        self.require_admin("fatura iptali")?;
        let transaction = self.connection.unchecked_transaction()?;

        let record: Option<(String, String, String, String, i64)> = transaction
            .query_row(
                "SELECT status, invoice_type, invoice_date, invoice_number, paid_amount
                   FROM invoices WHERE id = ?1",
                [id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .optional()?;

        let Some((status, invoice_type, invoice_date, invoice_number, paid_amount)) = record else {
            return Err(AppError::InvoiceNotFound(id));
        };

        if InvoiceStatus::from_db(&status) == Some(InvoiceStatus::Cancelled) {
            return Err(AppError::InvoiceAlreadyCancelled(id));
        }

        if paid_amount > 0 {
            return Err(AppError::InvoiceHasPayments(id));
        }

        let invoice_type = InvoiceType::from_db(&invoice_type).ok_or_else(|| {
            AppError::InvalidRecord(format!("bilinmeyen fatura türü: '{invoice_type}'"))
        })?;

        let movement_description = format!("Fatura iptali: {invoice_number}");

        // İptal, kesinleşmedeki stok etkisinin tersini uygular.
        let reverse_sign = -invoice_type.stock_sign();

        {
            let mut statement = transaction
                .prepare("SELECT product_id, quantity FROM invoice_lines WHERE invoice_id = ?1")?;
            let rows = statement.query_map([id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
            })?;

            let mut offsets = Vec::new();
            for row in rows {
                offsets.push(row?);
            }
            drop(statement);

            for (product_id, quantity_milli) in offsets {
                let reverse = quantity_milli
                    .checked_mul(reverse_sign)
                    .ok_or(AppError::QuantityOverflow)?;

                transaction.execute(
                    "INSERT INTO product_stock_movements
                         (product_id, movement_type, quantity_delta, movement_date, description,
                          invoice_id)
                     VALUES (?1, 'duzeltme', ?2, ?3, ?4, ?5)",
                    params![product_id, reverse, invoice_date, movement_description, id,],
                )?;

                transaction.execute(
                    "UPDATE products
                        SET current_stock = current_stock + ?2, updated_at = datetime('now')
                      WHERE id = ?1",
                    params![product_id, reverse],
                )?;
            }
        }

        transaction.execute(
            "UPDATE invoices
                SET status = 'iptal',
                    cancelled_at = datetime('now'),
                    cancel_reason = ?2,
                    updated_at = datetime('now')
              WHERE id = ?1",
            params![id, reason],
        )?;

        transaction.commit()?;
        Ok(())
    }

    /// Bir cariye bağlı faturaları (en yeni önce) döndürür.
    pub fn invoice_list_for_customer(&self, customer_id: i64) -> Result<Vec<Invoice>> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {INVOICE_COLUMNS}
             FROM invoices i
             JOIN customers c ON c.id = i.customer_id
             WHERE i.customer_id = ?1
             ORDER BY i.invoice_date DESC, i.id DESC"
        ))?;

        let rows = statement.query_map([customer_id], row_to_invoice)?;

        let mut invoices = Vec::new();
        for row in rows {
            invoices.push(row?);
        }
        Ok(invoices)
    }

    /// Son faturaları (en yeni önce) döndürür.
    pub fn recent_invoices(&self, limit: usize) -> Result<Vec<Invoice>> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {INVOICE_COLUMNS}
             FROM invoices i
             JOIN customers c ON c.id = i.customer_id
             ORDER BY i.invoice_date DESC, i.id DESC
             LIMIT ?1"
        ))?;

        let rows = statement.query_map([limit as i64], row_to_invoice)?;

        let mut invoices = Vec::new();
        for row in rows {
            invoices.push(row?);
        }
        Ok(invoices)
    }

    /// Kesinleşmiş satış faturalarının genel toplamı (kuruş).
    pub fn confirmed_sales_total(&self) -> Result<Money> {
        let total: i64 = self.connection.query_row(
            "SELECT COALESCE(SUM(grand_total), 0) FROM invoices
             WHERE invoice_type = 'satis' AND status = 'kesin'",
            [],
            |row| row.get(0),
        )?;
        Ok(Money::from_kurus(total))
    }

    /// Kesinleşmiş alış faturalarının genel toplamı (kuruş).
    pub fn confirmed_purchases_total(&self) -> Result<Money> {
        let total: i64 = self.connection.query_row(
            "SELECT COALESCE(SUM(grand_total), 0) FROM invoices
             WHERE invoice_type = 'alis' AND status = 'kesin'",
            [],
            |row| row.get(0),
        )?;
        Ok(Money::from_kurus(total))
    }
}

/// Bir veritabanı satırını [`Invoice`] tipine dönüştürür.
fn row_to_invoice(row: &Row<'_>) -> rusqlite::Result<Invoice> {
    let type_value: String = row.get(INVOICE_TYPE_INDEX)?;
    let invoice_type = InvoiceType::from_db(&type_value).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            INVOICE_TYPE_INDEX,
            Type::Text,
            Box::new(AppError::InvalidRecord(format!(
                "bilinmeyen fatura türü: '{type_value}'"
            ))),
        )
    })?;

    let status_value: String = row.get(INVOICE_STATUS_INDEX)?;
    let status = InvoiceStatus::from_db(&status_value).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            INVOICE_STATUS_INDEX,
            Type::Text,
            Box::new(AppError::InvalidRecord(format!(
                "bilinmeyen fatura durumu: '{status_value}'"
            ))),
        )
    })?;

    Ok(Invoice {
        id: row.get(0)?,
        number: row.get(1)?,
        invoice_type,
        customer_id: row.get(3)?,
        customer_code: row.get(4)?,
        customer_name: row.get(5)?,
        invoice_date: row.get(6)?,
        due_date: row.get(7)?,
        notes: row.get(8)?,
        totals: InvoiceTotals {
            subtotal: Money::from_kurus(row.get(9)?),
            discount_total: Money::from_kurus(row.get(10)?),
            net_total: Money::from_kurus(row.get(11)?),
            vat_total: Money::from_kurus(row.get(12)?),
            grand_total: Money::from_kurus(row.get(13)?),
        },
        status,
        cancelled_at: row.get(15)?,
        cancel_reason: row.get(16)?,
        created_at: row.get(17)?,
        updated_at: row.get(18)?,
        paid_amount: Money::from_kurus(row.get(19)?),
    })
}

/// Bir veritabanı satırını [`InvoiceLine`] tipine dönüştürür.
fn row_to_invoice_line(row: &Row<'_>) -> rusqlite::Result<InvoiceLine> {
    let unit_value: String = row.get(LINE_UNIT_INDEX)?;
    let unit = Unit::from_db(&unit_value).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            LINE_UNIT_INDEX,
            Type::Text,
            Box::new(AppError::InvalidRecord(format!(
                "bilinmeyen ölçü birimi: '{unit_value}'"
            ))),
        )
    })?;

    Ok(InvoiceLine {
        id: row.get(0)?,
        invoice_id: row.get(1)?,
        product_id: row.get(2)?,
        line_no: row.get(3)?,
        product_code: row.get(4)?,
        product_name: row.get(5)?,
        unit,
        description: row.get(7)?,
        quantity: Quantity::from_milli(row.get(8)?),
        unit_price: Money::from_kurus(row.get(9)?),
        discount_rate: row.get(10)?,
        vat_rate: row.get(11)?,
        amounts: LineAmounts {
            gross: Money::from_kurus(row.get(12)?),
            discount: Money::from_kurus(row.get(13)?),
            net: Money::from_kurus(row.get(14)?),
            vat: Money::from_kurus(row.get(15)?),
            total: Money::from_kurus(row.get(16)?),
        },
    })
}

/// Yazma hatalarını kullanıcıya anlamlı gelen hatalara dönüştürür.
///
/// Fatura numarası benzersizlik ihlali ayrıca ele alınır; diğer kısıt
/// ihlalleri ve tüm diğer SQLite hataları genel veritabanı hatası olarak döner.
fn map_invoice_write_error(error: rusqlite::Error, number: &str) -> AppError {
    if let rusqlite::Error::SqliteFailure(inner, message) = &error
        && inner.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
    {
        let text = message.as_deref().unwrap_or_default();
        if text.contains("invoice_number") {
            return AppError::InvoiceNumberTaken(number.to_owned());
        }
    }

    AppError::Database(crate::error::sqlite_error_to_turkish(&error.to_string()))
}
