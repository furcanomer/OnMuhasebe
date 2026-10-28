// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Tahsilat/ödeme veri erişimi.
//!
//! SQL ifadeleri yalnızca bu modülde bulunur; arayüz katmanı [`Database`]
//! üzerindeki metotları çağırır ve hiçbir zaman SQL yazmaz.
//!
//! ## Güvenlik ve bütünlük
//!
//! * Tüm kullanıcı girdileri **parametreli** sorgularla iletilir.
//! * Yazma metotları yalnızca [`ValidatedPayment`] kabul eder.
//! * Kayıt kesinleşirken ödeme, dağıtımlar, fatura ödenen tutarları ve
//!   kasa/banka bakiyesi **tek bir işlem (transaction) içinde** yazılır.
//!   Herhangi bir adım başarısız olursa hiçbir kayıt kalmaz.
//! * Kayıt **silinmez/düzenlenmez**: iptal `status` ile işaretlenir ve etkisi
//!   (fatura ödenen tutarları, kasa bakiyesi) geri alınır.

use rusqlite::types::Type;
use rusqlite::{OptionalExtension, Row, params};

use crate::error::{AppError, Result};
use crate::money::Money;
use crate::payment::{
    InvoicePaymentLink, Payment, PaymentAllocation, PaymentFilter, PaymentStatus, PaymentType,
    ValidatedPayment,
};

use super::Database;

/// Tahsilat/ödeme sorgularında kullanılan kolon listesi (sırası
/// [`row_to_payment`] ile aynıdır).
///
/// Son iki kolon, `payment_allocations` tablosundan **hesaplanan** alanlardır:
/// dağıtılan toplam tutar ve dağıtım sayısı. Böylece avans bilgisi tek
/// sorguda okunur.
const PAYMENT_COLUMNS: &str = "p.id, p.payment_type, p.customer_id, c.code, c.name, \
                               p.cash_account_id, a.name, p.payment_date, p.amount, \
                               p.description, p.status, p.cancelled_at, p.cancel_reason, \
                               p.created_at, p.updated_at, \
                               COALESCE((SELECT SUM(al.amount) FROM payment_allocations al \
                                   WHERE al.payment_id = p.id), 0), \
                               (SELECT COUNT(*) FROM payment_allocations al \
                                   WHERE al.payment_id = p.id)";

/// Tür kolonunun indeksi.
const PAYMENT_TYPE_INDEX: usize = 1;
/// Durum kolonunun indeksi.
const PAYMENT_STATUS_INDEX: usize = 10;

