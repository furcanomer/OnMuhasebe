// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Veri aktarımı: CSV içe aktarma işlemleri ve yedek doğrulama.
//!
//! CSV içe aktarma **tek bir SQLite işlemi** içinde yapılır. Herhangi bir
//! satır yazılamazsa işlem tamamen geri alınır; yarım kalmış aktarım oluşmaz.
//! Veritabanında zaten bulunan kodlar atlanır ve sonuçta raporlanır.
//!
//! Yedek doğrulaması yalnızca **salt okunur** bir bağlantı açar; mevcut
//! veritabanına dokunmaz. Yedek dosyasının gerçek bir SQLite veritabanı,
//! `schema_migrations` tablosunun ve `app_metadata.app_name = "OnMuhasebe"`
//! kaydının bulunması zorunludur.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, OptionalExtension, params};

use crate::customer::ValidatedCustomer;
use crate::error::{AppError, Result};
use crate::product::ValidatedProduct;

use super::{APP_DB_NAME, Database, latest_version};

/// İçe aktarmada atlanan tek bir kayıt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSkip {
    /// Kaydın kodu (boşsa `(kod yok)`).
    pub code: String,
    /// Atlanma nedeni.
    pub reason: String,
}

/// İçe aktarma sonucu.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportOutcome {
    /// Eklenen kayıt sayısı.
    pub inserted: usize,
    /// Atlanan kayıtlar.
    pub skipped: Vec<ImportSkip>,
}

/// Doğrulanmış yedek dosyasının özeti.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupInfo {
    /// Yedek dosyasının yolu.
    pub path: PathBuf,
    /// Yedekteki en yüksek şema sürümü.
    pub schema_version: i32,
    /// Yedeği oluşturan uygulama adı.
    pub app_name: String,
    /// Yedeğin oluşturulma zamanı (varsa).
    pub created_at: Option<String>,
}

impl Database {
    /// WAL içeriğini ana veritabanı dosyasına aktarır.
    ///
    /// Yedek kopyalanmadan önce çağrılır; böylece `.onm-backup` dosyası tek
    /// başına açılabilen, bayt bayt tutarlı bir SQLite veritabanı olur.
    pub fn checkpoint(&self) -> Result<()> {
        self.connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(())
    }

    /// Cari hesapları tek işlem içinde içe aktarır.
    pub fn import_customers(&self, rows: &[ValidatedCustomer]) -> Result<ImportOutcome> {
        self.require_admin("CSV içe aktarma")?;
        let transaction = self.connection.unchecked_transaction()?;
        let mut outcome = ImportOutcome::default();

        for row in rows {
            let exists: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM customers WHERE code = ?1)",
                [row.code.as_str()],
                |result| result.get(0),
            )?;
            if exists {
                outcome.skipped.push(ImportSkip {
                    code: row.code.clone(),
                    reason: "kod zaten mevcut".to_owned(),
                });
                continue;
            }

