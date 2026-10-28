// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Gelir/gider kategorileri ve hareketleri veri erişimi.
//!
//! SQL ifadeleri yalnızca bu modülde bulunur; arayüz katmanı [`Database`]
//! üzerindeki metotları çağırır ve hiçbir zaman SQL yazmaz.
//!
//! ## Güvenlik ve bütünlük
//!
//! * Tüm kullanıcı girdileri **parametreli** sorgularla iletilir.
//! * Yazma metotları yalnızca doğrulanmış tipleri kabul eder.
//! * Kategori adı, türü içinde harf duyarsız benzersizdir; ihlal anlamlı bir
//!   hataya çevrilir.
//! * Kayıt kesinleşirken hareket ve kasa/banka bakiyesi **tek bir işlem
//!   (transaction) içinde** yazılır. Herhangi bir adım başarısız olursa hiçbir
//!   kayıt kalmaz.
//! * İptalde kasa/banka etkisi geri alınır ve durum `iptal` olur. Kayıt
//!   **silinmez/düzenlenmez**.

use rusqlite::types::Type;
use rusqlite::{OptionalExtension, Row, params};

use crate::error::{AppError, Result};
use crate::income_expense::{
    CategoryFilter, EntryFilter, EntryStatus, EntrySummary, EntryType, ExpenseCategory,
    IncomeExpenseEntry, ValidatedCategory, ValidatedEntry,
};
use crate::money::Money;

use super::Database;

/// Kategori sorgularında kullanılan kolon listesi (sırası
/// [`row_to_category`] ile aynıdır).
///
/// Son kolon, kategoriyi kullanan hareket sayısıdır (iptaller dahil). Kategori
/// silinemediği için bu sayı aynı zamanda "kaç kayıtta referans veriliyor?"
/// sorusunun cevabıdır.
const CATEGORY_COLUMNS: &str = "c.id, c.name, c.category_type, c.description, c.active, \
                                c.created_at, c.updated_at, \
                                (SELECT COUNT(*) FROM income_expense_entries e \
                                    WHERE e.category_id = c.id)";

/// Hareket sorgularında kullanılan kolon listesi (sırası
/// [`row_to_entry`] ile aynıdır).
const ENTRY_COLUMNS: &str = "e.id, e.entry_type, e.category_id, c.name, e.cash_account_id, \
                             a.name, e.entry_date, e.amount, e.description, e.document_ref, \
                             e.status, e.cancelled_at, e.cancel_reason, e.created_at, \
                             e.updated_at";

/// Hareket türü kolonunun indeksi.
const ENTRY_TYPE_INDEX: usize = 1;
/// Hareket durumu kolonunun indeksi.
const ENTRY_STATUS_INDEX: usize = 10;

