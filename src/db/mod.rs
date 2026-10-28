// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Veritabanı katmanı.
//!
//! Arayüz kodu SQLite'a **doğrudan erişmez**; tüm veri erişimi buradaki
//! [`Database`] tipi üzerinden yapılır. Böylece SQL ve şema bilgisi tek bir
//! katmanda toplanır, arayüz kodu yalnızca anlamlı tiplerle çalışır.
//!
//! ## Dosya konumu
//!
//! Veritabanı dosyası, kullanıcının yerel uygulama verisi dizininde saklanır
//! (bkz. [`crate::config::AppConfig`]). Dosya ve üst dizin ilk açılışta
//! otomatik oluşturulur.

mod cash;
mod customers;
mod income_expense;
mod invoices;
mod migrations;
mod onboarding;
mod payments;
mod products;
mod reports;
pub mod search;
mod settings;
mod transfer;
mod users;

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};

use crate::auth::{Actor, Role};
use crate::error::{AppError, Result};

pub use migrations::{AppliedMigration, MIGRATIONS, Migration, latest_version};
pub use search::{SearchGroup, SearchHit, SearchKind};
pub use settings::SettingEntry;
pub use transfer::{BackupInfo, ImportOutcome, ImportSkip};
pub use users::UserDraft;

/// `app_metadata` tablosunda yedeğin bu uygulamaya ait olduğunu belirten değer.
pub const APP_DB_NAME: &str = "OnMuhasebe";

/// `app_metadata` tablosunda uygulama adını tutan anahtar.
pub const META_APP_NAME: &str = "app_name";

/// `app_metadata` tablosunda veritabanının ilk oluşturulma zamanını tutan anahtar.
pub const META_CREATED_AT: &str = "created_at";

/// `app_metadata` tablosunda veritabanını en son açan uygulama sürümünü tutan anahtar.
pub const META_LAST_APP_VERSION: &str = "last_app_version";

/// Açık bir SQLite veritabanı bağlantısı.
pub struct Database {
    connection: Connection,
    path: Option<PathBuf>,
    /// Uygulama oturumundaki kullanıcı. Doğrudan DB kullanan testlerde `None`
    /// olabilir; arayüz her girişte bu alanı doldurur ve yetki kontrolü burada
    /// (savunma derinliği) tekrar yapılır.
    actor: RefCell<Option<Actor>>,
}

