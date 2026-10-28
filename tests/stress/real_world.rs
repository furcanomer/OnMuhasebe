// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Aşama 13 gerçek dünya senaryoları: G1, G5, G8 ve G9.
//!
//! Süreç öldürme testleri `#[ignore]` işaretlidir; hızlı hata/saat testleri
//! normal `cargo test --all-targets` çalışmasında da doğrulanır.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

use chrono::{Local, TimeDelta};
use on_muhasebe::auth::{hash_password, verify_password};
use on_muhasebe::config::AppConfig;
use on_muhasebe::db::Database;
use on_muhasebe::error::AppError;
use on_muhasebe::settings::AppSettings;
use rusqlite::{Connection, params};

use super::{
    assert_correlations, build_test_db, kill_process_hard, raw_connection, seed_minimal,
    spawn_app_process, wait_for_condition,
};

fn wal_path(database_path: &Path) -> PathBuf {
    PathBuf::from(format!("{}-wal", database_path.display()))
}

fn app_started(config: &AppConfig) -> bool {
    config.logs_dir().exists()
}

fn seed_uncommitted_invoice(connection: &Connection, customer_id: i64) {
    connection
        .execute_batch(&format!(
            "BEGIN IMMEDIATE;
             INSERT INTO invoices
                 (invoice_number, invoice_type, customer_id, invoice_date,
                  subtotal, discount_total, net_total, vat_total, grand_total, status)
             VALUES ('G1-YARIM', 'satis', {customer_id}, '2026-02-01',
                     10000, 0, 10000, 2000, 12000, 'kesin');"
        ))
        .expect("yarıda kalan fatura transaction'ı başlatılmalı");
}

fn insert_customer(connection: &Connection, code: &str, name: &str) -> rusqlite::Result<()> {
    connection.execute(
        "INSERT INTO customers (code, name, kind) VALUES (?1, ?2, 'musteri')",
        params![code, name],
    )?;
    Ok(())
}

/// G1: Süreç, commit edilmemiş bir fatura transaction'ı sırasında öldürülür.
#[test]
#[ignore = "ağır test: gerçek süreç kill + WAL kurtarma"]
fn g1_yarida_kalan_transaction_surec_kill() {
    let mut durations = Vec::new();
    for run in 0..5 {
        let directory = tempfile::tempdir().expect("geçici dizin");
        let config = AppConfig::with_data_dir(directory.path().join("veri"));
        config.ensure_data_dir().expect("veri dizini");
        let database = build_test_db(config.database_path());
        let seed = seed_minimal(&database);
        drop(database);

        let child = spawn_app_process(config.data_dir());
        assert!(
            wait_for_condition(|| app_started(&config), 10_000),
            "uygulama {run}. turda açılmadı"
        );
        thread::sleep(Duration::from_millis(200));

        let start = Instant::now();
        let connection = Connection::open(config.database_path()).expect("test bağlantısı");
        connection
            .busy_timeout(Duration::from_secs(5))
            .expect("busy_timeout");
        seed_uncommitted_invoice(&connection, seed.customer_id);
        thread::sleep(Duration::from_millis(50));
        kill_process_hard(child);
        drop(connection);
        durations.push(start.elapsed().as_millis());

        let database = build_test_db(config.database_path());
        assert_eq!(
            database.invoice_count().expect("fatura sayısı"),
            0,
            "G1 {run}. tur: commit edilmemiş fatura kurtarılmamalı"
        );
        assert_correlations(&database);
        println!(
            "[G1] tur {}: süreç öldürüldü, yarım fatura yok ({} ms)",
            run + 1,
            durations[run]
        );
    }
    assert_eq!(durations.len(), 5);
    println!("[G1] 5/5 geçti; süreler: {durations:?} ms");
}

