// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! `app_settings` tablosunun tipli uygulama ayarları.
//!
//! Arayüz ham anahtar/değer metinleriyle uğraşmaz; bu tip varsayılanları,
//! aralık kontrollerini ve yedekleme zamanı eşiğini merkezîleştirir.

use chrono::{Local, NaiveDateTime, TimeDelta};
use std::time::Duration;

use crate::backup::RotationPolicy;
use crate::db::Database;
use crate::error::{AppError, Result};

/// Ayarlar ekranındaki genel/backup değerleri.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppSettings {
    /// Dil (şimdilik yalnızca `tr`).
    pub language: String,
    /// Otomatik kilit açık mı?
    pub auto_lock_enabled: bool,
    /// Hareketsizlik süresi (dakika). `0` kapalı anlamına gelir.
    pub auto_lock_minutes: u32,
    /// Log seviyesi `info` mu (aksi hâlde `warn`)?
    pub log_info: bool,
    /// Son başarılı otomatik yedek (yerel saat).
    pub last_auto_backup_at: Option<NaiveDateTime>,
    /// Otomatik yedek rotasyon politikası.
    pub rotation: RotationPolicy,
    /// Toplam yedek boyutu uyarı eşiği (MB).
    pub backup_warn_mb: u64,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            language: "tr".to_owned(),
            auto_lock_enabled: true,
            auto_lock_minutes: 15,
            log_info: false,
            last_auto_backup_at: None,
            rotation: RotationPolicy::default(),
            backup_warn_mb: 1000,
        }
    }
}

fn parse_bool(value: Option<String>, default: bool) -> bool {
    value
        .as_deref()
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(default)
}