impl Database {
    /// Ölçütlere uyan tahsilat/ödemeleri (en yeni önce) döndürür.
    ///
    /// Tür, durum, cari, kasa ve tarih ölçütleri SQL'de (parametreli)
    /// uygulanır. Serbest metin araması Rust tarafında yapılır.
    pub fn payment_list(&self, filter: &PaymentFilter) -> Result<Vec<Payment>> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {PAYMENT_COLUMNS}
             FROM payments p
             JOIN customers c ON c.id = p.customer_id
             JOIN cash_accounts a ON a.id = p.cash_account_id
             WHERE (?1 IS NULL OR p.payment_type = ?1)
               AND (?2 IS NULL OR p.status = ?2)
               AND (?3 IS NULL OR p.customer_id = ?3)
               AND (?4 IS NULL OR p.cash_account_id = ?4)
               AND (?5 = '' OR p.payment_date >= ?5)
               AND (?6 = '' OR p.payment_date <= ?6)
             ORDER BY p.payment_date DESC, p.id DESC"
        ))?;

        let payment_type = filter.payment_type.map(PaymentType::as_db);
        let status = filter.status.map(PaymentStatus::as_db);
        let date_from = filter.date_from.trim();
        let date_to = filter.date_to.trim();

        let rows = statement.query_map(
            params![
                payment_type,
                status,
                filter.customer_id,
                filter.cash_account_id,
                date_from,
                date_to
            ],
            row_to_payment,
        )?;

        let mut payments = Vec::new();
        for row in rows {
            payments.push(row?);
        }

        let needle = crate::customer::normalize_for_search(filter.search.trim());
        if !needle.is_empty() {
            payments.retain(|payment| payment.matches_search(&needle));
        }

        Ok(payments)
    }

    /// Kimliğine göre tek bir tahsilat/ödeme okur.
    pub fn payment_get(&self, id: i64) -> Result<Option<Payment>> {
        let payment = self
            .connection
            .query_row(
                &format!(
                    "SELECT {PAYMENT_COLUMNS}
                     FROM payments p
                     JOIN customers c ON c.id = p.customer_id
                     JOIN cash_accounts a ON a.id = p.cash_account_id
                     WHERE p.id = ?1"
                ),
                [id],
                row_to_payment,
            )
            .optional()?;
        Ok(payment)
    }

    /// Toplam tahsilat/ödeme kaydı sayısı (iptaller dâhil).
    pub fn payment_count(&self) -> Result<usize> {
        let count: i64 = self
            .connection
            .query_row("SELECT COUNT(*) FROM payments", [], |row| row.get(0))?;
        Ok(count.max(0) as usize)
    }

    /// Bir cariye bağlı tahsilat/ödemeleri (en yeni önce) döndürür.
    pub fn payment_list_for_customer(&self, customer_id: i64) -> Result<Vec<Payment>> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {PAYMENT_COLUMNS}
             FROM payments p
             JOIN customers c ON c.id = p.customer_id
             JOIN cash_accounts a ON a.id = p.cash_account_id
             WHERE p.customer_id = ?1
             ORDER BY p.payment_date DESC, p.id DESC"
        ))?;

        let rows = statement.query_map([customer_id], row_to_payment)?;

        let mut payments = Vec::new();
        for row in rows {
            payments.push(row?);
        }
        Ok(payments)
    }

    /// Son tahsilat/ödemeleri (en yeni önce) döndürür.
    pub fn recent_payments(&self, limit: usize) -> Result<Vec<Payment>> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {PAYMENT_COLUMNS}
             FROM payments p
             JOIN customers c ON c.id = p.customer_id
             JOIN cash_accounts a ON a.id = p.cash_account_id
             ORDER BY p.payment_date DESC, p.id DESC
             LIMIT ?1"
        ))?;

        let rows = statement.query_map([limit as i64], row_to_payment)?;

        let mut payments = Vec::new();
        for row in rows {
            payments.push(row?);
        }
        Ok(payments)
    }

    /// Bir tahsilat/ödemenin fatura dağıtımlarını döndürür.
    pub fn payment_allocations(&self, payment_id: i64) -> Result<Vec<PaymentAllocation>> {
        let mut statement = self.connection.prepare(
            "SELECT al.id, al.payment_id, al.invoice_id, i.invoice_number, i.invoice_date, \
                    i.grand_total, al.amount
               FROM payment_allocations al
               JOIN invoices i ON i.id = al.invoice_id
              WHERE al.payment_id = ?1
              ORDER BY al.id",
        )?;

        let rows = statement.query_map([payment_id], |row| {
            Ok(PaymentAllocation {
                id: row.get(0)?,
                payment_id: row.get(1)?,
                invoice_id: row.get(2)?,
                invoice_number: row.get(3)?,
                invoice_date: row.get(4)?,
                invoice_grand_total: Money::from_kurus(row.get(5)?),
                amount: Money::from_kurus(row.get(6)?),
            })
        })?;

        let mut allocations = Vec::new();
        for row in rows {
            allocations.push(row?);
        }
        Ok(allocations)
    }

    /// Bir faturaya bağlı tahsilat/ödeme bağlantılarını döndürür.
    pub fn payment_links_for_invoice(&self, invoice_id: i64) -> Result<Vec<InvoicePaymentLink>> {
        let mut statement = self.connection.prepare(
            "SELECT p.id, p.payment_type, p.payment_date, a.name, al.amount, p.status
               FROM payment_allocations al
               JOIN payments p ON p.id = al.payment_id
               JOIN cash_accounts a ON a.id = p.cash_account_id
              WHERE al.invoice_id = ?1
              ORDER BY p.payment_date DESC, p.id DESC",
        )?;

        let rows = statement.query_map([invoice_id], |row| {
            let type_value: String = row.get(1)?;
            let payment_type = PaymentType::from_db(&type_value).ok_or_else(|| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    Type::Text,
                    Box::new(AppError::InvalidRecord(format!(
                        "bilinmeyen ödeme türü: '{type_value}'"
                    ))),
                )
            })?;
            let status_value: String = row.get(5)?;
            let status = PaymentStatus::from_db(&status_value).ok_or_else(|| {
                rusqlite::Error::FromSqlConversionFailure(
                    5,
                    Type::Text,
                    Box::new(AppError::InvalidRecord(format!(
                        "bilinmeyen ödeme durumu: '{status_value}'"
                    ))),
                )
            })?;
            Ok(InvoicePaymentLink {
                payment_id: row.get(0)?,
                payment_type,
                payment_date: row.get(2)?,
                cash_account_name: row.get(3)?,
                amount: Money::from_kurus(row.get(4)?),
                status,
            })
        })?;

        let mut links = Vec::new();
        for row in rows {
            links.push(row?);
        }
        Ok(links)
    }

    /// Tahsilat/ödemeyi kesinleştirir ve kimliğini döndürür.
    ///
    /// Tüm adımlar tek bir transaction içindedir: ödeme kaydı, dağıtımlar,
    /// fatura `paid_amount` güncellemeleri ve kasa/banka bakiyesi.
    pub fn payment_insert(&self, payment: &ValidatedPayment) -> Result<i64> {
        let transaction = self.connection.unchecked_transaction()?;

        // Kasa/banka hesabı ve cari mevcut olmalı.
        let account_exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM cash_accounts WHERE id = ?1)",
            [payment.cash_account_id],
            |row| row.get(0),
        )?;
        if !account_exists {
            return Err(AppError::CashAccountNotFound(payment.cash_account_id));
        }

        transaction.execute(
            "INSERT INTO payments
                 (payment_type, customer_id, cash_account_id, payment_date, amount,
                  description, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'kesin')",
            params![
                payment.payment_type.as_db(),
                payment.customer_id,
                payment.cash_account_id,
                payment.payment_date,
                payment.amount.kurus(),
                payment.description.as_deref(),
            ],
        )?;
        let payment_id = transaction.last_insert_rowid();

        let mut allocated_total: i128 = 0;

        for allocation in &payment.allocations {
            if allocation.amount.kurus() <= 0 {
                return Err(AppError::PaymentAmountNotPositive);
            }

            let record: Option<(i64, String, String, i64, i64)> = transaction
                .query_row(
                    "SELECT customer_id, invoice_type, status, grand_total, paid_amount
                       FROM invoices WHERE id = ?1",
                    [allocation.invoice_id],
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

            let Some((customer_id, invoice_type, status, grand_total, paid_amount)) = record else {
                return Err(AppError::InvoiceNotFound(allocation.invoice_id));
            };

            if customer_id != payment.customer_id {
                return Err(AppError::AllocationCustomerMismatch(allocation.invoice_id));
            }
            if PaymentStatus::from_db(&status) != Some(PaymentStatus::Kesin) {
                return Err(AppError::AllocationToCancelledInvoice(
                    allocation.invoice_id,
                ));
            }
            if invoice_type != payment.payment_type.invoice_type().as_db() {
                return Err(AppError::AllocationTypeMismatch(allocation.invoice_id));
            }

            let remaining = i128::from(grand_total) - i128::from(paid_amount);
            if i128::from(allocation.amount.kurus()) > remaining {
                return Err(AppError::AllocationExceedsRemaining(allocation.invoice_id));
            }

            transaction.execute(
                "INSERT INTO payment_allocations (payment_id, invoice_id, amount)
                 VALUES (?1, ?2, ?3)",
                params![payment_id, allocation.invoice_id, allocation.amount.kurus()],
            )?;

            transaction.execute(
                "UPDATE invoices
                    SET paid_amount = paid_amount + ?2, updated_at = datetime('now')
                  WHERE id = ?1",
                params![allocation.invoice_id, allocation.amount.kurus()],
            )?;

            allocated_total += i128::from(allocation.amount.kurus());
        }

        if allocated_total > i128::from(payment.amount.kurus()) {
            return Err(AppError::PaymentAmountNotPositive);
        }

        // Kasa/banka bakiyesi: tahsilat `+`, ödeme `-`.
        let delta = payment.payment_type.cash_sign() * payment.amount.kurus();
        transaction.execute(
            "UPDATE cash_accounts
                SET current_balance = current_balance + ?2, updated_at = datetime('now')
              WHERE id = ?1",
            params![payment.cash_account_id, delta],
        )?;

        transaction.commit()?;
        Ok(payment_id)
    }

    /// Kesinleşmiş bir tahsilat/ödemeyi iptal eder.
    ///
    /// Dağıtımların fatura `paid_amount` etkisi geri alınır, kasa/banka
    /// bakiyesi tersine çevrilir ve kayıt `iptal` olarak işaretlenir.
    /// Faturalardan biri ödenen tutarı negatife düşürecekse işlem reddedilir.
    pub fn payment_cancel(&self, id: i64, reason: &str) -> Result<()> {
        self.require_admin("ödeme/tahsilat iptali")?;
        let transaction = self.connection.unchecked_transaction()?;

        let record: Option<(String, i64, String, i64)> = transaction
            .query_row(
                "SELECT status, amount, payment_type, cash_account_id
                   FROM payments WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;

        let Some((status, amount, payment_type, cash_account_id)) = record else {
            return Err(AppError::PaymentNotFound(id));
        };

        if PaymentStatus::from_db(&status) == Some(PaymentStatus::Iptal) {
            return Err(AppError::PaymentAlreadyCancelled(id));
        }

        let payment_type = PaymentType::from_db(&payment_type).ok_or_else(|| {
            AppError::InvalidRecord(format!("bilinmeyen ödeme türü: '{payment_type}'"))
        })?;

        let mut allocations = Vec::new();
        {
            let mut statement = transaction.prepare(
                "SELECT invoice_id, amount FROM payment_allocations WHERE payment_id = ?1",
            )?;
            let rows = statement.query_map([id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
            })?;
            for row in rows {
                allocations.push(row?);
            }
        }

        for (invoice_id, allocation_amount) in allocations {
            let paid: Option<i64> = transaction
                .query_row(
                    "SELECT paid_amount FROM invoices WHERE id = ?1",
                    [invoice_id],
                    |row| row.get(0),
                )
                .optional()?;

            let Some(paid) = paid else {
                return Err(AppError::InvoiceNotFound(invoice_id));
            };

            if paid < allocation_amount {
                return Err(AppError::InvoicePaidAmountUnderflow(invoice_id));
            }

            transaction.execute(
                "UPDATE invoices
                    SET paid_amount = paid_amount - ?2, updated_at = datetime('now')
                  WHERE id = ?1",
                params![invoice_id, allocation_amount],
            )?;
        }

        // Kasa/banka bakiyesini tersine çevir.
        let delta = -payment_type.cash_sign() * amount;
        transaction.execute(
            "UPDATE cash_accounts
                SET current_balance = current_balance + ?2, updated_at = datetime('now')
              WHERE id = ?1",
            params![cash_account_id, delta],
        )?;

        transaction.execute(
            "UPDATE payments
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
}

/// Bir veritabanı satırını [`Payment`] tipine dönüştürür.
fn row_to_payment(row: &Row<'_>) -> rusqlite::Result<Payment> {
    let type_value: String = row.get(PAYMENT_TYPE_INDEX)?;
    let payment_type = PaymentType::from_db(&type_value).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            PAYMENT_TYPE_INDEX,
            Type::Text,
            Box::new(AppError::InvalidRecord(format!(
                "bilinmeyen ödeme türü: '{type_value}'"
            ))),
        )
    })?;

    let status_value: String = row.get(PAYMENT_STATUS_INDEX)?;
    let status = PaymentStatus::from_db(&status_value).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            PAYMENT_STATUS_INDEX,
            Type::Text,
            Box::new(AppError::InvalidRecord(format!(
                "bilinmeyen ödeme durumu: '{status_value}'"
            ))),
        )
    })?;

    Ok(Payment {
        id: row.get(0)?,
        payment_type,
        customer_id: row.get(2)?,
        customer_code: row.get(3)?,
        customer_name: row.get(4)?,
        cash_account_id: row.get(5)?,
        cash_account_name: row.get(6)?,
        payment_date: row.get(7)?,
        amount: Money::from_kurus(row.get(8)?),
        description: row.get(9)?,
        status,
        cancelled_at: row.get(11)?,
        cancel_reason: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
        allocated_total: Money::from_kurus(row.get(15)?),
        allocation_count: row.get::<_, i64>(16)?.max(0) as usize,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cash::CashAccountDraft;
    use crate::customer::{CustomerDraft, CustomerType};
    use crate::income_expense::{EntryType, ValidatedCategory, ValidatedEntry};
    use crate::invoice::{InvoiceDraft, InvoiceType};
    use crate::product::ProductDraft;

    fn database() -> Database {
        Database::open_in_memory().expect("bellek içi veritabanı açılmalı")
    }

    fn customer(database: &Database, code: &str, kind: CustomerType) -> i64 {
        let draft = CustomerDraft {
            code: code.to_owned(),
            name: format!("Cari {code}"),
            kind,
            ..CustomerDraft::default()
        };
        database
            .customer_insert(&draft.validate().expect("cari geçerli"))
            .expect("cari eklenmeli")
    }

    fn cash_account(database: &Database, name: &str) -> i64 {
        let draft = CashAccountDraft {
            name: name.to_owned(),
            ..CashAccountDraft::default()
        };
        database
            .cash_account_insert(&draft.validate().expect("hesap geçerli"))
            .expect("hesap eklenmeli")
    }

    /// Tek satırlı, KDV'siz bir fatura ekler ve kimliğini döndürür.
    fn invoice(
        database: &Database,
        number: &str,
        invoice_type: InvoiceType,
        customer_id: i64,
        grand_total: Money,
    ) -> i64 {
        let product_draft = ProductDraft {
            code: format!("URN-{number}"),
            name: "Deneme".to_owned(),
            vat_rate: "0".to_owned(),
            ..ProductDraft::default()
        };
        let product_id = database
            .product_insert(&product_draft.validate().expect("ürün geçerli"))
            .expect("ürün eklenmeli");

        let mut draft = InvoiceDraft::new(invoice_type);
        draft.number = number.to_owned();
        draft.customer_id = Some(customer_id);
        draft.invoice_date = "2026-10-18".to_owned();
        let line = &mut draft.lines[0];
        line.product_id = Some(product_id);
        line.product_code = format!("URN-{number}");
        line.product_name = "Deneme".to_owned();
        line.quantity = "1".to_owned();
        line.unit_price = grand_total.format_turkish();
        line.discount_rate = "0".to_owned();
        line.vat_rate = "0".to_owned();

        let validated = draft.validate().expect("fatura doğrulanmalı");
        database
            .invoice_insert(&validated)
            .expect("fatura eklenmeli")
    }

    fn payment(
        payment_type: PaymentType,
        customer_id: i64,
        cash_account_id: i64,
        amount: Money,
        allocations: Vec<(i64, Money)>,
    ) -> ValidatedPayment {
        ValidatedPayment {
            payment_type,
            customer_id,
            cash_account_id,
            payment_date: "2026-10-18".to_owned(),
            amount,
            description: None,
            allocations: allocations
                .into_iter()
                .map(|(invoice_id, amount)| crate::payment::ValidatedAllocation {
                    invoice_id,
                    amount,
                })
                .collect(),
        }
    }

    /// Gelir/gider kategorisi ekler ve kimliğini döndürür.
    fn expense_category(database: &Database, name: &str, category_type: EntryType) -> i64 {
        database
            .expense_category_insert(&ValidatedCategory {
                name: name.to_owned(),
                category_type,
                description: None,
                active: true,
            })
            .expect("kategori eklenmeli")
    }

    /// Gelir/gider hareketi ekler ve kimliğini döndürür.
    fn income_expense_entry(
        database: &Database,
        entry_type: EntryType,
        category_id: i64,
        cash_account_id: i64,
        amount: Money,
    ) -> i64 {
        database
            .income_expense_insert(&ValidatedEntry {
                entry_type,
                category_id,
                cash_account_id,
                entry_date: "2026-10-18".to_owned(),
                amount,
                description: None,
                document_ref: None,
            })
            .expect("gelir/gider eklenmeli")
    }

    #[test]
    fn tahsilat_kesinlesir_kasa_ve_fatura_guncellenir() {
        let database = database();
        let customer_id = customer(&database, "CAR-0001", CustomerType::Customer);
        let account_id = cash_account(&database, "Merkez Kasa");
        let invoice_id = invoice(
            &database,
            "SF-2026-0001",
            InvoiceType::Sales,
            customer_id,
            Money::from_kurus(250_000),
        );

        let id = database
            .payment_insert(&payment(
                PaymentType::Tahsilat,
                customer_id,
                account_id,
                Money::from_kurus(100_000),
                vec![(invoice_id, Money::from_kurus(100_000))],
            ))
            .expect("tahsilat kesinleşmeli");

        let stored = database.invoice_get(invoice_id).unwrap().unwrap();
        assert_eq!(stored.paid_amount, Money::from_kurus(100_000));
        assert_eq!(stored.remaining(), Money::from_kurus(150_000));
        assert_eq!(
            stored.payment_state(),
            Some(crate::invoice::PaymentState::Partial)
        );

        let account = database.cash_account_get(account_id).unwrap().unwrap();
        assert_eq!(account.current_balance, Money::from_kurus(100_000));

        let payment = database.payment_get(id).unwrap().unwrap();
        assert_eq!(payment.allocated_total, Money::from_kurus(100_000));
        assert_eq!(payment.remainder(), Money::ZERO);
    }

    #[test]
    fn odeme_kasa_bakiyesini_dusurur() {
        let database = database();
        let customer_id = customer(&database, "CAR-0001", CustomerType::Supplier);
        let account_id = cash_account(&database, "Merkez Kasa");
        let invoice_id = invoice(
            &database,
            "AF-2026-0001",
            InvoiceType::Purchase,
            customer_id,
            Money::from_kurus(50_000),
        );

        database
            .payment_insert(&payment(
                PaymentType::Odeme,
                customer_id,
                account_id,
                Money::from_kurus(30_000),
                vec![(invoice_id, Money::from_kurus(30_000))],
            ))
            .expect("ödeme kesinleşmeli");

        let account = database.cash_account_get(account_id).unwrap().unwrap();
        assert_eq!(account.current_balance, Money::from_kurus(-30_000));

        // Bağıntı: current == opening + tahsilat - ödeme
        assert_eq!(
            account.current_balance,
            Money::from_kurus(account.opening_balance.kurus() - 30_000)
        );
    }

    #[test]
    fn tam_avans_tahsilati_kabul_edilir() {
        let database = database();
        let customer_id = customer(&database, "CAR-0001", CustomerType::Customer);
        let account_id = cash_account(&database, "Merkez Kasa");

        let id = database
            .payment_insert(&payment(
                PaymentType::Tahsilat,
                customer_id,
                account_id,
                Money::from_kurus(40_000),
                vec![],
            ))
            .expect("avans tahsilatı kabul edilmeli");

        let stored = database.payment_get(id).unwrap().unwrap();
        assert!(stored.is_advance());
        assert_eq!(stored.remainder(), Money::from_kurus(40_000));
    }

    #[test]
    fn iptal_edilen_faturaya_dagitim_reddedilir() {
        let database = database();
        let customer_id = customer(&database, "CAR-0001", CustomerType::Customer);
        let account_id = cash_account(&database, "Merkez Kasa");
        let invoice_id = invoice(
            &database,
            "SF-2026-0001",
            InvoiceType::Sales,
            customer_id,
            Money::from_kurus(100_000),
        );
        database.invoice_cancel(invoice_id, "Deneme").unwrap();

        let error = database
            .payment_insert(&payment(
                PaymentType::Tahsilat,
                customer_id,
                account_id,
                Money::from_kurus(10_000),
                vec![(invoice_id, Money::from_kurus(10_000))],
            ))
            .expect_err("iptal fatura reddedilmeli");
        assert!(matches!(error, AppError::AllocationToCancelledInvoice(_)));

        // Hiçbir kayıt yazılmamalı.
        assert_eq!(database.payment_count().unwrap(), 0);
    }

    #[test]
    fn yanlis_cari_ve_tur_uyusmazligi_reddedilir() {
        let database = database();
        let owner = customer(&database, "CAR-0001", CustomerType::Customer);
        let other = customer(&database, "CAR-0002", CustomerType::Customer);
        let account_id = cash_account(&database, "Merkez Kasa");
        let sales = invoice(
            &database,
            "SF-2026-0001",
            InvoiceType::Sales,
            owner,
            Money::from_kurus(100_000),
        );

        // Yanlış cari.
        let error = database
            .payment_insert(&payment(
                PaymentType::Tahsilat,
                other,
                account_id,
                Money::from_kurus(10_000),
                vec![(sales, Money::from_kurus(10_000))],
            ))
            .expect_err("yanlış cari reddedilmeli");
        assert!(matches!(error, AppError::AllocationCustomerMismatch(_)));

        // Tür uyuşmazlığı: tahsilat -> satış beklenir, alış faturası verilemez.
        let purchase = invoice(
            &database,
            "AF-2026-0001",
            InvoiceType::Purchase,
            owner,
            Money::from_kurus(100_000),
        );
        let error = database
            .payment_insert(&payment(
                PaymentType::Tahsilat,
                owner,
                account_id,
                Money::from_kurus(10_000),
                vec![(purchase, Money::from_kurus(10_000))],
            ))
            .expect_err("tür uyuşmazlığı reddedilmeli");
        assert!(matches!(error, AppError::AllocationTypeMismatch(_)));
    }

    #[test]
    fn fatura_kalanini_asan_dagitim_reddedilir() {
        let database = database();
        let customer_id = customer(&database, "CAR-0001", CustomerType::Customer);
        let account_id = cash_account(&database, "Merkez Kasa");
        let invoice_id = invoice(
            &database,
            "SF-2026-0001",
            InvoiceType::Sales,
            customer_id,
            Money::from_kurus(100_000),
        );

        let error = database
            .payment_insert(&payment(
                PaymentType::Tahsilat,
                customer_id,
                account_id,
                Money::from_kurus(200_000),
                vec![(invoice_id, Money::from_kurus(150_000))],
            ))
            .expect_err("kalan aşan dağıtım reddedilmeli");
        assert!(matches!(error, AppError::AllocationExceedsRemaining(_)));
        assert_eq!(database.payment_count().unwrap(), 0);
        assert_eq!(
            database
                .cash_account_get(account_id)
                .unwrap()
                .unwrap()
                .current_balance,
            Money::ZERO
        );
    }

    #[test]
    fn iptal_fatura_odeneni_ve_kasayi_geri_alir() {
        let database = database();
        let customer_id = customer(&database, "CAR-0001", CustomerType::Customer);
        let account_id = cash_account(&database, "Merkez Kasa");
        let invoice_id = invoice(
            &database,
            "SF-2026-0001",
            InvoiceType::Sales,
            customer_id,
            Money::from_kurus(250_000),
        );

        let id = database
            .payment_insert(&payment(
                PaymentType::Tahsilat,
                customer_id,
                account_id,
                Money::from_kurus(100_000),
                vec![(invoice_id, Money::from_kurus(100_000))],
            ))
            .unwrap();

        database.payment_cancel(id, "Yanlış kayıt").unwrap();

        assert_eq!(
            database
                .invoice_get(invoice_id)
                .unwrap()
                .unwrap()
                .paid_amount,
            Money::ZERO
        );
        assert_eq!(
            database
                .cash_account_get(account_id)
                .unwrap()
                .unwrap()
                .current_balance,
            Money::ZERO
        );
        let stored = database.payment_get(id).unwrap().unwrap();
        assert_eq!(stored.status, PaymentStatus::Iptal);
        assert!(stored.cancelled_at.is_some());
        assert_eq!(stored.cancel_reason.as_deref(), Some("Yanlış kayıt"));

        // İkinci kez iptal edilemez.
        assert!(matches!(
            database.payment_cancel(id, "yine"),
            Err(AppError::PaymentAlreadyCancelled(_))
        ));
    }

    #[test]
    fn odenmis_fatura_iptal_edilemez() {
        let database = database();
        let customer_id = customer(&database, "CAR-0001", CustomerType::Customer);
        let account_id = cash_account(&database, "Merkez Kasa");
        let invoice_id = invoice(
            &database,
            "SF-2026-0001",
            InvoiceType::Sales,
            customer_id,
            Money::from_kurus(100_000),
        );
        database
            .payment_insert(&payment(
                PaymentType::Tahsilat,
                customer_id,
                account_id,
                Money::from_kurus(50_000),
                vec![(invoice_id, Money::from_kurus(50_000))],
            ))
            .unwrap();

        let error = database
            .invoice_cancel(invoice_id, "Deneme")
            .expect_err("ödenmiş fatura iptal edilememeli");
        assert!(matches!(error, AppError::InvoiceHasPayments(_)));
        assert_eq!(
            error.to_string(),
            "Bu faturaya bağlı tahsilat/ödeme bulunuyor. Önce ilgili ödemeleri iptal edin."
        );
    }

    #[test]
    fn bagintilar_hesaplarla_dogrudur() {
        let database = database();
        let customer_id = customer(&database, "CAR-0001", CustomerType::Customer);
        let account_id = cash_account(&database, "Merkez Kasa");
        let first = invoice(
            &database,
            "SF-2026-0001",
            InvoiceType::Sales,
            customer_id,
            Money::from_kurus(100_000),
        );
        let second = invoice(
            &database,
            "SF-2026-0002",
            InvoiceType::Sales,
            customer_id,
            Money::from_kurus(200_000),
        );

        // İki tahsilat, iki faturaya çapraz dağıtım.
        database
            .payment_insert(&payment(
                PaymentType::Tahsilat,
                customer_id,
                account_id,
                Money::from_kurus(120_000),
                vec![
                    (first, Money::from_kurus(60_000)),
                    (second, Money::from_kurus(60_000)),
                ],
            ))
            .unwrap();
        database
            .payment_insert(&payment(
                PaymentType::Tahsilat,
                customer_id,
                account_id,
                Money::from_kurus(50_000),
                vec![(first, Money::from_kurus(40_000))],
            ))
            .unwrap();

        // Faturaların ödenen tutarı dağıtımların toplamına eşit olmalı.
        let first_paid = database.invoice_get(first).unwrap().unwrap().paid_amount;
        assert_eq!(first_paid, Money::from_kurus(100_000));
        let second_paid = database.invoice_get(second).unwrap().unwrap().paid_amount;
        assert_eq!(second_paid, Money::from_kurus(60_000));

        // Gelir/gider kayıtları (biri iptal) cariye ve faturaya dokunmamalı.
        let income_category = expense_category(&database, "Kira Geliri", EntryType::Gelir);
        let expense_category_id = expense_category(&database, "Kira", EntryType::Gider);
        income_expense_entry(
            &database,
            EntryType::Gelir,
            income_category,
            account_id,
            Money::from_kurus(30_000),
        );
        income_expense_entry(
            &database,
            EntryType::Gider,
            expense_category_id,
            account_id,
            Money::from_kurus(10_000),
        );
        let cancelled = income_expense_entry(
            &database,
            EntryType::Gider,
            expense_category_id,
            account_id,
            Money::from_kurus(5_000),
        );
        database
            .income_expense_cancel(cancelled, "Yanlış kayıt")
            .unwrap();

        assert_eq!(
            database.invoice_get(first).unwrap().unwrap().paid_amount,
            first_paid,
            "gelir/gider fatura ödenen tutarını değiştirmemeli"
        );
        assert_eq!(
            database.invoice_get(second).unwrap().unwrap().paid_amount,
            second_paid
        );
        assert_eq!(
            database
                .customer_get(customer_id)
                .unwrap()
                .unwrap()
                .payment_count,
            2,
            "gelir/gider cari hareket sayısına girmez"
        );

        // Kasa bakiyesi: açılış + Σ kesin tahsilat - Σ kesin ödeme
        // + Σ kesin gelir - Σ kesin gider. İptal edilen gelir/gider katılmaz.
        let payments = database.payment_list(&PaymentFilter::default()).unwrap();
        let entries = database
            .income_expense_list(&crate::income_expense::EntryFilter::default())
            .unwrap();
        let account = database.cash_account_get(account_id).unwrap().unwrap();
        let mut expected = account.opening_balance.kurus();
        for payment in &payments {
            if payment.status != PaymentStatus::Kesin {
                continue;
            }
            expected += payment.amount.kurus() * payment.payment_type.cash_sign();
        }
        for entry in &entries {
            if entry.is_cancelled() {
                continue;
            }
            expected += entry.amount.kurus() * entry.entry_type.cash_sign();
        }
        assert_eq!(
            account.current_balance.kurus(),
            expected,
            "kasa bakiyesi kesin tahsilat/ödeme ve gelir/giderle tutarlı olmalı"
        );
        assert_eq!(
            expected,
            account.opening_balance.kurus() + (120_000 + 50_000) + 30_000 - 10_000
        );
    }

    #[test]
    fn ayrintida_dagitimlar_ve_fatura_baglantilari_okunur() {
        let database = database();
        let customer_id = customer(&database, "CAR-0001", CustomerType::Customer);
        let account_id = cash_account(&database, "Merkez Kasa");
        let invoice_id = invoice(
            &database,
            "SF-2026-0001",
            InvoiceType::Sales,
            customer_id,
            Money::from_kurus(100_000),
        );

        let id = database
            .payment_insert(&payment(
                PaymentType::Tahsilat,
                customer_id,
                account_id,
                Money::from_kurus(60_000),
                vec![(invoice_id, Money::from_kurus(60_000))],
            ))
            .unwrap();

        let allocations = database.payment_allocations(id).unwrap();
        assert_eq!(allocations.len(), 1);
        assert_eq!(allocations[0].invoice_number, "SF-2026-0001");
        assert_eq!(
            allocations[0].invoice_grand_total,
            Money::from_kurus(100_000)
        );
        assert_eq!(allocations[0].amount, Money::from_kurus(60_000));

        let links = database.payment_links_for_invoice(invoice_id).unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].payment_id, id);
        assert_eq!(links[0].amount, Money::from_kurus(60_000));
        assert_eq!(links[0].cash_account_name, "Merkez Kasa");
    }

    #[test]
    fn liste_filtre_ve_arama_calisir() {
        let database = database();
        let customer_id = customer(&database, "CAR-0001", CustomerType::Customer);
        let account_id = cash_account(&database, "Merkez Kasa");
        let other_account = cash_account(&database, "Ziraat TL");

        database
            .payment_insert(&payment(
                PaymentType::Tahsilat,
                customer_id,
                account_id,
                Money::from_kurus(10_000),
                vec![],
            ))
            .unwrap();
        let odeme = database
            .payment_insert(&payment(
                PaymentType::Odeme,
                customer_id,
                other_account,
                Money::from_kurus(20_000),
                vec![],
            ))
            .unwrap();
        database.payment_cancel(odeme, "iptal").unwrap();

        assert_eq!(database.payment_count().unwrap(), 2);

        let collections = database
            .payment_list(&PaymentFilter {
                payment_type: Some(PaymentType::Tahsilat),
                ..PaymentFilter::default()
            })
            .unwrap();
        assert_eq!(collections.len(), 1);

        let cancelled = database
            .payment_list(&PaymentFilter {
                status: Some(PaymentStatus::Iptal),
                ..PaymentFilter::default()
            })
            .unwrap();
        assert_eq!(cancelled.len(), 1);

        let by_account = database
            .payment_list(&PaymentFilter {
                cash_account_id: Some(other_account),
                ..PaymentFilter::default()
            })
            .unwrap();
        assert_eq!(by_account.len(), 1);

        let by_customer = database.payment_list_for_customer(customer_id).unwrap();
        assert_eq!(by_customer.len(), 2);
    }
}