/// G1b: WAL dosyası checkpoint edilmeden süreç öldürülür; WAL kurtarma çalışmalı.
#[test]
#[ignore = "ağır test: WAL dosyası varken süreç kill"]
fn g1b_wal_varken_surec_kill() {
    let directory = tempfile::tempdir().expect("geçici dizin");
    let config = AppConfig::with_data_dir(directory.path().join("veri"));
    config.ensure_data_dir().expect("veri dizini");
    let database = build_test_db(config.database_path());
    let _seed = seed_minimal(&database);

    let connection = Connection::open(config.database_path()).expect("WAL bağlantısı");
    connection
        .busy_timeout(Duration::from_secs(5))
        .expect("busy_timeout");
    connection
        .execute_batch(
            "PRAGMA wal_autocheckpoint = 0;
             INSERT INTO customers (code, name, kind)
                 VALUES ('G1B-WAL', 'WAL Kaydı', 'musteri');",
        )
        .expect("WAL kaydı yazılmalı");
    assert!(
        wait_for_condition(|| wal_path(config.database_path()).exists(), 5_000),
        "WAL dosyası oluşmadı"
    );

    let child = spawn_app_process(config.data_dir());
    assert!(wait_for_condition(|| app_started(&config), 10_000));
    thread::sleep(Duration::from_millis(200));
    kill_process_hard(child);
    drop(connection);

    let database = build_test_db(config.database_path());
    let count = database.customer_count().expect("cari sayısı");
    assert_eq!(count, 2, "WAL'daki cari kurtarılmalı");
    assert_correlations(&database);
    println!("[G1b] WAL kurtarma başarılı; cari sayısı: {count}");
}

/// G5: Aynı veritabanına iki uygulama süreci ve iki eşzamanlı yazma.
#[test]
#[ignore = "ağır test: iki uygulama süreci + eşzamanlı SQLite yazması"]
fn g5_iki_uygulama_ayni_veritabani() {
    let directory = tempfile::tempdir().expect("geçici dizin");
    let config = AppConfig::with_data_dir(directory.path().join("veri"));
    config.ensure_data_dir().expect("veri dizini");
    let database = build_test_db(config.database_path());
    let seed = seed_minimal(&database);
    let hash_a = hash_password("GucluSifre1").expect("şifre özeti");
    let hash_b = hash_password("GucluSifre2").expect("şifre özeti");
    {
        let connection = raw_connection(&database);
        connection
            .execute(
                "INSERT INTO users (username, password_hash, role, active)
                 VALUES ('g5a', ?1, 'yonetici', 1)",
                [&hash_a],
            )
            .expect("kullanıcı A");
        connection
            .execute(
                "INSERT INTO users (username, password_hash, role, active)
                 VALUES ('g5b', ?1, 'operator', 1)",
                [&hash_b],
            )
            .expect("kullanıcı B");
    }
    drop(database);

    let first = spawn_app_process(config.data_dir());
    let second = spawn_app_process(config.data_dir());
    assert!(
        wait_for_condition(|| app_started(&config), 10_000),
        "uygulama süreçleri açılmadı"
    );
    thread::sleep(Duration::from_millis(500));

    let barrier = Arc::new(Barrier::new(2));
    let mut handles = Vec::new();
    for (index, timeout_ms) in [(0usize, 5_000u64), (1, 100)] {
        let path = config.database_path().to_path_buf();
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            let connection = Connection::open(path).expect("eşzamanlı bağlantı");
            connection
                .busy_timeout(Duration::from_millis(timeout_ms))
                .expect("busy_timeout");
            barrier.wait();
            let result = (|| -> rusqlite::Result<()> {
                connection.execute_batch("BEGIN IMMEDIATE;")?;
                insert_customer(
                    &connection,
                    &format!("G5-{index}"),
                    &format!("Eşzamanlı Cari {index}"),
                )?;
                thread::sleep(Duration::from_millis(200));
                connection.execute_batch("COMMIT;")?;
                Ok(())
            })();
            result.map_err(|error| error.to_string())
        }));
    }

    let results: Vec<std::result::Result<(), String>> = handles
        .into_iter()
        .map(|handle| handle.join().expect("yazma iş parçacığı"))
        .collect();
    let successes = results.iter().filter(|result| result.is_ok()).count();
    assert!(
        successes >= 1,
        "en az bir eşzamanlı yazma başarılı olmalı: {results:?}"
    );
    for result in &results {
        if let Err(error) = result {
            let lower = error.to_ascii_lowercase();
            assert!(
                lower.contains("locked") || lower.contains("busy"),
                "beklenmeyen eşzamanlı yazma hatası: {error}"
            );
        }
    }

    let login_barrier = Arc::new(Barrier::new(2));
    let mut logins = Vec::new();
    for (username, password) in [("g5a", "GucluSifre1"), ("g5b", "GucluSifre2")] {
        let path = config.database_path().to_path_buf();
        let barrier = Arc::clone(&login_barrier);
        logins.push(thread::spawn(move || {
            let database = Database::open(path).expect("giriş bağlantısı");
            barrier.wait();
            let user = database
                .user_by_username(username)
                .expect("kullanıcı okunmalı")
                .expect("kullanıcı bulunmalı");
            let valid = verify_password(&user.password_hash, password);
            if valid {
                database.user_set_last_login(user.id).expect("son giriş");
            }
            valid
        }));
    }
    for login in logins {
        assert!(
            login.join().expect("giriş iş parçacığı"),
            "iki kullanıcı da giriş yapabilmeli"
        );
    }

    kill_process_hard(first);
    kill_process_hard(second);

    let database = build_test_db(config.database_path());
    let expected = 1 + successes;
    assert_eq!(
        database.customer_count().expect("cari sayısı"),
        expected,
        "başarılı yazmaların tamamı korunmalı"
    );
    assert_correlations(&database);
    let _ = seed;
    println!("[G5] eşzamanlı yazma: {successes}/2 başarılı; veri bozulmadı");
}

