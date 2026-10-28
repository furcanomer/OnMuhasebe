// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Geri yükleme günlüğü (journal).
//!
//! Yedek geri yükleme, veritabanı dosyasının yerine yeni bir dosya koyduğu için
//! süreç tam ortasında kesilebilir. Bu modül, işlemin hangi adımda olduğunu
//! veri dizinindeki `restore.journal` dosyasına yazar. Uygulama bir sonraki
//! açılışta yarıda kalmış bir geri yükleme görürse **hiçbir şeyi otomatik
//! uygulamaz**; kullanıcıya üç seçenek sunar.
//!
//! Adımlar: `copying -> validating -> reopening -> done`.
//! `done` adımındaki günlük açılışta sessizce silinir.

use std::path::{Path, PathBuf};

use chrono::Local;

use crate::error::{AppError, Result};

/// Veri dizinindeki günlük dosyasının adı.
pub const JOURNAL_FILE_NAME: &str = "restore.journal";

/// Geri yükleme işleminin bulunduğu adım.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreStep {
    /// Yedek geçici dosyaya kopyalanıyor.
    Copying,
    /// Geçici kopya doğrulanıyor.
    Validating,
    /// Yeni dosya veritabanının yerine konuyor ve bağlantı yeniden açılıyor.
    Reopening,
    /// İşlem tamamlandı; günlük silinebilir.
    Done,
}

impl RestoreStep {
    /// Günlük dosyasına yazılan metin.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Copying => "copying",
            Self::Validating => "validating",
            Self::Reopening => "reopening",
            Self::Done => "done",
        }
    }

    /// Günlük dosyasındaki metni adıma çevirir.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "copying" => Some(Self::Copying),
            "validating" => Some(Self::Validating),
            "reopening" => Some(Self::Reopening),
            "done" => Some(Self::Done),
            _ => None,
        }
    }

    /// Bu adım yarıda kalmış bir geri yüklemeyi mi gösterir?
    pub fn is_interrupted(self) -> bool {
        self != Self::Done
    }
}

/// `restore.journal` dosyasının içeriği.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreJournal {
    /// İşlemin başladığı an (ISO 8601).
    pub start: String,
    /// Geri yüklenen yedek dosyasının tam yolu.
    pub source_backup: PathBuf,
    /// Geri yükleme öncesi alınan güvenlik yedeğinin tam yolu.
    pub pre_restore_backup: PathBuf,
    /// Yerine yazılan veritabanı dosyasının tam yolu.
    pub target_db: PathBuf,
    /// İşlemin son bilinen adımı.
    pub step: RestoreStep,
}

impl RestoreJournal {
    /// Yeni bir günlük oluşturur (`copying` adımıyla).
    pub fn new(
        source_backup: impl Into<PathBuf>,
        pre_restore_backup: impl Into<PathBuf>,
        target_db: impl Into<PathBuf>,
    ) -> Self {
        Self {
            start: Local::now().to_rfc3339(),
            source_backup: source_backup.into(),
            pre_restore_backup: pre_restore_backup.into(),
            target_db: target_db.into(),
            step: RestoreStep::Copying,
        }
    }

    /// Veri dizinine karşılık gelen günlük yolu.
    pub fn path(data_dir: &Path) -> PathBuf {
        data_dir.join(JOURNAL_FILE_NAME)
    }

