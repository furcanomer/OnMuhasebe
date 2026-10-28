// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Hata enjeksiyonu testleri (F1-F8).

use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Barrier};
use std::thread;

use on_muhasebe::customer::CustomerDraft;
use on_muhasebe::db::Database;
use on_muhasebe::error::AppError;
use on_muhasebe::invoice::{InvoiceDraft, InvoiceLineDraft, InvoiceType};
use on_muhasebe::money::Money;
use on_muhasebe::product::Unit;
use on_muhasebe::quantity::Quantity;
use on_muhasebe::report::validate_date_range;

use super::{build_test_db, raw_connection, seed_minimal};

struct RestoreWritePermission {
    path: PathBuf,
    original: std::fs::Permissions,
}

impl Drop for RestoreWritePermission {
    fn drop(&mut self) {
        let _ = std::fs::set_permissions(&self.path, self.original.clone());
    }
}

fn valid_customer(code: &str, name: &str) -> on_muhasebe::customer::ValidatedCustomer {
    CustomerDraft {
        code: code.to_owned(),
        name: name.to_owned(),
        ..CustomerDraft::default()
    }
    .validate()
    .expect("cari doğrulanmalı")
}

/// F1: Yazmaya kapalı veritabanı dosyası panik üretmeden hata döndürmeli.
#[test]
fn f1_disk_yazma_hatasi_panik_yok() {
    let directory = tempfile::tempdir().expect("geçici dizin");
    let path = directory.path().join("readonly.db");
    let database = build_test_db(&path);
    seed_minimal(&database);
    database.checkpoint().expect("checkpoint");
    drop(database);

    let original = std::fs::metadata(&path)
        .expect("dosya bilgisi")
        .permissions();
    let mut permissions = original.clone();
    permissions.set_readonly(true);
    std::fs::set_permissions(&path, permissions).expect("salt okunur yapılmalı");
    let _guard = RestoreWritePermission {
        path: path.clone(),
        original,
    };

    let open_result = Database::open(&path);
    match open_result {
        Err(error) => {
            println!("[F1] açılış beklenen şekilde reddedildi: {error}");
        }
        Ok(database) => {
            let result = database.customer_insert(&valid_customer("CAR-RO", "Salt Okunur"));
            assert!(
                result.is_err(),
                "salt okunur veritabanında yazma başarılı olmamalı"
            );
            println!(
                "[F1] yazma beklenen şekilde reddedildi: {}",
                result.unwrap_err()
            );
        }
    }
}

/// F2: Fatura transaction'ının ortasında hata; kısmi kayıt kalmamalı.
#[test]
fn f2_yarida_kalan_fatura_geri_alnir() {
    let directory = tempfile::tempdir().expect("geçici dizin");
    let path = directory.path().join("rollback.db");
    let database = build_test_db(&path);
    let seed = seed_minimal(&database);
    let before_stock = database
        .product_get(seed.product_id)
        .unwrap()
        .unwrap()
        .current_stock;

    let mut draft = InvoiceDraft::new(InvoiceType::Sales);
    draft.number = "SF-ROLLBACK-1".to_owned();
    draft.customer_id = Some(seed.customer_id);
    draft.invoice_date = "2026-10-23".to_owned();
    draft.lines = vec![
        InvoiceLineDraft {
            product_id: Some(seed.product_id),
            product_code: "URN-0001".to_owned(),
            product_name: "Test Ürün".to_owned(),
            unit: Unit::Adet,
            quantity: "1".to_owned(),
            unit_price: "100,00".to_owned(),
            ..InvoiceLineDraft::default()
        },
        InvoiceLineDraft {
            product_id: Some(seed.product_id + 999_999),
            product_code: "YOK".to_owned(),
            product_name: "Olmayan Ürün".to_owned(),
            unit: Unit::Adet,
            quantity: "1".to_owned(),
            unit_price: "100,00".to_owned(),
            ..InvoiceLineDraft::default()
        },
    ];
    let validated = draft.validate().expect("fatura doğrulanmalı");
    let result = database.invoice_insert(&validated);
    assert!(result.is_err(), "olmayan ürünle fatura yazılamamalı");
    println!("[F2] beklenen hata: {}", result.unwrap_err());

    assert_eq!(database.invoice_count().unwrap(), 0);
    let line_count: i64 = raw_connection(&database)
        .query_row("SELECT COUNT(*) FROM invoice_lines", [], |row| row.get(0))
        .unwrap();
    assert_eq!(line_count, 0, "kısmi fatura satırı kalmamalı");
    let after_stock = database
        .product_get(seed.product_id)
        .unwrap()
        .unwrap()
        .current_stock;
    assert_eq!(before_stock, after_stock, "stok geri alınmalı");
    drop(database);
    Database::open(&path).expect("veritabanı yeniden açılabilmeli");
}

/// F3: İlk 16 baytı bozuk veritabanı anlaşılır hata vermeli, panik olmamalı.
#[test]
fn f3_bozuk_veritabani_panik_yok() {
    let directory = tempfile::tempdir().expect("geçici dizin");
    let path = directory.path().join("corrupt.db");
    let database = build_test_db(&path);
    seed_minimal(&database);
    database.checkpoint().expect("checkpoint");
    drop(database);

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .expect("dosya açılmalı");
    file.write_all(&[0xFF; 16]).expect("başlık bozulmalı");
    file.flush().expect("yazma");
    drop(file);

    let result = Database::open(&path);
    assert!(result.is_err(), "bozuk veritabanı açılmamalı");
    println!("[F3] açılış hatası: {}", result.err().unwrap());

    if let Ok(connection) = rusqlite::Connection::open(&path) {
        let checkpoint = connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
        let _ = checkpoint;
    }
}