fn parse_usize(value: Option<String>, default: usize) -> usize {
    value
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn parse_u32(value: Option<String>, default: u32) -> u32 {
    value
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn parse_u64(value: Option<String>, default: u64) -> u64 {
    value
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn parse_datetime(value: Option<String>) -> Option<NaiveDateTime> {
    let value = value?;
    if value.trim().is_empty() {
        return None;
    }
    NaiveDateTime::parse_from_str(&value, "%Y-%m-%d %H:%M:%S").ok()
}

impl AppSettings {
    /// Veritabanından okur; eksik/geçersiz değerlerde varsayılana döner.
    pub fn load(database: &Database) -> Result<Self> {
        let defaults = Self::default();
        Ok(Self {
            language: database
                .setting_get("language")?
                .unwrap_or(defaults.language),
            auto_lock_enabled: parse_bool(
                database.setting_get("auto_lock_enabled")?,
                defaults.auto_lock_enabled,
            ),
            auto_lock_minutes: parse_u32(
                database.setting_get("auto_lock_minutes")?,
                defaults.auto_lock_minutes,
            ),
            log_info: database
                .setting_get("log_level")?
                .is_some_and(|value| value.eq_ignore_ascii_case("info")),
            last_auto_backup_at: parse_datetime(database.setting_get("last_auto_backup_at")?),
            rotation: RotationPolicy {
                daily_keep: parse_usize(
                    database.setting_get("backup_daily_keep")?,
                    defaults.rotation.daily_keep,
                )
                .clamp(1, 3650),
                monthly_keep: parse_usize(
                    database.setting_get("backup_monthly_keep")?,
                    defaults.rotation.monthly_keep,
                )
                .clamp(1, 120),
                yearly_keep: parse_usize(
                    database.setting_get("backup_yearly_keep")?,
                    defaults.rotation.yearly_keep,
                )
                .clamp(1, 50),
            },
            backup_warn_mb: parse_u64(
                database.setting_get("backup_total_warn_mb")?,
                defaults.backup_warn_mb,
            )
            .max(1),
        })
    }

    /// Otomatik kilit süresi; kapalıysa `None`.
    pub fn auto_lock_duration(&self) -> Option<Duration> {
        if self.auto_lock_enabled && self.auto_lock_minutes > 0 {
            Some(Duration::from_secs(u64::from(self.auto_lock_minutes) * 60))
        } else {
            None
        }
    }

    /// Genel ve güvenlik ayarlarını yazar.
    pub fn save_general(&self, database: &Database) -> Result<()> {
        database.setting_set("language", &self.language)?;
        database.setting_set(
            "auto_lock_enabled",
            if self.auto_lock_enabled { "1" } else { "0" },
        )?;
        database.setting_set("auto_lock_minutes", &self.auto_lock_minutes.to_string())?;
        database.setting_set("log_level", if self.log_info { "info" } else { "warn" })?;
        Ok(())
    }

    /// Rotasyon ve disk uyarı eşiğini yazar.
    pub fn save_backup(&self, database: &Database) -> Result<()> {
        database.setting_set("backup_daily_keep", &self.rotation.daily_keep.to_string())?;
        database.setting_set(
            "backup_monthly_keep",
            &self.rotation.monthly_keep.to_string(),
        )?;
        database.setting_set("backup_yearly_keep", &self.rotation.yearly_keep.to_string())?;
        database.setting_set("backup_total_warn_mb", &self.backup_warn_mb.to_string())?;
        Ok(())
    }

    /// Son otomatik yedek zamanını şimdi olarak yazar.
    pub fn mark_auto_backup_now(&mut self, database: &Database) -> Result<()> {
        let now = Local::now().naive_local();
        database.setting_set(
            "last_auto_backup_at",
            &now.format("%Y-%m-%d %H:%M:%S").to_string(),
        )?;
        self.last_auto_backup_at = Some(now);
        Ok(())
    }

    /// Son otomatik yedek 24 saatten eskiyse (veya hiç yoksa) yeni yedek alınmalı.
    pub fn should_auto_backup(&self, now: NaiveDateTime) -> bool {
        match self.last_auto_backup_at {
            Some(last) => now - last >= TimeDelta::hours(24),
            None => true,
        }
    }

    /// Son otomatik yedek zamanı sistem saatine göre gelecekte mi?
    ///
    /// Sistem saati geri alınmışsa `last_auto_backup_at` gelecekte kalabilir.
    /// Bu durumda 24 saat kuralı asla tetiklenmez; uygulama bunu tespit edip
    /// otomatik yedeği atlar ve log'a uyarı yazar.
    pub fn clock_skew_detected(&self, now: NaiveDateTime) -> bool {
        self.last_auto_backup_at
            .is_some_and(|last| last > now + TimeDelta::minutes(5))
    }

    /// Toplam boyut uyarı eşiğini aşıyor mu?
    pub fn backup_size_exceeds_warning(&self, bytes: u64) -> bool {
        bytes >= self.backup_warn_mb.saturating_mul(1024 * 1024)
    }

    /// Arayüzde geçerli seçenekler dışında değer kalmasın diye doğrular.
    pub fn validate(&self) -> Result<()> {
        if !matches!(self.language.as_str(), "tr") {
            return Err(AppError::InvalidRecord(
                "şimdilik yalnızca Türkçe dil desteği vardır".to_owned(),
            ));
        }
        if self.auto_lock_enabled && !matches!(self.auto_lock_minutes, 5 | 15 | 30 | 60) {
            return Err(AppError::InvalidRecord(
                "otomatik kilit süresi 5, 15, 30 veya 60 dakika olmalıdır".to_owned(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varsayilanlar_yuklenir() {
        let database = Database::open_in_memory().unwrap();
        let settings = AppSettings::load(&database).unwrap();
        assert_eq!(settings.language, "tr");
        assert!(settings.auto_lock_enabled);
        assert_eq!(settings.auto_lock_minutes, 15);
        assert!(!settings.log_info);
        assert_eq!(settings.rotation.daily_keep, 30);
    }

    #[test]
    fn ayarlar_yazilir_ve_tekrar_okunur() {
        let database = Database::open_in_memory().unwrap();
        let mut settings = AppSettings::load(&database).unwrap();
        settings.auto_lock_minutes = 30;
        settings.log_info = true;
        settings.rotation.daily_keep = 10;
        settings.save_general(&database).unwrap();
        settings.save_backup(&database).unwrap();

        let loaded = AppSettings::load(&database).unwrap();
        assert_eq!(loaded.auto_lock_minutes, 30);
        assert!(loaded.log_info);
        assert_eq!(loaded.rotation.daily_keep, 10);
    }

    #[test]
    fn yirmi_dort_saat_esigi_dogru() {
        let now =
            NaiveDateTime::parse_from_str("2026-10-23 12:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let mut settings = AppSettings::default();
        assert!(settings.should_auto_backup(now));
        settings.last_auto_backup_at = Some(now - TimeDelta::hours(23));
        assert!(!settings.should_auto_backup(now));
        settings.last_auto_backup_at = Some(now - TimeDelta::hours(25));
        assert!(settings.should_auto_backup(now));
    }

    #[test]
    fn boyut_uyarisi_esigi() {
        let settings = AppSettings {
            backup_warn_mb: 10,
            ..Default::default()
        };
        assert!(!settings.backup_size_exceeds_warning(9 * 1024 * 1024));
        assert!(settings.backup_size_exceeds_warning(10 * 1024 * 1024));
    }
}