    /// Günlüğü diske yazar; üst dizini gerekirse oluşturur.
    pub fn write(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)
                .map_err(|error| AppError::file_system(parent, error))?;
        }
        std::fs::write(path, self.to_text()).map_err(|error| AppError::file_system(path, error))
    }

    /// Günlüğü okur; dosya yoksa `None` döner.
    pub fn read(path: &Path) -> Result<Option<Self>> {
        if !path.is_file() {
            return Ok(None);
        }
        let text =
            std::fs::read_to_string(path).map_err(|error| AppError::file_system(path, error))?;
        Self::parse(&text).map(Some)
    }

    /// Günlük metnini ayrıştırır.
    pub fn parse(text: &str) -> Result<Self> {
        let mut start = None;
        let mut source_backup = None;
        let mut pre_restore_backup = None;
        let mut target_db = None;
        let mut step = None;

        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "start" => start = Some(value.to_owned()),
                "source_backup" => source_backup = Some(PathBuf::from(value)),
                "pre_restore_backup" => pre_restore_backup = Some(PathBuf::from(value)),
                "target_db" => target_db = Some(PathBuf::from(value)),
                "step" => {
                    step = Some(RestoreStep::parse(value).ok_or_else(|| {
                        AppError::Maintenance(format!(
                            "Geri yükleme günlüğünde bilinmeyen adım: '{value}'"
                        ))
                    })?)
                }
                _ => {}
            }
        }

        let missing = |field: &str| {
            AppError::Maintenance(format!(
                "Geri yükleme günlüğü eksik veya bozuk: '{field}' alanı yok."
            ))
        };

        Ok(Self {
            start: start.ok_or_else(|| missing("start"))?,
            source_backup: source_backup.ok_or_else(|| missing("source_backup"))?,
            pre_restore_backup: pre_restore_backup.ok_or_else(|| missing("pre_restore_backup"))?,
            target_db: target_db.ok_or_else(|| missing("target_db"))?,
            step: step.ok_or_else(|| missing("step"))?,
        })
    }

    /// Günlüğün disk biçimi.
    pub fn to_text(&self) -> String {
        format!(
            "start={}\nsource_backup={}\npre_restore_backup={}\ntarget_db={}\nstep={}\n",
            self.start,
            self.source_backup.display(),
            self.pre_restore_backup.display(),
            self.target_db.display(),
            self.step.as_str()
        )
    }

    /// Adımı günceller ve günlüğü yeniden yazar.
    pub fn update_step(&mut self, path: &Path, step: RestoreStep) -> Result<()> {
        self.step = step;
        self.write(path)
    }

    /// Günlük dosyasını siler (yoksa sessizce başarılı olur).
    pub fn remove(path: &Path) -> Result<()> {
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(AppError::file_system(path, error)),
        }
    }

    /// Yarıda kalmış geri yükleme mi?
    pub fn is_interrupted(&self) -> bool {
        self.step.is_interrupted()
    }
}

/// Veri dizinindeki günlüğü denetler.
///
/// * Günlük yoksa `None`.
/// * `step=done` ise günlük sessizce silinir ve `None` döner.
/// * Diğer adımlarda yarıda kalmış geri yükleme döner; **otomatik işlem yapılmaz**.
pub fn detect(data_dir: &Path) -> Result<Option<RestoreJournal>> {
    let path = RestoreJournal::path(data_dir);
    match RestoreJournal::read(&path)? {
        None => Ok(None),
        Some(journal) if journal.step == RestoreStep::Done => {
            RestoreJournal::remove(&path)?;
            Ok(None)
        }
        Some(journal) => Ok(Some(journal)),
    }
}

/// Kullanıcının "Güvenlik yedeğine dön" seçimini uygular.
///
/// Güvenlik yedeğini hedef veritabanının üzerine kopyalar. Çağıran taraf,
/// veritabanı bağlantısını önceden kapatmalıdır.
pub fn apply_safety_backup(journal: &RestoreJournal) -> Result<()> {
    if !journal.pre_restore_backup.is_file() {
        return Err(AppError::Maintenance(format!(
            "Güvenlik yedeği bulunamadı: {}",
            journal.pre_restore_backup.display()
        )));
    }

    remove_sidecars(&journal.target_db);
    std::fs::copy(&journal.pre_restore_backup, &journal.target_db)
        .map_err(|error| AppError::file_system(&journal.target_db, error))?;
    remove_sidecars(&journal.target_db);
    Ok(())
}

fn remove_sidecars(path: &Path) {
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}
