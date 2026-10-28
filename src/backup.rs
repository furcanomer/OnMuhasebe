// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Otomatik/manual yedek dosyaları, doğrulama ve rotasyon politikası.
//!
//! Yedekler veri dizininin yanındaki `backups` klasöründe tutulur. Bir yedek
//! oluşturulduktan sonra [`crate::db::Database::validate_backup`] ile aynı
//! doğrulama çalıştırılır; bozuksa dosya silinir. Rotasyon yalnızca
//! `onmuhasebe-auto-*` dosyalarına uygulanır; manual yedekler korunur.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{Datelike, Local, NaiveDate, NaiveDateTime, TimeDelta};

use crate::db::Database;
use crate::error::{AppError, Result};

/// Yedek türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupKind {
    /// Uygulama açılışında alınan yedek.
    Auto,
    /// Kullanıcının Ayarlar ekranından aldığı yedek.
    Manual,
}

impl BackupKind {
    /// Dosya adı öneki.
    pub fn prefix(self) -> &'static str {
        match self {
            Self::Auto => "onmuhasebe-auto-",
            Self::Manual => "onmuhasebe-manual-",
        }
    }

    /// Kullanıcıya gösterilen etiket.
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Otomatik",
            Self::Manual => "Manuel",
        }
    }
}

/// Disk üzerindeki bir yedek dosyasının özeti.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupEntry {
    /// Dosya yolu.
    pub path: PathBuf,
    /// Dosya adı.
    pub file_name: String,
    /// Otomatik mi manual mi?
    pub kind: BackupKind,
    /// Dosya adından çözülen yerel tarih/saat.
    pub taken_at: Option<NaiveDateTime>,
    /// Dosya boyutu (bayt).
    pub size_bytes: u64,
}

/// Otomatik yedek rotasyon politikası.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RotationPolicy {
    /// Son kaç gün için günde bir yedek tutulur.
    pub daily_keep: usize,
    /// Son kaç ay için ayda bir yedek tutulur.
    pub monthly_keep: usize,
    /// Son kaç yıl için yılda bir yedek tutulur.
    pub yearly_keep: usize,
}

impl Default for RotationPolicy {
    fn default() -> Self {
        Self {
            daily_keep: 30,
            monthly_keep: 12,
            yearly_keep: 5,
        }
    }
}

/// `onmuhasebe-auto-YYYYMMDD-HHMMSS.onm-backup` biçiminde dosya adı üretir.
pub fn backup_file_name(kind: BackupKind, at: NaiveDateTime) -> String {
    format!("{}{}.onm-backup", kind.prefix(), at.format("%Y%m%d-%H%M%S"))
}

/// Şu anki yerel zamana göre dosya adı üretir; aynı saniyede çakışma olursa
/// sonuna `-1`, `-2` eklenir.
pub fn unique_backup_path(directory: &Path, kind: BackupKind) -> PathBuf {
    let base = backup_file_name(kind, Local::now().naive_local());
    let candidate = directory.join(&base);
    if !candidate.exists() {
        return candidate;
    }
    let stem = base.trim_end_matches(".onm-backup");
    for suffix in 1..1000 {
        let path = directory.join(format!("{stem}-{suffix}.onm-backup"));
        if !path.exists() {
            return path;
        }
    }
    directory.join(base)
}

/// Yedek dosyasını adından çözümler.
pub fn parse_backup_file(path: &Path) -> Option<(BackupKind, NaiveDateTime)> {
    if !path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("onm-backup"))
    {
        return None;
    }
    let file_name = path.file_name()?.to_str()?;
    let (kind, rest) = if let Some(rest) = file_name.strip_prefix(BackupKind::Auto.prefix()) {
        (BackupKind::Auto, rest)
    } else {
        (
            BackupKind::Manual,
            file_name.strip_prefix(BackupKind::Manual.prefix())?,
        )
    };
    let stamp = rest.strip_suffix(".onm-backup")?;
    // Biçim: YYYYMMDD-HHMMSS; aynı saniyedeki çakışmalar için "-N" eklenebilir.
    if stamp.len() < 15 || !stamp.is_char_boundary(15) {
        return None;
    }
    let base = &stamp[..15];
    let suffix = &stamp[15..];
    if !suffix.is_empty() && !suffix.starts_with('-') {
        return None;
    }
    let parsed = NaiveDateTime::parse_from_str(base, "%Y%m%d-%H%M%S").ok()?;
    Some((kind, parsed))
}

