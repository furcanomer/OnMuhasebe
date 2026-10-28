// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Kasa/banka hesabı veri erişimi.
//!
//! SQL ifadeleri yalnızca bu modülde bulunur; arayüz katmanı [`Database`]
//! üzerindeki metotları çağırır ve hiçbir zaman SQL yazmaz.
//!
//! ## Güvenlik ve bütünlük
//!
//! * Tüm kullanıcı girdileri **parametreli** sorgularla (`?1`, `?2`, ...)
//!   iletilir.
//! * Yazma metotları yalnızca [`ValidatedCashAccount`] kabul eder.
//! * `name` alanındaki `UNIQUE` kısıtı veritabanı düzeyinde korunur.
//! * Kayıt **silinmez**; pasife alma için [`Database::cash_account_set_active`]
//!   kullanılır.
//! * Açılış bakiyesi yalnızca **hiç hareketi olmayan** hesaplarda
//!   güncellenebilir; aksi hâlde yok sayılır (güncel bakiye bozulmaz).

use rusqlite::{OptionalExtension, Row, params};

use crate::cash::{AccountType, CashAccount, CashAccountFilter, ValidatedCashAccount};
use crate::error::{AppError, Result};
use crate::money::Money;

use super::Database;

/// Kasa/banka sorgularında kullanılan kolon listesi (sırası
/// [`row_to_cash_account`] ile aynıdır).
///
/// Son iki kolon, `payments` ve `income_expense_entries` tablolarından
/// **hesaplanan** hareket sayılarıdır; böylece "açılış bakiyesi düzenlenebilir
/// mi?" bilgisi tek sorguda okunur.
const CASH_ACCOUNT_COLUMNS: &str = "a.id, a.name, a.account_type, a.bank_name, a.iban, \
                                    a.opening_balance, a.current_balance, a.notes, a.active, \
                                    a.created_at, a.updated_at, \
                                    (SELECT COUNT(*) FROM payments p \
                                        WHERE p.cash_account_id = a.id), \
                                    (SELECT COUNT(*) FROM income_expense_entries e \
                                        WHERE e.cash_account_id = a.id)";

/// Hesap türü kolonunun indeksi.
const ACCOUNT_TYPE_INDEX: usize = 2;