impl Database {
    /// Ölçütlere uyan kategorileri ada göre sıralı döndürür.
    ///
    /// Tür ve durum ölçütleri SQL'de (parametreli) uygulanır. Serbest metin
    /// araması Türkçe karakterlerden etkilenmemesi için Rust tarafında yapılır.
    pub fn expense_category_list(&self, filter: &CategoryFilter) -> Result<Vec<ExpenseCategory>> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {CATEGORY_COLUMNS}
             FROM expense_categories c
             WHERE (?1 IS NULL OR c.category_type = ?1)
               AND (?2 IS NULL OR c.active = ?2)
             ORDER BY c.name COLLATE NOCASE, c.id"
        ))?;

        let category_type = filter.category_type.map(EntryType::as_db);
        let active = filter.active.map(i64::from);
        let needle = crate::customer::normalize_for_search(&filter.search);

        let rows = statement.query_map(params![category_type, active], row_to_category)?;
        let mut categories = Vec::new();
        for row in rows {
            let category = row?;
            if category.matches_search(&needle) {
                categories.push(category);
            }
        }
        Ok(categories)
    }

    /// Tek bir kategoriyi kimliğine göre döndürür.
    pub fn expense_category_get(&self, id: i64) -> Result<Option<ExpenseCategory>> {
        let category = self
            .connection
            .query_row(
                &format!("SELECT {CATEGORY_COLUMNS} FROM expense_categories c WHERE c.id = ?1"),
                [id],
                row_to_category,
            )
            .optional()?;
        Ok(category)
    }

    /// Toplam kategori sayısı (pasifler dahil).
    pub fn expense_category_count(&self) -> Result<usize> {
        let count: i64 =
            self.connection
                .query_row("SELECT COUNT(*) FROM expense_categories", [], |row| {
                    row.get(0)
                })?;
        Ok(count.max(0) as usize)
    }

    /// Aynı ad ve türde başka bir kategori var mı?
    ///
    /// `exclude_id` verilirse bu kayıt dışarıda bırakılır (düzenleme için).
    /// Karşılaştırma `name` kolonunun `COLLATE NOCASE` özelliği sayesinde
    /// büyük/küçük harf duyarsızdır.
    pub fn expense_category_name_exists(
        &self,
        name: &str,
        category_type: EntryType,
        exclude_id: Option<i64>,
    ) -> Result<bool> {
        let exists: bool = self.connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM expense_categories
                 WHERE name = ?1
                   AND category_type = ?2
                   AND (?3 IS NULL OR id <> ?3)
            )",
            params![name, category_type.as_db(), exclude_id],
            |row| row.get(0),
        )?;
        Ok(exists)
    }

    /// Yeni kategori ekler ve kimliğini döndürür.
    pub fn expense_category_insert(&self, category: &ValidatedCategory) -> Result<i64> {
        self.connection
            .execute(
                "INSERT INTO expense_categories (name, category_type, description, active)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    category.name,
                    category.category_type.as_db(),
                    category.description,
                    i64::from(category.active)
                ],
            )
            .map_err(|error| map_category_name_error(error, &category.name))?;
        Ok(self.connection.last_insert_rowid())
    }

    /// Var olan kategoriyi günceller.
    pub fn expense_category_update(&self, id: i64, category: &ValidatedCategory) -> Result<()> {
        let current = self
            .expense_category_get(id)?
            .ok_or(AppError::ExpenseCategoryNotFound(id))?;
        if !self.session_is_manager() && current.active != category.active {
            tracing::warn!(user_id = id, "yetki reddedildi: kategori durumu değiştirme");
            return Err(AppError::PermissionDenied {
                operation: "kategori durumunu değiştirme".to_owned(),
            });
        }
        let affected = self
            .connection
            .execute(
                "UPDATE expense_categories
                    SET name = ?2, category_type = ?3, description = ?4, active = ?5,
                        updated_at = datetime('now')
                  WHERE id = ?1",
                params![
                    id,
                    category.name,
                    category.category_type.as_db(),
                    category.description,
                    i64::from(category.active)
                ],
            )
            .map_err(|error| map_category_name_error(error, &category.name))?;
        if affected == 0 {
            return Err(AppError::ExpenseCategoryNotFound(id));
        }
        Ok(())
    }

    /// Kategoriyi pasife alır veya yeniden aktife alır.
    pub fn expense_category_set_active(&self, id: i64, active: bool) -> Result<()> {
        self.require_admin("kategori pasife alma/aktifleştirme")?;
        let affected = self.connection.execute(
            "UPDATE expense_categories
                SET active = ?2, updated_at = datetime('now')
              WHERE id = ?1",
            params![id, i64::from(active)],
        )?;
        if affected == 0 {
            return Err(AppError::ExpenseCategoryNotFound(id));
        }
        Ok(())
    }

    /// Ölçütlere uyan gelir/gider hareketlerini tarihe göre (yeniden eskiye)
    /// döndürür.
    pub fn income_expense_list(&self, filter: &EntryFilter) -> Result<Vec<IncomeExpenseEntry>> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {ENTRY_COLUMNS}
             FROM income_expense_entries e
             JOIN expense_categories c ON c.id = e.category_id
             JOIN cash_accounts a ON a.id = e.cash_account_id
             WHERE (?1 IS NULL OR e.entry_type = ?1)
               AND (?2 IS NULL OR e.category_id = ?2)
               AND (?3 IS NULL OR e.cash_account_id = ?3)
               AND (?4 IS NULL OR e.status = ?4)
               AND (?5 = '' OR e.entry_date >= ?5)
               AND (?6 = '' OR e.entry_date <= ?6)
             ORDER BY e.entry_date DESC, e.id DESC"
        ))?;

        let entry_type = filter.entry_type.map(EntryType::as_db);
        let status = filter.status.map(EntryStatus::as_db);
        let date_from = filter.date_from.trim();
        let date_to = filter.date_to.trim();
        let needle = crate::customer::normalize_for_search(&filter.search);

        let rows = statement.query_map(
            params![
                entry_type,
                filter.category_id,
                filter.cash_account_id,
                status,
                date_from,
                date_to
            ],
            row_to_entry,
        )?;

        let mut entries = Vec::new();
        for row in rows {
            let entry = row?;
            if entry.matches_search(&needle) {
                entries.push(entry);
            }
        }
        Ok(entries)
    }

    /// Tek bir gelir/gider hareketini kimliğine göre döndürür.
    pub fn income_expense_get(&self, id: i64) -> Result<Option<IncomeExpenseEntry>> {
        let entry = self
            .connection
            .query_row(
                &format!(
                    "SELECT {ENTRY_COLUMNS}
                     FROM income_expense_entries e
                     JOIN expense_categories c ON c.id = e.category_id
                     JOIN cash_accounts a ON a.id = e.cash_account_id
                     WHERE e.id = ?1"
                ),
                [id],
                row_to_entry,
            )
            .optional()?;
        Ok(entry)
    }

    /// Toplam gelir/gider hareketi sayısı (iptaller dahil).
    pub fn income_expense_count(&self) -> Result<usize> {
        let count: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM income_expense_entries",
            [],
            |row| row.get(0),
        )?;
        Ok(count.max(0) as usize)
    }

    /// Verilen tarih aralığında **herhangi bir** gelir/gider kaydı var mı?
    /// (iptaller dahil; boş tarih = sınır yok)
    pub fn income_expense_has_entries(&self, date_from: &str, date_to: &str) -> Result<bool> {
        let exists: bool = self.connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM income_expense_entries
                 WHERE (?1 = '' OR entry_date >= ?1)
                   AND (?2 = '' OR entry_date <= ?2)
            )",
            params![date_from.trim(), date_to.trim()],
            |row| row.get(0),
        )?;
        Ok(exists)
    }

    /// Verilen tarih aralığındaki kesin gelir/gider toplamları.
    pub fn income_expense_summary(&self, date_from: &str, date_to: &str) -> Result<EntrySummary> {
        let (income, expense): (i64, i64) = self.connection.query_row(
            "SELECT
                COALESCE(SUM(CASE WHEN entry_type = 'gelir' THEN amount ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN entry_type = 'gider' THEN amount ELSE 0 END), 0)
             FROM income_expense_entries
             WHERE status = 'kesin'
               AND (?1 = '' OR entry_date >= ?1)
               AND (?2 = '' OR entry_date <= ?2)",
            params![date_from.trim(), date_to.trim()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        Ok(EntrySummary {
            income_total: Money::from_kurus(income),
            expense_total: Money::from_kurus(expense),
        })
    }

    /// Yeni gelir/gider hareketini kesinleştirir.
    ///
    /// Hareket satırı ve kasa/banka bakiyesi **tek bir işlem içinde** yazılır.
    /// Kategori pasifse, türü uyuşmuyorsa veya kasa/banka hesabı pasifse
    /// kayıt yapılmaz. Kasa/banka bakiyesi eksiye düşebilir.
    pub fn income_expense_insert(&self, entry: &ValidatedEntry) -> Result<i64> {
        let transaction = self.connection.unchecked_transaction()?;

        let category: Option<(String, i64)> = transaction
            .query_row(
                "SELECT category_type, active FROM expense_categories WHERE id = ?1",
                [entry.category_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((category_type, active)) = category else {
            return Err(AppError::ExpenseCategoryNotFound(entry.category_id));
        };
        let category_type = EntryType::from_db(&category_type).ok_or_else(|| {
            AppError::InvalidRecord(format!("bilinmeyen kategori türü: '{category_type}'"))
        })?;
        if category_type != entry.entry_type {
            return Err(AppError::ExpenseCategoryTypeMismatch(entry.category_id));
        }
        if active == 0 {
            return Err(AppError::ExpenseCategoryInactive(entry.category_id));
        }

        let account_active: Option<i64> = transaction
            .query_row(
                "SELECT active FROM cash_accounts WHERE id = ?1",
                [entry.cash_account_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(account_active) = account_active else {
            return Err(AppError::CashAccountNotFound(entry.cash_account_id));
        };
        if account_active == 0 {
            return Err(AppError::CashAccountInactive(entry.cash_account_id));
        }

        transaction.execute(
            "INSERT INTO income_expense_entries
                (entry_type, category_id, cash_account_id, entry_date, amount,
                 description, document_ref, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'kesin')",
            params![
                entry.entry_type.as_db(),
                entry.category_id,
                entry.cash_account_id,
                entry.entry_date,
                entry.amount.kurus(),
                entry.description,
                entry.document_ref
            ],
        )?;
        let id = transaction.last_insert_rowid();

        let delta = entry
            .amount
            .kurus()
            .saturating_mul(entry.entry_type.cash_sign());
        transaction.execute(
            "UPDATE cash_accounts
                SET current_balance = current_balance + ?2, updated_at = datetime('now')
              WHERE id = ?1",
            params![entry.cash_account_id, delta],
        )?;

        transaction.commit()?;
        Ok(id)
    }

    /// Gelir/gider hareketini iptal eder.
    ///
    /// Kasa/banka bakiyesine olan etki geri alınır ve durum `iptal` olur.
    /// Kayıt silinmez. İşlem tek bir transaction içinde yapılır.
    pub fn income_expense_cancel(&self, id: i64, reason: &str) -> Result<()> {
        self.require_admin("gelir/gider kaydı iptali")?;
        let transaction = self.connection.unchecked_transaction()?;

        let record: Option<(String, i64, i64, String)> = transaction
            .query_row(
                "SELECT entry_type, cash_account_id, amount, status
                   FROM income_expense_entries
                  WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;
        let Some((entry_type, cash_account_id, amount, status)) = record else {
            return Err(AppError::IncomeExpenseNotFound(id));
        };
        if status == EntryStatus::Iptal.as_db() {
            return Err(AppError::IncomeExpenseAlreadyCancelled(id));
        }
        let entry_type = EntryType::from_db(&entry_type).ok_or_else(|| {
            AppError::InvalidRecord(format!("bilinmeyen hareket türü: '{entry_type}'"))
        })?;

        // İptal, kesinleşirken uygulanan etkinin tersini uygular.
        let reversal = entry_type
            .cash_sign()
            .saturating_mul(amount)
            .saturating_neg();
        transaction.execute(
            "UPDATE cash_accounts
                SET current_balance = current_balance + ?2, updated_at = datetime('now')
              WHERE id = ?1",
            params![cash_account_id, reversal],
        )?;

        transaction.execute(
            "UPDATE income_expense_entries
                SET status = 'iptal', cancelled_at = datetime('now'), cancel_reason = ?2,
                    updated_at = datetime('now')
              WHERE id = ?1",
            params![id, reason],
        )?;

        transaction.commit()?;
        Ok(())
    }
}

/// Bir veritabanı satırını [`ExpenseCategory`] tipine dönüştürür.
fn row_to_category(row: &Row<'_>) -> rusqlite::Result<ExpenseCategory> {
    let type_value: String = row.get(2)?;
    let category_type = EntryType::from_db(&type_value).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            2,
            Type::Text,
            Box::new(AppError::InvalidRecord(format!(
                "bilinmeyen kategori türü: '{type_value}'"
            ))),
        )
    })?;
    let active: i64 = row.get(4)?;
    let usage_count: i64 = row.get(7)?;

    Ok(ExpenseCategory {
        id: row.get(0)?,
        name: row.get(1)?,
        category_type,
        description: row.get(3)?,
        active: active != 0,
        usage_count: usage_count.max(0) as usize,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

/// Bir veritabanı satırını [`IncomeExpenseEntry`] tipine dönüştürür.
fn row_to_entry(row: &Row<'_>) -> rusqlite::Result<IncomeExpenseEntry> {
    let type_value: String = row.get(ENTRY_TYPE_INDEX)?;
    let entry_type = EntryType::from_db(&type_value).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            ENTRY_TYPE_INDEX,
            Type::Text,
            Box::new(AppError::InvalidRecord(format!(
                "bilinmeyen hareket türü: '{type_value}'"
            ))),
        )
    })?;
    let status_value: String = row.get(ENTRY_STATUS_INDEX)?;
    let status = EntryStatus::from_db(&status_value).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            ENTRY_STATUS_INDEX,
            Type::Text,
            Box::new(AppError::InvalidRecord(format!(
                "bilinmeyen hareket durumu: '{status_value}'"
            ))),
        )
    })?;

    Ok(IncomeExpenseEntry {
        id: row.get(0)?,
        entry_type,
        category_id: row.get(2)?,
        category_name: row.get(3)?,
        cash_account_id: row.get(4)?,
        cash_account_name: row.get(5)?,
        entry_date: row.get(6)?,
        amount: Money::from_kurus(row.get(7)?),
        description: row.get(8)?,
        document_ref: row.get(9)?,
        status,
        cancelled_at: row.get(11)?,
        cancel_reason: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

/// Kategori adı benzersizlik ihlalini kullanıcıya anlamlı hataya çevirir.
fn map_category_name_error(error: rusqlite::Error, name: &str) -> AppError {
    if let rusqlite::Error::SqliteFailure(inner, _) = &error
        && inner.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
    {
        return AppError::ExpenseCategoryNameTaken(name.to_owned());
    }
    AppError::Database(crate::error::sqlite_error_to_turkish(&error.to_string()))
}
