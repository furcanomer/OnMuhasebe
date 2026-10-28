// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! `app_settings` anahtar-değer tablosunun veri erişimi.
//!
//! Ayarlar migration v7'de varsayılan değerlerle oluşturulur. Arayüz yalnızca
//! bu metotları kullanır; SQL arayüz katmanına sızmaz.

use rusqlite::{OptionalExtension, params};

use super::Database;
use crate::error::{AppError, Result};

/// Arayüzde listelenebilen bir ayar satırı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingEntry {
    /// Ayar anahtarı.
    pub key: String,
    /// Ayar değeri.
    pub value: String,
    /// Son güncelleme zamanı (UTC).
    pub updated_at: String,
}

impl Database {
    /// Tek bir ayarı okur; yoksa `None`.
    pub fn setting_get(&self, key: &str) -> Result<Option<String>> {
        let value = self
            .connection
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                [key],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        Ok(value)
    }

    /// Ayarı yazar; varsa günceller.
    pub fn setting_set(&self, key: &str, value: &str) -> Result<()> {
        if key.trim().is_empty() {
            return Err(AppError::InvalidRecord(
                "ayar anahtarı boş olamaz".to_owned(),
            ));
        }
        self.connection.execute(
            "INSERT INTO app_settings (key, value, updated_at)
             VALUES (?1, ?2, datetime('now'))
             ON CONFLICT(key) DO UPDATE SET
                 value = excluded.value,
                 updated_at = excluded.updated_at",
            params![key, value],
        )?;
        Ok(())
    }

    /// Tüm ayarları anahtar sırasıyla döndürür.
    pub fn setting_all(&self) -> Result<Vec<SettingEntry>> {
        let mut statement = self
            .connection
            .prepare("SELECT key, value, updated_at FROM app_settings ORDER BY key")?;
        let rows = statement.query_map([], |row| {
            Ok(SettingEntry {
                key: row.get(0)?,
                value: row.get(1)?,
                updated_at: row.get(2)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(AppError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varsayilan_ayarlar_migration_ile_gelir() {
        let database = Database::open_in_memory().unwrap();
        assert_eq!(
            database
                .setting_get("auto_lock_minutes")
                .unwrap()
                .as_deref(),
            Some("15")
        );
        assert_eq!(
            database
                .setting_get("backup_daily_keep")
                .unwrap()
                .as_deref(),
            Some("30")
        );
        assert_eq!(
            database.setting_get("log_level").unwrap().as_deref(),
            Some("warn")
        );
    }

    #[test]
    fn ayar_yazilip_guncellenir() {
        let database = Database::open_in_memory().unwrap();
        database.setting_set("auto_lock_minutes", "30").unwrap();
        assert_eq!(
            database
                .setting_get("auto_lock_minutes")
                .unwrap()
                .as_deref(),
            Some("30")
        );
        let all = database.setting_all().unwrap();
        assert!(all.iter().any(|entry| entry.key == "auto_lock_minutes"));
    }

    #[test]
    fn bos_anahtar_reddedilir() {
        let database = Database::open_in_memory().unwrap();
        let error = database.setting_set("  ", "x").unwrap_err();
        assert!(matches!(error, AppError::InvalidRecord(_)));
    }
}