            transaction.execute(
                "INSERT INTO customers
                     (code, name, kind, phone, email, address, tax_office, tax_number,
                      national_id, contact_person, notes, active)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    row.code,
                    row.name,
                    row.kind.as_db(),
                    row.phone.as_deref(),
                    row.email.as_deref(),
                    row.address.as_deref(),
                    row.tax_office.as_deref(),
                    row.tax_number.as_deref(),
                    row.national_id.as_deref(),
                    row.contact_person.as_deref(),
                    row.notes.as_deref(),
                    i64::from(row.active),
                ],
            )?;
            outcome.inserted += 1;
        }

        transaction.commit()?;
        Ok(outcome)
    }

    /// Ürünleri tek işlem içinde içe aktarır.
    ///
    /// İçe aktarılan ürünlerin başlangıç stoğu **sıfırdır** ve stok hareketi
    /// oluşturulmaz. Stok yalnızca sonraki hareketlerle değişir.
    pub fn import_products(&self, rows: &[ValidatedProduct]) -> Result<ImportOutcome> {
        self.require_admin("CSV içe aktarma")?;
        let transaction = self.connection.unchecked_transaction()?;
        let mut outcome = ImportOutcome::default();

        for row in rows {
            let code_exists: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM products WHERE code = ?1)",
                [row.code.as_str()],
                |result| result.get(0),
            )?;
            if code_exists {
                outcome.skipped.push(ImportSkip {
                    code: row.code.clone(),
                    reason: "kod zaten mevcut".to_owned(),
                });
                continue;
            }

            if let Some(barcode) = row.barcode.as_deref().filter(|value| !value.is_empty()) {
                let barcode_exists: bool = transaction.query_row(
                    "SELECT EXISTS(SELECT 1 FROM products WHERE barcode = ?1)",
                    [barcode],
                    |result| result.get(0),
                )?;
                if barcode_exists {
                    outcome.skipped.push(ImportSkip {
                        code: row.code.clone(),
                        reason: "barkod zaten mevcut".to_owned(),
                    });
                    continue;
                }
            }

            transaction.execute(
                "INSERT INTO products
                     (code, barcode, name, unit, category, purchase_price, sale_price,
                      vat_rate, min_stock, current_stock, notes, active)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, ?10, ?11)",
                params![
                    row.code,
                    row.barcode.as_deref(),
                    row.name,
                    row.unit.as_db(),
                    row.category.as_deref(),
                    row.purchase_price.kurus(),
                    row.sale_price.kurus(),
                    row.vat_rate,
                    row.min_stock.milli(),
                    row.notes.as_deref(),
                    i64::from(row.active),
                ],
            )?;
            outcome.inserted += 1;
        }

        transaction.commit()?;
        Ok(outcome)
    }

    /// Yedek dosyasını mevcut veritabanına dokunmadan doğrular.
    pub fn validate_backup(path: impl AsRef<Path>) -> Result<BackupInfo> {
        let path = path.as_ref();
        validate_sqlite_header(path)?;
        // Yedek dosyası statiktir. `immutable=1` olmadan WAL kipindeki bir
        // kopyayı açmak `-wal`/`-shm` yan dosyaları oluşturur; bu dosyalar
        // yedek klasöründe gereksiz kirlilik bırakır.
        let uri = immutable_backup_uri(path);
        let connection = Connection::open_with_flags(
            &uri,
            OpenFlags::SQLITE_OPEN_READ_ONLY
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_URI,
        )
        .map_err(|error| AppError::BackupInvalid(format!("Yedek dosyası açılamadı: {error}")))?;

        let check: String = connection
            .query_row("PRAGMA quick_check(1)", [], |row| row.get(0))
            .map_err(|error| {
                AppError::BackupInvalid(format!("Yedek dosyası geçersiz veya bozuk: {error}"))
            })?;
        if !check.eq_ignore_ascii_case("ok") {
            return Err(AppError::BackupInvalid(format!(
                "Yedek dosyası bozuk: {check}"
            )));
        }

        let has_migrations: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master
                  WHERE type = 'table' AND name = 'schema_migrations')",
                [],
                |row| row.get(0),
            )
            .map_err(backup_read_error)?;
        if !has_migrations {
            return Err(AppError::BackupInvalid(
                "Yedek dosyasında şema bilgisi (schema_migrations) bulunamadı.".to_owned(),
            ));
        }

        let schema_version: i32 = connection
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .map_err(backup_read_error)?;
        if schema_version <= 0 {
            return Err(AppError::BackupInvalid(
                "Yedek dosyasında uygulanmış şema sürümü bulunamadı.".to_owned(),
            ));
        }

        let supported = latest_version();
        if schema_version > supported {
            return Err(AppError::BackupTooNew {
                found: schema_version,
                supported,
            });
        }

        let has_metadata: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master
                  WHERE type = 'table' AND name = 'app_metadata')",
                [],
                |row| row.get(0),
            )
            .map_err(backup_read_error)?;
        if !has_metadata {
            return Err(AppError::BackupWrongApp);
        }

        let app_name: Option<String> = connection
            .query_row(
                "SELECT value FROM app_metadata WHERE key = 'app_name'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(backup_read_error)?;
        if app_name.as_deref() != Some(APP_DB_NAME) {
            return Err(AppError::BackupWrongApp);
        }

        let created_at: Option<String> = connection
            .query_row(
                "SELECT value FROM app_metadata WHERE key = 'created_at'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(backup_read_error)?;

        Ok(BackupInfo {
            path: path.to_path_buf(),
            schema_version,
            app_name: APP_DB_NAME.to_owned(),
            created_at,
        })
    }
}