/// G8: SQLite `max_page_count` ile disk dolu simülasyonu ve rollback.
#[test]
fn g8_disk_dolu_max_page_count_rollback() {
    let directory = tempfile::tempdir().expect("geçici dizin");
    let database = build_test_db(&directory.path().join("disk.db"));
    let _seed = seed_minimal(&database);
    let connection = raw_connection(&database);

    let page_count: i64 = connection
        .query_row("PRAGMA page_count", [], |row| row.get(0))
        .expect("page_count");
    connection
        .execute_batch(&format!("PRAGMA max_page_count = {};", page_count + 1))
        .expect("max_page_count sınırı");

    let transaction = connection
        .unchecked_transaction()
        .expect("disk dolu transaction'ı");
    let mut full_error = None;
    for index in 0..10_000usize {
        let result = transaction.execute(
            "INSERT INTO customers (code, name, kind) VALUES (?1, ?2, 'musteri')",
            params![format!("FULL-{index:05}"), "X".repeat(2048)],
        );
        if let Err(error) = result {
            full_error = Some(error);
            break;
        }
    }
    drop(transaction);
    let error = full_error.expect("max_page_count sınırı aşıldığında yazma reddedilmeli");
    let app_error = AppError::from(error);
    let message = app_error.to_string();
    assert!(
        message.contains("Disk dolu"),
        "disk dolu hatası Türkçe ve anlaşılır olmalı: {message}"
    );

    let customer_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM customers", [], |row| row.get(0))
        .expect("cari sayısı");
    assert_eq!(customer_count, 1, "başarısız transaction geri alınmalı");
    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .expect("integrity_check");
    assert_eq!(integrity, "ok");
    assert_correlations(&database);
    println!("[G8] disk dolu: yazma reddedildi, rollback tamam; mesaj: {message}");
}

/// G9: Sistem saati geri alınmış gibi `last_auto_backup_at` gelecekte.
#[test]
fn g9_sistem_saati_kaymasi() {
    let now = Local::now().naive_local();
    let settings = AppSettings {
        last_auto_backup_at: Some(now + TimeDelta::days(365)),
        ..AppSettings::default()
    };

    assert!(
        settings.clock_skew_detected(now),
        "gelecekteki yedek zamanı saat kayması olarak tespit edilmeli"
    );
    assert!(
        !settings.should_auto_backup(now),
        "saat kaymasında otomatik yedek atlanmalı"
    );

    let directory = tempfile::tempdir().expect("geçici dizin");
    let database = build_test_db(&directory.path().join("saat.db"));
    let _seed = seed_minimal(&database);
    database
        .setting_set(
            "last_auto_backup_at",
            &(now + TimeDelta::days(365))
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
        )
        .expect("ayar yazılmalı");
    let loaded = AppSettings::load(&database).expect("ayar okunmalı");
    assert!(loaded.clock_skew_detected(now));
    assert!(!loaded.should_auto_backup(now));
    assert_correlations(&database);
    println!("[G9] saat kayması: otomatik yedek atlandı + uyarı yolu etkin");
}
