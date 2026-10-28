// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! İlk çalıştırma sihirbazının veritabanı işlemleri.

use rusqlite::params;

use crate::error::{AppError, Result};
use crate::onboarding::{ONBOARDING_COMPLETED_KEY, OnboardingPlan};

use super::Database;

impl Database {
    /// Sihirbaz tamamlandı mı?
    pub fn onboarding_completed(&self) -> Result<bool> {
        Ok(self
            .setting_get(ONBOARDING_COMPLETED_KEY)?
            .is_some_and(|value| value == "1"))
    }

    /// Sihirbazı atlandı/tamamlandı olarak işaretler.
    pub fn mark_onboarding_completed(&self) -> Result<()> {
        self.setting_set(ONBOARDING_COMPLETED_KEY, "1")
    }

    /// İlk çalıştırma sihirbazının tüm yazımlarını **tek transaction** içinde
    /// uygular.
    ///
    /// Şirket bilgileri `app_settings` anahtarlarına yazılır. Kasa/banka hesabı
    /// ve kategoriler ise yalnızca seçildiyse eklenir. Aynı ada sahip kategori
    /// zaten varsa sessizce atlanır; kasa hesabı adı çakışırsa transaction
    /// geri alınır ve [`AppError::CashAccountNameTaken`] döner.
    pub fn apply_onboarding(&self, plan: &OnboardingPlan) -> Result<()> {
        let transaction = self.connection.unchecked_transaction()?;

        for (key, value) in plan.company.as_pairs() {
            transaction.execute(
                "INSERT INTO app_settings (key, value, updated_at)
                 VALUES (?1, ?2, datetime('now'))
                 ON CONFLICT(key) DO UPDATE SET
                     value = excluded.value,
                     updated_at = excluded.updated_at",
                params![key, value],
            )?;
        }

        if let Some(cash) = &plan.cash {
            let exists: bool = transaction.query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM cash_accounts WHERE name = ?1 COLLATE NOCASE
                 )",
                [cash.name.as_str()],
                |row| row.get(0),
            )?;
            if exists {
                return Err(AppError::CashAccountNameTaken(cash.name.clone()));
            }

            transaction.execute(
                "INSERT INTO cash_accounts
                     (name, account_type, bank_name, iban, opening_balance, current_balance,
                      notes, active)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6, ?7)",
                params![
                    cash.name,
                    cash.account_type.as_db(),
                    cash.bank_name.as_deref(),
                    cash.iban.as_deref(),
                    cash.opening_balance.kurus(),
                    cash.notes.as_deref(),
                    i64::from(cash.active),
                ],
            )?;
        }

        for category in &plan.categories {
            let exists: bool = transaction.query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM expense_categories
                     WHERE name = ?1 COLLATE NOCASE AND category_type = ?2
                 )",
                params![category.name.as_str(), category.category_type.as_db()],
                |row| row.get(0),
            )?;
            if exists {
                continue;
            }

            transaction.execute(
                "INSERT INTO expense_categories (name, category_type, description, active)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    category.name,
                    category.category_type.as_db(),
                    category.description,
                    i64::from(category.active),
                ],
            )?;
        }

        transaction.execute(
            "INSERT INTO app_settings (key, value, updated_at)
             VALUES (?1, '1', datetime('now'))
             ON CONFLICT(key) DO UPDATE SET
                 value = '1',
                 updated_at = excluded.updated_at",
            [ONBOARDING_COMPLETED_KEY],
        )?;

        transaction.commit()?;
        Ok(())
    }

    /// Şirket bilgisi anahtarının var olup olmadığını (test/teshis amaçlı) okur.
    pub fn onboarding_company_name(&self) -> Result<Option<String>> {
        self.setting_get(crate::onboarding::COMPANY_NAME_KEY)
    }
}