/// Bir yedeği oluşturur: WAL'i boşaltır, dosyayı kopyalar ve doğrular.
///
/// Doğrulama başarısızsa hedef dosya silinir ve hata döner.
pub fn create_backup(database: &Database, target: &Path) -> Result<()> {
    let source = database.path().ok_or_else(|| {
        AppError::Maintenance("Yedek alınamadı: veritabanı dosyası bulunamadı.".to_owned())
    })?;
    if source == target {
        return Err(AppError::Maintenance(
            "Yedek hedefi veritabanı dosyası olamaz.".to_owned(),
        ));
    }

    database.checkpoint()?;
    if let Some(parent) = target
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| AppError::file_system(parent, error))?;
    }
    fs::copy(source, target).map_err(|error| AppError::file_system(target, error))?;

    match Database::validate_backup(target) {
        Ok(_) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(target);
            Err(error)
        }
    }
}

/// Yedek klasöründeki dosyaları tarihe göre (en yeni önce) listeler.
pub fn list_backups(directory: &Path) -> Result<Vec<BackupEntry>> {
    if !directory.exists() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(directory).map_err(|error| AppError::file_system(directory, error))? {
        let entry = entry.map_err(|error| AppError::file_system(directory, error))?;
        let path = entry.path();
        let Some((kind, taken_at)) = parse_backup_file(&path) else {
            continue;
        };
        let metadata = entry
            .metadata()
            .map_err(|error| AppError::file_system(&path, error))?;
        entries.push(BackupEntry {
            file_name: entry.file_name().to_string_lossy().into_owned(),
            path,
            kind,
            taken_at: Some(taken_at),
            size_bytes: metadata.len(),
        });
    }

    entries.sort_by(|left, right| {
        right
            .taken_at
            .cmp(&left.taken_at)
            .then_with(|| right.file_name.cmp(&left.file_name))
    });
    Ok(entries)
}

/// Klasördeki tüm yedeklerin toplam boyutu.
pub fn total_backup_bytes(directory: &Path) -> Result<u64> {
    Ok(list_backups(directory)?
        .iter()
        .map(|entry| entry.size_bytes)
        .sum())
}

fn months_back(year: i32, month: u32, count: usize) -> (i32, u32) {
    let total = year * 12 + (month as i32 - 1) - count as i32;
    (total.div_euclid(12), total.rem_euclid(12) as u32 + 1)
}