/// Statik yedek dosyası için SQLite `immutable=1` URI'si üretir.
///
/// Dosya yolu URI kurallarına göre yüzde kodlanır; böylece Türkçe karakterler,
/// boşluklar ve `?`/`#` gibi karakterler yol ayrıştırmasını bozmaz.
/// SQLite başlığının `quick_check` tarafından her zaman yakalanmayan kritik
/// alanlarını doğrular.
///
/// `quick_check` tek başına yazma/okuma sürümü, sayfa boyutu, şema biçimi,
/// metin kodlaması ve ayrılmış başlık baytlarındaki bozulmaları her zaman
/// reddetmez. Yedek geri yükleme öncesinde bu alanlar ayrıca kontrol edilir.
fn validate_sqlite_header(path: &Path) -> Result<()> {
    let mut file = File::open(path)
        .map_err(|error| AppError::BackupInvalid(format!("Yedek dosyası açılamadı: {error}")))?;
    let mut header = [0u8; 100];
    file.read_exact(&mut header).map_err(|error| {
        AppError::BackupInvalid(format!("Yedek dosyası başlığı okunamadı: {error}"))
    })?;

    if &header[0..16] != b"SQLite format 3\0" {
        return Err(AppError::BackupInvalid(
            "Yedek dosyası SQLite başlığı taşımıyor.".to_owned(),
        ));
    }
    let raw_page_size = u16::from_be_bytes([header[16], header[17]]);
    let page_size = if raw_page_size == 1 {
        65_536u32
    } else {
        u32::from(raw_page_size)
    };
    if !(512..=65_536).contains(&page_size) || !page_size.is_power_of_two() {
        return Err(AppError::BackupInvalid(
            "Yedek dosyasında geçersiz sayfa boyutu var.".to_owned(),
        ));
    }
    if !matches!(header[18], 1 | 2) || !matches!(header[19], 1 | 2) {
        return Err(AppError::BackupInvalid(
            "Yedek dosyasında geçersiz SQLite sürüm bilgisi var.".to_owned(),
        ));
    }
    if header[20] != 0 {
        return Err(AppError::BackupInvalid(
            "Yedek dosyasında ayrılmış başlık baytı sıfır değil.".to_owned(),
        ));
    }
    if header[21] != 64 || header[22] != 32 || header[23] != 32 {
        return Err(AppError::BackupInvalid(
            "Yedek dosyasında geçersiz yük oranı var.".to_owned(),
        ));
    }
    let schema_format = u32::from_be_bytes([header[44], header[45], header[46], header[47]]);
    if !(1..=4).contains(&schema_format) {
        return Err(AppError::BackupInvalid(
            "Yedek dosyasında geçersiz şema biçimi var.".to_owned(),
        ));
    }
    let text_encoding = u32::from_be_bytes([header[56], header[57], header[58], header[59]]);
    if !(1..=3).contains(&text_encoding) {
        return Err(AppError::BackupInvalid(
            "Yedek dosyasında geçersiz metin kodlaması var.".to_owned(),
        ));
    }
    if header[72..92].iter().any(|byte| *byte != 0) {
        return Err(AppError::BackupInvalid(
            "Yedek dosyasında ayrılmış başlık alanı bozulmuş.".to_owned(),
        ));
    }
    Ok(())
}

fn immutable_backup_uri(path: &Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    let normalized = if path.starts_with('/') {
        path
    } else {
        format!("/{path}")
    };
    let mut uri = String::from("file:");
    for byte in normalized.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b':' | b'.' | b'-' | b'_' | b'~') {
            uri.push(char::from(byte));
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    uri.push_str("?immutable=1");
    uri
}

