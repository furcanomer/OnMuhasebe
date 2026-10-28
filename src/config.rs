// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Uygulama yapılandırması.
//!
//! Uygulamanın veritabanını nereye yazacağını belirler. Varsayılan olarak
//! işletim sisteminin bildirdiği **yerel uygulama verisi** dizini kullanılır:
//!
//! * Windows: `%LOCALAPPDATA%\OnMuhasebe\OnMuhasebe\data\on_muhasebe.db`
//!
//! Taşınabilir (portable) kullanım ve testler için `ON_MUHASEBE_DATA_DIR`
//! ortam değişkeni ile bu dizin geçici olarak değiştirilebilir.

use std::path::{Path, PathBuf};

use directories::ProjectDirs;

use crate::DATABASE_FILE_NAME;
use crate::error::{AppError, Result};

/// Veri dizinini geçersiz kılan ortam değişkeni.
pub const DATA_DIR_ENV_VAR: &str = "ON_MUHASEBE_DATA_DIR";

/// Uygulamanın dosya sistemi yapılandırması.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    data_dir: PathBuf,
    database_path: PathBuf,
}

impl AppConfig {
    /// Yapılandırmayı ortamdan çözer.
    ///
    /// Öncelik sırası:
    /// 1. `ON_MUHASEBE_DATA_DIR` ortam değişkeni (boş değilse)
    /// 2. İşletim sisteminin yerel uygulama verisi dizini
    pub fn load() -> Result<Self> {
        if let Some(directory) =
            std::env::var_os(DATA_DIR_ENV_VAR).filter(|value| !value.is_empty())
        {
            return Ok(Self::with_data_dir(PathBuf::from(directory)));
        }

        Ok(Self::with_data_dir(default_data_dir()?))
    }

    /// Verilen veri dizinini kullanan bir yapılandırma oluşturur.
    pub fn with_data_dir(data_dir: impl Into<PathBuf>) -> Self {
        let data_dir = data_dir.into();
        let database_path = data_dir.join(DATABASE_FILE_NAME);
        Self {
            data_dir,
            database_path,
        }
    }

    /// Veritabanı dosyasının bulunduğu dizin.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Veritabanı dosyasının tam yolu.
    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    /// Otomatik ve manual yedeklerin tutulduğu klasör.
    pub fn backups_dir(&self) -> PathBuf {
        self.data_dir.join("backups")
    }

    /// Günlük (log) dosyalarının tutulduğu klasör.
    pub fn logs_dir(&self) -> PathBuf {
        self.data_dir.join("logs")
    }

    /// Veri dizininin var olduğundan emin olur; yoksa oluşturur.
    pub fn ensure_data_dir(&self) -> Result<()> {
        std::fs::create_dir_all(&self.data_dir)
            .map_err(|error| AppError::file_system(&self.data_dir, error))
    }
}

/// İşletim sisteminin bildirdiği yerel uygulama verisi dizini.
fn default_data_dir() -> Result<PathBuf> {
    let project_dirs = ProjectDirs::from("tr", "OnMuhasebe", "OnMuhasebe")
        .ok_or(AppError::DataDirectoryUnavailable)?;
    Ok(project_dirs.data_local_dir().to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn veritabani_yolu_veri_dizini_icindedir() {
        let config = AppConfig::with_data_dir(PathBuf::from("C:/gecici/on-muhasebe"));

        assert_eq!(config.data_dir(), Path::new("C:/gecici/on-muhasebe"));
        assert_eq!(
            config.database_path(),
            Path::new("C:/gecici/on-muhasebe").join(DATABASE_FILE_NAME)
        );
    }

    #[test]
    fn veri_dizini_gerekirse_olusturulur() {
        let root = std::env::temp_dir().join("on_muhasebe_config_test");
        let _ = std::fs::remove_dir_all(&root);

        let config = AppConfig::with_data_dir(root.join("alt-dizin"));
        config.ensure_data_dir().expect("dizin oluşturulmalı");
        assert!(config.data_dir().is_dir());

        std::fs::remove_dir_all(&root).expect("geçici dizin temizlenmeli");
    }

    #[test]
    fn varsayilan_veri_dizini_mevcuttur() {
        // Windows'ta %LOCALAPPDATA% her zaman bulunur.
        let directory = default_data_dir().expect("varsayılan dizin belirlenmeli");
        assert!(
            directory.is_absolute(),
            "dizin mutlak olmalı: {directory:?}"
        );
    }
}