impl Database {
    /// Ölçütlere uyan kasa/banka hesaplarını ada göre sıralı döndürür.
    ///
    /// Tür ve durum ölçütleri SQL'de (parametreli) uygulanır. Serbest metin
    /// araması ise Türkçe karakterlerden etkilenmemesi için Rust tarafında
    /// yapılır.
    pub fn cash_account_list(&self, filter: &CashAccountFilter) -> Result<Vec<CashAccount>> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {CASH_ACCOUNT_COLUMNS}
             FROM cash_accounts a
             WHERE (?1 IS NULL OR a.account_type = ?1)
               AND (?2 IS NULL OR a.active = ?2)
             ORDER BY a.name COLLATE NOCASE"
        ))?;

        let account_type = filter.account_type.map(AccountType::as_db);
        let active = filter.active.map(i64::from);

        let rows = statement.query_map(params![account_type, active], row_to_cash_account)?;

        let mut accounts = Vec::new();
        for row in rows {
            accounts.push(row?);
        }

        let needle = crate::customer::normalize_for_search(filter.search.trim());
        if !needle.is_empty() {
            accounts.retain(|account| account.matches_search(&needle));
        }

        Ok(accounts)
    }

    /// Kimliğine göre tek bir kasa/banka hesabı okur.
    pub fn cash_account_get(&self, id: i64) -> Result<Option<CashAccount>> {
        let account = self
            .connection
            .query_row(
                &format!("SELECT {CASH_ACCOUNT_COLUMNS} FROM cash_accounts a WHERE a.id = ?1"),
                [id],
                row_to_cash_account,
            )
            .optional()?;
        Ok(account)
    }

    /// Toplam kasa/banka hesabı sayısı (pasif kayıtlar dâhil).
    pub fn cash_account_count(&self) -> Result<usize> {
        let count: i64 =
            self.connection
                .query_row("SELECT COUNT(*) FROM cash_accounts", [], |row| row.get(0))?;
        Ok(count.max(0) as usize)
    }

    /// Hesap adı kullanımda mı?
    ///
    /// `exclude_id` verilirse bu kayıt dışındaki kayıtlar denetlenir. Karşılaştırma
    /// büyük/küçük harf duyarsızdır; veritabanındaki `UNIQUE` kısıtı ile aynı
    /// kuralı uygular.
    pub fn cash_account_name_exists(&self, name: &str, exclude_id: Option<i64>) -> Result<bool> {
        let exists: bool = self.connection.query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM cash_accounts
                 WHERE name = ?1 COLLATE NOCASE AND (?2 IS NULL OR id <> ?2)
             )",
            params![name, exclude_id],
            |row| row.get(0),
        )?;
        Ok(exists)
    }

    /// Yeni kasa/banka hesabı ekler ve oluşturulan kaydın kimliğini döndürür.
    ///
    /// Açılış ve güncel bakiye başlangıçta eşittir (henüz hareket yoktur).
    pub fn cash_account_insert(&self, account: &ValidatedCashAccount) -> Result<i64> {
        let opening = account.opening_balance.kurus();
        let result = self.connection.execute(
            "INSERT INTO cash_accounts
                 (name, account_type, bank_name, iban, opening_balance, current_balance,
                  notes, active)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6, ?7)",
            params![
                account.name,
                account.account_type.as_db(),
                account.bank_name.as_deref(),
                account.iban.as_deref(),
                opening,
                account.notes.as_deref(),
                i64::from(account.active),
            ],
        );

        result.map_err(|error| map_name_error(error, &account.name))?;
        Ok(self.connection.last_insert_rowid())
    }

    /// Var olan kasa/banka hesabını günceller.
    ///
    /// Hesabın hareketi varsa açılış bakiyesi **değiştirilmez** (güncel bakiye
    /// korunur); hareket yoksa açılış ve güncel bakiye birlikte güncellenir.
    pub fn cash_account_update(&self, id: i64, account: &ValidatedCashAccount) -> Result<()> {
        let current = self
            .cash_account_get(id)?
            .ok_or(AppError::CashAccountNotFound(id))?;
        if !self.session_is_manager() && current.active != account.active {
            tracing::warn!(
                user_id = id,
                "yetki reddedildi: kasa hesabı durumu değiştirme"
            );
            return Err(AppError::PermissionDenied {
                operation: "kasa/banka hesabı durumunu değiştirme".to_owned(),
            });
        }
        let transaction = self.connection.unchecked_transaction()?;

        let exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM cash_accounts WHERE id = ?1)",
            [id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(AppError::CashAccountNotFound(id));
        }

        let movements: i64 = transaction.query_row(
            "SELECT
                (SELECT COUNT(*) FROM payments WHERE cash_account_id = ?1)
              + (SELECT COUNT(*) FROM income_expense_entries WHERE cash_account_id = ?1)",
            [id],
            |row| row.get(0),
        )?;

        let result = if movements == 0 {
            transaction.execute(
                "UPDATE cash_accounts
                    SET name = ?2, account_type = ?3, bank_name = ?4, iban = ?5,
                        opening_balance = ?6, current_balance = ?6, notes = ?7, active = ?8,
                        updated_at = datetime('now')
                  WHERE id = ?1",
                params![
                    id,
                    account.name,
                    account.account_type.as_db(),
                    account.bank_name.as_deref(),
                    account.iban.as_deref(),
                    account.opening_balance.kurus(),
                    account.notes.as_deref(),
                    i64::from(account.active),
                ],
            )
        } else {
            transaction.execute(
                "UPDATE cash_accounts
                    SET name = ?2, account_type = ?3, bank_name = ?4, iban = ?5,
                        notes = ?6, active = ?7, updated_at = datetime('now')
                  WHERE id = ?1",
                params![
                    id,
                    account.name,
                    account.account_type.as_db(),
                    account.bank_name.as_deref(),
                    account.iban.as_deref(),
                    account.notes.as_deref(),
                    i64::from(account.active),
                ],
            )
        };

        result.map_err(|error| map_name_error(error, &account.name))?;
        transaction.commit()?;
        Ok(())
    }

    /// Hesabı pasife alır veya yeniden aktife alır.
    pub fn cash_account_set_active(&self, id: i64, active: bool) -> Result<()> {
        self.require_admin("kasa/banka hesabı pasife alma/aktifleştirme")?;
        let updated = self.connection.execute(
            "UPDATE cash_accounts SET active = ?2, updated_at = datetime('now') WHERE id = ?1",
            params![id, i64::from(active)],
        )?;
        if updated == 0 {
            return Err(AppError::CashAccountNotFound(id));
        }
        Ok(())
    }

    /// Belirli türdeki **aktif** hesapların toplam güncel bakiyesi.
    pub fn cash_accounts_total(&self, account_type: AccountType) -> Result<Money> {
        let total: i64 = self.connection.query_row(
            "SELECT COALESCE(SUM(current_balance), 0) FROM cash_accounts
             WHERE account_type = ?1 AND active = 1",
            [account_type.as_db()],
            |row| row.get(0),
        )?;
        Ok(Money::from_kurus(total))
    }
}