/// Yedek okuma hatalarını kullanıcıya anlaşılır biçimde sarmalar.
fn backup_read_error(error: rusqlite::Error) -> AppError {
    AppError::BackupInvalid(format!("Yedek dosyası okunamadı: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::customer::CustomerDraft;
    use crate::db::META_APP_NAME;
    use crate::product::ProductDraft;

    fn open_database() -> (tempfile::TempDir, Database) {
        let directory = tempfile::tempdir().expect("geçici dizin");
        let database =
            Database::open(directory.path().join("on_muhasebe.db")).expect("veritabanı açılmalı");
        (directory, database)
    }

    fn validate(draft: CustomerDraft) -> ValidatedCustomer {
        draft.validate().expect("cari doğrulanmalı")
    }

    fn customer(code: &str, name: &str) -> ValidatedCustomer {
        validate(CustomerDraft {
            code: code.to_owned(),
            name: name.to_owned(),
            ..CustomerDraft::default()
        })
    }

    fn product(code: &str, barcode: &str) -> ValidatedProduct {
        ProductDraft {
            code: code.to_owned(),
            barcode: barcode.to_owned(),
            name: format!("{code} ürünü"),
            unit: crate::product::Unit::Kilogram,
            purchase_price: "1.234,56".to_owned(),
            sale_price: "1.500,00".to_owned(),
            min_stock: "2,5".to_owned(),
            ..ProductDraft::default()
        }
        .validate()
        .expect("ürün doğrulanmalı")
    }

    #[test]
    fn cari_ice_aktarma_ekler_ve_var_olan_kodu_atlar() {
        let (_directory, database) = open_database();
        database
            .customer_insert(&customer("CAR-1", "Mevcut Cari"))
            .unwrap();

        let outcome = database
            .import_customers(&[
                customer("CAR-2", "Yeni Cari"),
                customer("car-1", "Tekrar Cari"),
                customer("CAR-3", "Son Cari"),
            ])
            .unwrap();

        assert_eq!(outcome.inserted, 2);
        assert_eq!(outcome.skipped.len(), 1);
        assert_eq!(outcome.skipped[0].code, "car-1");
        assert_eq!(outcome.skipped[0].reason, "kod zaten mevcut");

        let all = database
            .customer_list(&crate::customer::CustomerFilter::default())
            .unwrap();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn urun_ice_aktarma_sifir_stokla_ekler_ve_barkodu_atlar() {
        let (_directory, database) = open_database();
        database.product_insert(&product("URN-1", "1111")).unwrap();

        let outcome = database
            .import_products(&[
                product("URN-2", "2222"),
                product("URN-3", "1111"),
                product("URN-1", ""),
            ])
            .unwrap();

        assert_eq!(outcome.inserted, 1);
        assert_eq!(outcome.skipped.len(), 2);
        assert_eq!(outcome.skipped[0].reason, "barkod zaten mevcut");
        assert_eq!(outcome.skipped[1].reason, "kod zaten mevcut");

        let imported = database
            .product_get(
                database
                    .product_list(&crate::product::ProductFilter::default())
                    .unwrap()
                    .into_iter()
                    .find(|product| product.code == "URN-2")
                    .unwrap()
                    .id,
            )
            .unwrap()
            .unwrap();
        assert_eq!(imported.current_stock, crate::quantity::Quantity::ZERO);
        assert_eq!(imported.purchase_price.kurus(), 123_456);
        let movements = database.movement_list_for_product(imported.id).unwrap();
        assert!(movements.is_empty());
    }

    #[test]
    fn ice_aktarma_hatasinda_tum_partiyi_geri_alir() {
        let (_directory, database) = open_database();
        database
            .connection
            .execute_batch(
                "CREATE TRIGGER test_rollback BEFORE INSERT ON customers
                 WHEN NEW.code = 'HATA'
                 BEGIN SELECT RAISE(ABORT, 'test hatası'); END;",
            )
            .unwrap();

        let result = database
            .import_customers(&[customer("ILK", "İlk Cari"), customer("HATA", "Hatalı Cari")]);

        assert!(result.is_err(), "tetikleyici hata üretmeli");
        let all = database
            .customer_list(&crate::customer::CustomerFilter::default())
            .unwrap();
        assert!(all.is_empty(), "ilk satır da geri alınmalı");
    }

    #[test]
    fn yeni_veritabaninda_uygulama_adi_yazilir() {
        let (_directory, database) = open_database();
        assert_eq!(
            database.metadata(META_APP_NAME).unwrap().as_deref(),
            Some(APP_DB_NAME)
        );
    }

    fn valid_backup(database: &Database, path: &Path) {
        database.checkpoint().unwrap();
        std::fs::copy(database.path().unwrap(), path).unwrap();
    }

    #[test]
    fn gecerli_yedek_sema_ve_uygulama_adiyla_dogrulanir() {
        let (directory, database) = open_database();
        let path = directory.path().join("yedek.onm-backup");
        valid_backup(&database, &path);

        let info = Database::validate_backup(&path).unwrap();
        assert_eq!(info.schema_version, latest_version());
        assert_eq!(info.app_name, "OnMuhasebe");
        assert!(info.created_at.is_some());
        assert!(
            !PathBuf::from(format!("{}-wal", path.display())).exists(),
            "doğrulama yedek klasöründe -wal dosyası bırakmamalı"
        );
        assert!(
            !PathBuf::from(format!("{}-shm", path.display())).exists(),
            "doğrulama yedek klasöründe -shm dosyası bırakmamalı"
        );
    }

    #[test]
    fn eski_surum_yedegi_dogrulanir_ve_acilirken_yukseltilir() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("eski.onm-backup");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (
                     version INTEGER PRIMARY KEY NOT NULL,
                     name TEXT NOT NULL,
                     applied_at TEXT NOT NULL DEFAULT (datetime('now'))
                 );",
            )
            .unwrap();
        for migration in crate::db::MIGRATIONS
            .iter()
            .filter(|migration| migration.version <= 5)
        {
            connection.execute_batch(migration.sql).unwrap();
            connection
                .execute(
                    "INSERT INTO schema_migrations (version, name) VALUES (?1, ?2)",
                    params![migration.version, migration.name],
                )
                .unwrap();
        }
        connection
            .execute(
                "INSERT INTO app_metadata (key, value) VALUES ('app_name', ?1)",
                [APP_DB_NAME],
            )
            .unwrap();
        drop(connection);

        let info = Database::validate_backup(&path).unwrap();
        assert_eq!(info.schema_version, 5);

        let database = Database::open(&path).expect("eski yedek yükseltilerek açılmalı");
        assert_eq!(
            database
                .applied_migrations()
                .unwrap()
                .last()
                .map(|migration| migration.version),
            Some(latest_version())
        );
        assert_eq!(
            database.metadata(META_APP_NAME).unwrap().as_deref(),
            Some(APP_DB_NAME)
        );
    }

    #[test]
    fn bos_txt_uzantili_dosya_gecerli_yedek_sayilmaz() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("sahte.onm-backup");
        std::fs::write(&path, b"bu bir yedek degil").unwrap();

        let error = Database::validate_backup(&path).unwrap_err();
        assert!(matches!(error, AppError::BackupInvalid(_)));
    }

    #[test]
    fn baska_uygulamanin_veritabani_reddedilir() {
        let (directory, database) = open_database();
        let path = directory.path().join("baska.onm-backup");
        valid_backup(&database, &path);
        let connection = Connection::open(&path).unwrap();
        connection
            .execute(
                "UPDATE app_metadata SET value = 'BaskaUygulama' WHERE key = 'app_name'",
                [],
            )
            .unwrap();
        drop(connection);

        let error = Database::validate_backup(&path).unwrap_err();
        assert!(matches!(error, AppError::BackupWrongApp));
    }

    #[test]
    fn daha_yeni_sema_reddedilir_ve_mesaj_anlasilirdir() {
        let (directory, database) = open_database();
        let path = directory.path().join("yeni.onm-backup");
        valid_backup(&database, &path);
        let connection = Connection::open(&path).unwrap();
        connection
            .execute(
                "INSERT INTO schema_migrations (version, name) VALUES (999, 'gelecek')",
                [],
            )
            .unwrap();
        drop(connection);

        let error = Database::validate_backup(&path).unwrap_err();
        assert!(matches!(error, AppError::BackupTooNew { .. }));
        assert_eq!(
            error.to_string(),
            "Bu yedek, programın daha yeni bir sürümüyle oluşturulmuş. Önce programı güncelleyin."
        );
    }

    #[test]
    fn sema_tablosu_olmayan_dosya_reddedilir() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("sema-yok.onm-backup");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE app_metadata (
                     key TEXT PRIMARY KEY NOT NULL,
                     value TEXT NOT NULL,
                     updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                 );
                 INSERT INTO app_metadata (key, value) VALUES ('app_name', 'OnMuhasebe');",
            )
            .unwrap();
        drop(connection);

        let error = Database::validate_backup(&path).unwrap_err();
        assert!(matches!(error, AppError::BackupInvalid(_)));
    }
}
