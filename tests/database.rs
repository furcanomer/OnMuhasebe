// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Veritabanı katmanının uçtan uca (entegrasyon) testleri.
//!
//! Bu testler gerçek bir dosya üzerinde çalışır; böylece veritabanı dosyasının
//! gerçekten oluşturulduğu, şemanın kurulduğu ve verinin kalıcı olduğu doğrulanır.

use on_muhasebe::config::AppConfig;
use on_muhasebe::db::{
    Database, META_CREATED_AT, META_LAST_APP_VERSION, MIGRATIONS, latest_version,
};
use tempfile::TempDir;

/// Geçici veri dizini ve bu dizine bağlı yapılandırma üretir.
fn temporary_config() -> (TempDir, AppConfig) {
    let directory = tempfile::tempdir().expect("geçici dizin oluşturulmalı");
    let config = AppConfig::with_data_dir(directory.path().join("veri"));
    (directory, config)
}

#[test]
fn ilk_acilista_veritabani_dosyasi_ve_semasi_olusturulur() {
    let (_directory, config) = temporary_config();

    assert!(
        !config.database_path().exists(),
        "veritabanı dosyası henüz oluşturulmamış olmalı"
    );

    let database = Database::open(config.database_path()).expect("veritabanı açılmalı");

    assert!(
        config.database_path().is_file(),
        "veritabanı dosyası oluşturulmuş olmalı: {}",
        config.database_path().display()
    );
    assert_eq!(database.schema_version().unwrap(), latest_version());
    assert!(
        database.user_table_count().unwrap() >= 2,
        "schema_migrations ve app_metadata tabloları oluşturulmalı"
    );
}

#[test]
fn veriler_yeniden_acilista_korunur_ve_migration_tekrarlanmaz() {
    let (_directory, config) = temporary_config();

    let first_open_time = {
        let database = Database::open(config.database_path()).expect("ilk açılış");
        database
            .set_metadata("deneme_anahtari", "kalici_deger")
            .expect("meta veri yazılmalı");
        database
            .metadata(META_CREATED_AT)
            .unwrap()
            .expect("oluşturulma zamanı yazılmalı")
    };

    // İkinci açılış: migration tekrar uygulanmamalı, veri korunmalı.
    let database = Database::open(config.database_path()).expect("ikinci açılış");

    assert_eq!(database.schema_version().unwrap(), latest_version());
    assert_eq!(
        database.applied_migrations().unwrap().len(),
        MIGRATIONS.len()
    );
    assert_eq!(
        database.metadata("deneme_anahtari").unwrap().as_deref(),
        Some("kalici_deger")
    );
    assert_eq!(
        database.metadata(META_CREATED_AT).unwrap().as_deref(),
        Some(first_open_time.as_str()),
        "oluşturulma zamanı değişmemeli"
    );
}

#[test]
fn durum_ozeti_arayuz_icin_gerekli_bilgileri_saglar() {
    let (_directory, config) = temporary_config();
    let database = Database::open(config.database_path()).expect("veritabanı açılmalı");

    let status = database.status().expect("durum okunmalı");

    assert_eq!(status.path.as_deref(), Some(config.database_path()));
    assert!(
        status.file_size_bytes.unwrap_or(0) > 0,
        "dosya boyutu sıfırdan büyük olmalı"
    );
    assert_eq!(status.schema_version, latest_version());
    assert_eq!(status.supported_schema_version, latest_version());
    assert_eq!(status.applied_migrations.len(), MIGRATIONS.len());
    assert!(status.created_at.is_some());
    assert!(status.user_table_count >= 2);
}

#[test]
fn ayni_dosyaya_iki_baglanti_acilabilir() {
    let (_directory, config) = temporary_config();

    let first = Database::open(config.database_path()).expect("birinci bağlantı");
    let second = Database::open(config.database_path()).expect("ikinci bağlantı");

    assert_eq!(
        first.schema_version().unwrap(),
        second.schema_version().unwrap()
    );
    assert_eq!(
        first.metadata(META_LAST_APP_VERSION).unwrap(),
        second.metadata(META_LAST_APP_VERSION).unwrap()
    );
}

#[test]
fn veri_dizini_yoksa_otomatik_olusturulur() {
    let (_directory, config) = temporary_config();

    assert!(!config.data_dir().exists(), "dizin henüz yok");

    let _database = Database::open(config.database_path()).expect("veritabanı açılmalı");

    assert!(config.data_dir().is_dir(), "veri dizini oluşturulmalı");
}