/// Otomatik yedekler için rotasyonu uygular ve silinen dosyaları döndürür.
///
/// * Günlük: son `daily_keep` günün her biri için en yeni yedek.
/// * Aylık: son `monthly_keep` ayın her biri için en eski yedek.
/// * Yıllık: son `yearly_keep` yılın her biri için en eski yedek.
///
/// Manuel yedekler ve adı çözülemeyen dosyalar **silinmez**.
pub fn rotate_auto_backups(
    directory: &Path,
    policy: RotationPolicy,
    today: NaiveDate,
) -> Result<Vec<PathBuf>> {
    let entries = list_backups(directory)?;
    let mut auto: Vec<(NaiveDateTime, PathBuf)> = entries
        .into_iter()
        .filter(|entry| entry.kind == BackupKind::Auto)
        .filter_map(|entry| entry.taken_at.map(|taken_at| (taken_at, entry.path)))
        .collect();
    auto.sort_by_key(|(taken_at, _)| *taken_at);

    let mut keep: BTreeSet<PathBuf> = BTreeSet::new();

    if policy.daily_keep > 0 {
        let first_day = today - TimeDelta::days(policy.daily_keep as i64 - 1);
        let mut latest_per_day: BTreeMap<NaiveDate, (NaiveDateTime, PathBuf)> = BTreeMap::new();
        for (taken_at, path) in &auto {
            let date = taken_at.date();
            if date >= first_day && date <= today {
                latest_per_day.insert(date, (*taken_at, path.clone()));
            }
        }
        keep.extend(latest_per_day.into_values().map(|(_, path)| path));
    }

    if policy.monthly_keep > 0 {
        let (first_year, first_month) = months_back(
            today.year(),
            today.month(),
            policy.monthly_keep.saturating_sub(1),
        );
        let mut earliest_per_month: BTreeMap<(i32, u32), (NaiveDateTime, PathBuf)> =
            BTreeMap::new();
        for (taken_at, path) in &auto {
            let key = (taken_at.year(), taken_at.month());
            let after_start = (key.0, key.1) >= (first_year, first_month);
            let before_end = (key.0, key.1) <= (today.year(), today.month());
            if after_start && before_end {
                earliest_per_month
                    .entry(key)
                    .or_insert((*taken_at, path.clone()));
            }
        }
        keep.extend(earliest_per_month.into_values().map(|(_, path)| path));
    }

    if policy.yearly_keep > 0 {
        let first_year = today.year() - policy.yearly_keep as i32 + 1;
        let mut earliest_per_year: BTreeMap<i32, (NaiveDateTime, PathBuf)> = BTreeMap::new();
        for (taken_at, path) in &auto {
            if taken_at.year() >= first_year && taken_at.year() <= today.year() {
                earliest_per_year
                    .entry(taken_at.year())
                    .or_insert((*taken_at, path.clone()));
            }
        }
        keep.extend(earliest_per_year.into_values().map(|(_, path)| path));
    }

    let mut deleted = Vec::new();
    for (_, path) in auto {
        if !keep.contains(&path) {
            fs::remove_file(&path).map_err(|error| AppError::file_system(&path, error))?;
            deleted.push(path);
        }
    }
    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn touch(directory: &Path, name: &str) {
        let path = directory.join(name);
        let mut file = fs::File::create(path).unwrap();
        file.write_all(b"x").unwrap();
    }

    #[test]
    fn yedek_adi_ve_cozumleme_tutarli() {
        let at = NaiveDate::from_ymd_opt(2026, 10, 23)
            .unwrap()
            .and_hms_opt(14, 5, 9)
            .unwrap();
        let name = backup_file_name(BackupKind::Auto, at);
        assert_eq!(name, "onmuhasebe-auto-20261023-140509.onm-backup");
        let parsed = parse_backup_file(Path::new(&name)).unwrap();
        assert_eq!(parsed, (BackupKind::Auto, at));
        assert!(parse_backup_file(Path::new("baska.db")).is_none());
    }

    #[test]
    fn rotasyon_gunluk_aylik_yillik_yedekleri_korur() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();

        // 2026-10-23 bugününe göre: 3 günlük, 2 aylık, 2 yıllık.
        for (name, _) in [
            ("onmuhasebe-auto-20261023-090000.onm-backup", ()),
            ("onmuhasebe-auto-20261023-180000.onm-backup", ()),
            ("onmuhasebe-auto-20261022-090000.onm-backup", ()),
            ("onmuhasebe-auto-20261021-090000.onm-backup", ()),
            ("onmuhasebe-auto-20261001-090000.onm-backup", ()),
            ("onmuhasebe-auto-20260901-090000.onm-backup", ()),
            ("onmuhasebe-auto-20260101-090000.onm-backup", ()),
            ("onmuhasebe-auto-20250101-090000.onm-backup", ()),
            ("onmuhasebe-auto-20240101-090000.onm-backup", ()),
            ("onmuhasebe-manual-20261023-090000.onm-backup", ()),
        ] {
            touch(root, name);
        }

        let deleted = rotate_auto_backups(
            root,
            RotationPolicy {
                daily_keep: 3,
                monthly_keep: 2,
                yearly_keep: 2,
            },
            NaiveDate::from_ymd_opt(2026, 10, 23).unwrap(),
        )
        .unwrap();

        // Günlük: 23 (en yeni), 22, 21. Aylık: Ekim'in en eskisi (01),
        // Eylül (01). Yıllık: 2026'nın en eskisi (Ocak), 2025 (Ocak).
        assert!(
            !root
                .join("onmuhasebe-auto-20261023-090000.onm-backup")
                .exists(),
            "aynı günün daha eski yedeği silinmeli"
        );
        assert!(
            root.join("onmuhasebe-auto-20261023-180000.onm-backup")
                .exists(),
            "günün en yeni yedeği korunmalı"
        );
        assert!(
            !root
                .join("onmuhasebe-auto-20240101-090000.onm-backup")
                .exists(),
            "2 yıllık pencere dışındaki yedek silinmeli"
        );
        assert!(
            root.join("onmuhasebe-manual-20261023-090000.onm-backup")
                .exists(),
            "manuel yedek rotasyona dahil değildir"
        );
        assert!(deleted.iter().any(|path| {
            path.file_name()
                .is_some_and(|name| name == "onmuhasebe-auto-20261023-090000.onm-backup")
        }));
    }

    #[test]
    fn yedek_listesi_boyut_ve_siralama_dondurur() {
        let directory = tempfile::tempdir().unwrap();
        touch(
            directory.path(),
            "onmuhasebe-manual-20261023-090000.onm-backup",
        );
        touch(
            directory.path(),
            "onmuhasebe-auto-20261023-100000.onm-backup",
        );
        let entries = list_backups(directory.path()).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].kind, BackupKind::Auto);
        assert_eq!(total_backup_bytes(directory.path()).unwrap(), 2);
    }
}