/// F4: Bozuk WAL dosyası panik üretmeden kurtarılmalı veya reddedilmeli.
#[test]
fn f4_bozuk_wal_panik_yok() {
    let directory = tempfile::tempdir().expect("geçici dizin");
    let path = directory.path().join("wal.db");
    let database = build_test_db(&path);
    seed_minimal(&database);

    {
        let connection = raw_connection(&database);
        connection
            .execute(
                "INSERT INTO customers (code, name, kind) VALUES ('WAL-1', 'WAL Deneme', 'musteri')",
                [],
            )
            .expect("WAL üretilmeli");
    }

    let wal_path = PathBuf::from(format!("{}-wal", path.display()));
    let copy_path = directory.path().join("wal-copy.db");
    let copy_wal = PathBuf::from(format!("{}-wal", copy_path.display()));
    std::fs::copy(&path, &copy_path).expect("ana dosya kopyalanmalı");
    std::fs::copy(&wal_path, &copy_wal).expect("WAL kopyalanmalı");
    assert!(
        std::fs::metadata(&copy_wal).unwrap().len() > 16,
        "WAL kopyası boş olmamalı"
    );

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .open(&copy_wal)
        .expect("WAL açılmalı");
    file.write_all(&[0xAA; 16]).expect("WAL başlığı bozulmalı");
    file.flush().expect("yazma");
    drop(file);

    let result = Database::open(&copy_path);
    match result {
        Ok(database) => {
            println!("[F4] SQLite bozuk WAL'ı atlayarak açtı");
            let _ = database.checkpoint();
        }
        Err(error) => println!("[F4] SQLite bozuk WAL'ı reddetti: {error}"),
    }

    // Bozuk WAL üzerinde checkpoint çağrısı da paniklememeli.
    if let Ok(connection) = rusqlite::Connection::open(&copy_path) {
        let _ = connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
    }
}

/// F5: İki bağlantı aynı cari kodunu eklemeye çalışırsa UNIQUE kısıtı korur.
#[test]
fn f5_ayni_kod_iki_baglantida_tek_basarili() {
    let directory = tempfile::tempdir().expect("geçici dizin");
    let path = directory.path().join("race.db");
    let database = build_test_db(&path);
    drop(database);

    let barrier = Arc::new(Barrier::new(2));
    let mut handles = Vec::new();
    for index in 0..2 {
        let path = path.clone();
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            let database = Database::open(&path).expect("bağlantı açılmalı");
            barrier.wait();
            let customer = valid_customer("CAR-RACE", &format!("Yarış {index}"));
            database.customer_insert(&customer)
        }));
    }

    let results: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().expect("thread paniklememeli"))
        .collect();
    assert_eq!(
        results.iter().filter(|result| result.is_ok()).count(),
        1,
        "tam olarak bir bağlantı başarılı olmalı"
    );
    let error = results
        .iter()
        .find_map(|result| result.as_ref().err())
        .expect("bir hata olmalı");
    assert!(
        matches!(error, AppError::CustomerCodeTaken(code) if code == "CAR-RACE"),
        "beklenen CustomerCodeTaken, gelen: {error}"
    );
}

/// F6: i64 sınırlarında taşma hata döndürmeli, panik olmamalı.
#[test]
fn f6_asiri_buyuk_deger_tasmasi() {
    let money = Money::from_kurus(i64::MAX);
    assert!(matches!(
        money.checked_add(Money::from_kurus(1)),
        Err(AppError::AmountOverflow)
    ));
    assert!(matches!(
        Money::from_kurus(i64::MIN).checked_neg(),
        Err(AppError::AmountOverflow)
    ));

    let quantity = Quantity::from_milli(i64::MAX);
    assert!(matches!(
        quantity.checked_add(Quantity::from_milli(1)),
        Err(AppError::QuantityOverflow)
    ));
    assert!(matches!(
        Quantity::from_milli(i64::MIN).checked_neg(),
        Err(AppError::QuantityOverflow)
    ));
}

/// F7: Başlangıç > bitiş tarih aralığı anlaşılır hata vermeli.
#[test]
fn f7_ters_tarih_araligi() {
    let error =
        validate_date_range("2026-12-31", "2026-01-01").expect_err("ters aralık reddedilmeli");
    assert!(error.contains("Başlangıç"), "anlaşılır mesaj: {error}");
}

/// F8: schema_migrations zincirinden v5 silinirse veri bozulmadan açık hata.
#[test]
fn f8_migration_zinciri_kopuklugu() {
    let directory = tempfile::tempdir().expect("geçici dizin");
    let path = directory.path().join("chain.db");
    let database = build_test_db(&path);
    seed_minimal(&database);
    database.checkpoint().expect("checkpoint");
    drop(database);

    let connection = rusqlite::Connection::open(&path).expect("bağlantı");
    connection
        .execute("DELETE FROM schema_migrations WHERE version = 5", [])
        .expect("v5 kaydı silinmeli");
    drop(connection);

    let result = Database::open(&path);
    assert!(result.is_err(), "kopuk migration zinciri açılmamalı");
    let error = result.err().unwrap();
    println!("[F8] açılış hatası: {error}");
    assert!(
        matches!(error, AppError::MigrationChainBroken { missing: 5 }),
        "beklenen MigrationChainBroken(5), gelen: {error}"
    );

    let connection = rusqlite::Connection::open(&path).expect("veri dosyası açılmalı");
    let customers: i64 = connection
        .query_row("SELECT COUNT(*) FROM customers", [], |row| row.get(0))
        .expect("cari sayısı");
    assert_eq!(customers, 1, "mevcut veri korunmalı");
    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .expect("integrity");
    assert_eq!(integrity, "ok");
}