impl Database {
    /// Veritabanını açar.
    ///
    /// Üst dizin ve dosya yoksa oluşturulur, bağlantı ayarları uygulanır ve
    /// bekleyen migration'lar çalıştırılır.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();

        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)
                .map_err(|error| AppError::file_system(parent, error))?;
        }

        let connection = Connection::open(path)?;
        let mut database = Self {
            connection,
            path: Some(path.to_path_buf()),
            actor: RefCell::new(None),
        };
        database.initialize()?;
        Ok(database)
    }

    /// Yalnızca bellekte yaşayan geçici bir veritabanı açar (testler için).
    pub fn open_in_memory() -> Result<Self> {
        let connection = Connection::open_in_memory()?;
        let mut database = Self {
            connection,
            path: None,
            actor: RefCell::new(None),
        };
        database.initialize()?;
        Ok(database)
    }

    fn initialize(&mut self) -> Result<()> {
        self.configure()?;
        migrations::apply_pending(&mut self.connection)?;
        self.touch_metadata()?;
        Ok(())
    }

    /// Bağlantı ayarları: yabancı anahtarlar açık, kilitlenmede bekleme süresi
    /// tanımlı, WAL günlüğü etkin.
    fn configure(&self) -> Result<()> {
        self.connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;
             PRAGMA synchronous = NORMAL;
             PRAGMA journal_mode = WAL;",
        )?;
        Ok(())
    }

    /// İlk açılışta oluşturulma zamanını yazar, her açılışta sürüm bilgisini günceller.
    fn touch_metadata(&self) -> Result<()> {
        self.connection.execute(
            "INSERT OR IGNORE INTO app_metadata (key, value) VALUES (?1, datetime('now'))",
            [META_CREATED_AT],
        )?;

        self.connection.execute(
            "INSERT INTO app_metadata (key, value, updated_at)
             VALUES (?1, ?2, datetime('now'))
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![META_APP_NAME, APP_DB_NAME],
        )?;

        self.connection.execute(
            "INSERT INTO app_metadata (key, value, updated_at)
             VALUES (?1, ?2, datetime('now'))
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![META_LAST_APP_VERSION, crate::APP_VERSION],
        )?;

        Ok(())
    }

    /// Veritabanı dosyasının yolu (bellek içi veritabanlarında `None`).
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Oturumdaki aktörü ayarlar. Çıkışta `None` verilmelidir.
    pub fn set_actor(&self, actor: Option<Actor>) {
        *self.actor.borrow_mut() = actor;
    }

    /// Oturumdaki aktörün kopyası.
    pub fn actor(&self) -> Option<Actor> {
        self.actor.borrow().clone()
    }

    /// Oturumdaki kullanıcı yönetici mi? Aktör yoksa (sistem/test bağlamı)
    /// `true` döner; arayüz oturum açtığında aktörü mutlaka ayarlar.
    pub fn session_is_manager(&self) -> bool {
        self.actor
            .borrow()
            .as_ref()
            .is_none_or(|actor| actor.role == Role::Yonetici)
    }

    /// Yönetici yetkisi gerektiren DB işlemleri için savunma katmanı.
    ///
    /// Arayüz düğmeyi gizlese bile bu kontrol, doğrudan çağrılan DB
    /// fonksiyonlarının operatör tarafından kullanılmasını engeller.
    pub fn require_admin(&self, operation: &str) -> Result<()> {
        if let Some(actor) = self.actor.borrow().as_ref()
            && !actor.is_manager()
        {
            tracing::warn!(
                user = %crate::auth::mask_username(&actor.username),
                operation,
                "yetki reddedildi: yönetici yetkisi gerekli"
            );
            return Err(AppError::PermissionDenied {
                operation: operation.to_owned(),
            });
        }
        Ok(())
    }

    /// Kullanıcı adının kendisi veya yönetici olmasını gerektirir.
    pub fn require_self_or_admin(&self, user_id: i64, operation: &str) -> Result<()> {
        if let Some(actor) = self.actor.borrow().as_ref()
            && actor.user_id != user_id
        {
            self.require_admin(operation)?;
        }
        Ok(())
    }

    /// Mevcut şema sürümü.
    pub fn schema_version(&self) -> Result<i32> {
        migrations::schema_version(&self.connection)
    }

    /// Uygulanmış migration kayıtları.
    pub fn applied_migrations(&self) -> Result<Vec<AppliedMigration>> {
        migrations::applied_migrations(&self.connection)
    }

    /// `app_metadata` tablosundan bir değeri okur.
    pub fn metadata(&self, key: &str) -> Result<Option<String>> {
        let value = self
            .connection
            .query_row(
                "SELECT value FROM app_metadata WHERE key = ?1",
                [key],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        Ok(value)
    }

    /// `app_metadata` tablosuna bir değer yazar (varsa günceller).
    pub fn set_metadata(&self, key: &str, value: &str) -> Result<()> {
        self.connection.execute(
            "INSERT INTO app_metadata (key, value, updated_at)
             VALUES (?1, ?2, datetime('now'))
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, value],
        )?;
        Ok(())
    }

    /// Şema dışındaki kullanıcı tablolarının sayısı.
    pub fn user_table_count(&self) -> Result<usize> {
        let count: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
            [],
            |row| row.get(0),
        )?;
        Ok(count.max(0) as usize)
    }

    /// Arayüzde gösterilecek özet durum bilgisini toplar.
    pub fn status(&self) -> Result<DatabaseStatus> {
        let file_size_bytes = self
            .path
            .as_ref()
            .and_then(|path| std::fs::metadata(path).ok())
            .map(|metadata| metadata.len());

        Ok(DatabaseStatus {
            path: self.path.clone(),
            file_size_bytes,
            schema_version: self.schema_version()?,
            supported_schema_version: migrations::latest_version(),
            applied_migrations: self.applied_migrations()?,
            created_at: self.metadata(META_CREATED_AT)?,
            user_table_count: self.user_table_count()?,
        })
    }
}

/// Arayüzde gösterilen veritabanı durum özeti.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseStatus {
    /// Veritabanı dosyasının yolu.
    pub path: Option<PathBuf>,
    /// Dosya boyutu (bayt).
    pub file_size_bytes: Option<u64>,
    /// Veritabanının mevcut şema sürümü.
    pub schema_version: i32,
    /// Uygulamanın desteklediği en yüksek şema sürümü.
    pub supported_schema_version: i32,
    /// Uygulanmış migration kayıtları.
    pub applied_migrations: Vec<AppliedMigration>,
    /// Veritabanının ilk oluşturulma zamanı (UTC).
    pub created_at: Option<String>,
    /// Şema dışındaki (altyapı dışı) tablo sayısı.
    pub user_table_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bellek_ici_veritabani_acilir_ve_semayi_kurar() {
        let database = Database::open_in_memory().expect("veritabanı açılmalı");

        assert_eq!(database.schema_version().unwrap(), latest_version());
        assert!(database.path().is_none());
        assert!(database.metadata(META_CREATED_AT).unwrap().is_some());
        assert_eq!(
            database.metadata(META_LAST_APP_VERSION).unwrap().as_deref(),
            Some(crate::APP_VERSION)
        );
    }

    #[test]
    fn meta_veri_yazilip_okunabilir() {
        let database = Database::open_in_memory().unwrap();

        assert_eq!(database.metadata("olmayan_anahtar").unwrap(), None);
        database.set_metadata("deneme", "deger").unwrap();
        assert_eq!(
            database.metadata("deneme").unwrap().as_deref(),
            Some("deger")
        );

        // Aynı anahtar güncellenebilmeli.
        database.set_metadata("deneme", "yeni").unwrap();
        assert_eq!(
            database.metadata("deneme").unwrap().as_deref(),
            Some("yeni")
        );
    }
}