/// Bir veritabanı satırını [`CashAccount`] tipine dönüştürür.
fn row_to_cash_account(row: &Row<'_>) -> rusqlite::Result<CashAccount> {
    let type_value: String = row.get(ACCOUNT_TYPE_INDEX)?;
    let account_type = AccountType::from_db(&type_value).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            ACCOUNT_TYPE_INDEX,
            rusqlite::types::Type::Text,
            Box::new(AppError::InvalidRecord(format!(
                "bilinmeyen hesap türü: '{type_value}'"
            ))),
        )
    })?;

    Ok(CashAccount {
        id: row.get(0)?,
        name: row.get(1)?,
        account_type,
        bank_name: row.get(3)?,
        iban: row.get(4)?,
        opening_balance: Money::from_kurus(row.get(5)?),
        current_balance: Money::from_kurus(row.get(6)?),
        notes: row.get(7)?,
        active: row.get::<_, i64>(8)? != 0,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        payment_count: row.get::<_, i64>(11)?.max(0) as usize,
        entry_count: row.get::<_, i64>(12)?.max(0) as usize,
    })
}

/// Ad benzersizlik ihlalini kullanıcıya anlamlı hataya çevirir.
fn map_name_error(error: rusqlite::Error, name: &str) -> AppError {
    if let rusqlite::Error::SqliteFailure(inner, _) = &error
        && inner.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
    {
        return AppError::CashAccountNameTaken(name.to_owned());
    }
    AppError::Database(crate::error::sqlite_error_to_turkish(&error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cash::CashAccountDraft;

    fn valid(overrides: impl FnOnce(&mut CashAccountDraft)) -> ValidatedCashAccount {
        let mut draft = CashAccountDraft {
            name: "Merkez Kasa".to_owned(),
            ..CashAccountDraft::default()
        };
        overrides(&mut draft);
        draft.validate().expect("test verisi geçerli olmalı")
    }

    fn database() -> Database {
        Database::open_in_memory().expect("bellek içi veritabanı açılmalı")
    }

    #[test]
    fn hesap_eklenir_ve_okunur() {
        let database = database();
        let id = database
            .cash_account_insert(&valid(|draft| {
                draft.name = "Merkez Kasa".to_owned();
                draft.opening_balance = "1.000,00".to_owned();
            }))
            .expect("hesap eklenebilmeli");

        let account = database
            .cash_account_get(id)
            .unwrap()
            .expect("hesap bulunmalı");
        assert_eq!(account.name, "Merkez Kasa");
        assert_eq!(account.account_type, AccountType::Kasa);
        assert_eq!(account.opening_balance, Money::from_kurus(100_000));
        assert_eq!(account.current_balance, Money::from_kurus(100_000));
        assert!(account.active);
        assert!(!account.has_movements());
    }

    #[test]
    fn banka_hesabi_iban_ile_eklenir() {
        let database = database();
        let id = database
            .cash_account_insert(&valid(|draft| {
                draft.name = "Ziraat TL".to_owned();
                draft.account_type = AccountType::Banka;
                draft.bank_name = "Ziraat Bankası".to_owned();
                draft.iban = "TR330006100519786457841326".to_owned();
            }))
            .expect("banka hesabı eklenebilmeli");

        let account = database.cash_account_get(id).unwrap().unwrap();
        assert_eq!(account.account_type, AccountType::Banka);
        assert_eq!(account.bank_name.as_deref(), Some("Ziraat Bankası"));
        assert_eq!(account.shortened_iban().as_deref(), Some("TR3300…1326"));
    }

    #[test]
    fn ad_benzersizdir_harf_duyarsiz() {
        let database = database();
        database
            .cash_account_insert(&valid(|draft| draft.name = "Merkez Kasa".to_owned()))
            .unwrap();

        let error = database
            .cash_account_insert(&valid(|draft| draft.name = "merkez kasa".to_owned()))
            .expect_err("aynı ad ikinci kez eklenememeli");
        assert!(matches!(error, AppError::CashAccountNameTaken(_)));
        assert!(
            database
                .cash_account_name_exists("MERKEZ KASA", None)
                .unwrap()
        );
        assert!(
            !database
                .cash_account_name_exists("MERKEZ KASA", Some(1))
                .unwrap()
        );
    }

    #[test]
    fn acilis_bakiyesi_hareket_yoksa_guncellenir() {
        let database = database();
        let id = database
            .cash_account_insert(&valid(|draft| draft.opening_balance = "100,00".to_owned()))
            .unwrap();

        database
            .cash_account_update(
                id,
                &valid(|draft| {
                    draft.name = "Merkez Kasa".to_owned();
                    draft.opening_balance = "250,00".to_owned();
                }),
            )
            .unwrap();

        let account = database.cash_account_get(id).unwrap().unwrap();
        assert_eq!(account.opening_balance, Money::from_kurus(25_000));
        assert_eq!(account.current_balance, Money::from_kurus(25_000));
    }

    #[test]
    fn pasife_alma_ve_aktife_alma_calisir() {
        let database = database();
        let id = database.cash_account_insert(&valid(|_| {})).unwrap();

        database.cash_account_set_active(id, false).unwrap();
        assert!(!database.cash_account_get(id).unwrap().unwrap().active);

        database.cash_account_set_active(id, true).unwrap();
        assert!(database.cash_account_get(id).unwrap().unwrap().active);

        assert!(matches!(
            database.cash_account_set_active(999, false),
            Err(AppError::CashAccountNotFound(999))
        ));
    }

    #[test]
    fn liste_filtre_ve_arama_calisir() {
        let database = database();
        database
            .cash_account_insert(&valid(|draft| {
                draft.name = "Merkez Kasa".to_owned();
            }))
            .unwrap();
        database
            .cash_account_insert(&valid(|draft| {
                draft.name = "Ziraat Bankası".to_owned();
                draft.account_type = AccountType::Banka;
            }))
            .unwrap();
        database
            .cash_account_insert(&valid(|draft| {
                draft.name = "Pasif Kasa".to_owned();
                draft.active = false;
            }))
            .unwrap();

        assert_eq!(database.cash_account_count().unwrap(), 3);

        let all = database
            .cash_account_list(&CashAccountFilter::default())
            .unwrap();
        assert_eq!(all.len(), 3);

        let banks = database
            .cash_account_list(&CashAccountFilter {
                account_type: Some(AccountType::Banka),
                ..CashAccountFilter::default()
            })
            .unwrap();
        assert_eq!(banks.len(), 1);
        assert_eq!(banks[0].name, "Ziraat Bankası");

        let active = database
            .cash_account_list(&CashAccountFilter {
                active: Some(true),
                ..CashAccountFilter::default()
            })
            .unwrap();
        assert_eq!(active.len(), 2);

        let needle = crate::customer::normalize_for_search("ziraat");
        let searched = database
            .cash_account_list(&CashAccountFilter {
                search: needle,
                ..CashAccountFilter::default()
            })
            .unwrap();
        assert_eq!(searched.len(), 1);
    }
}
