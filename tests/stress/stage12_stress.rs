// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Aşama 12 ağır performans testleri.
//!
//! Bu testler `#[ignore]` ile işaretlidir; yalnızca release modunda ve
//! `--ignored --test-threads=1` ile çalıştırılır.

use std::fs;
use std::time::Instant;

use on_muhasebe::db::Database;
use on_muhasebe::ui::virtual_list::visible_range;
use rusqlite::types::Value;

use super::bulk_data::seed_bulk;
use super::{build_test_db, insert_batch, raw_connection};

/// Arama testi için 100.000 fatura ve 100.000 ödeme üretir.
///
/// Fatura satırları ve stok hareketleri arama performansını etkilemediği için
/// yalnızca arama sorgularının dokunduğu kolonlar doldurulur.
fn seed_invoices_and_payments(database: &Database, count: usize) {
    const BATCH: usize = 1000;

    let connection = raw_connection(database);
    connection
        .execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = OFF;
             PRAGMA temp_store = MEMORY;
             PRAGMA cache_size = -200000;",
        )
        .expect("toplu yazma ayarları");
    let transaction = connection
        .unchecked_transaction()
        .expect("fatura/ödeme işlemi başlatılmalı");
    let cash_account_id: i64 = transaction
        .query_row(
            "SELECT id FROM cash_accounts ORDER BY id LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("kasa hesabı bulunmalı");

    let mut inserted = 0;
    while inserted < count {
        let chunk = (count - inserted).min(BATCH);
        let invoice_rows: Vec<Vec<Value>> = (0..chunk)
            .map(|offset| {
                let index = inserted + offset + 1;
                let customer_id = ((index - 1) % 1_000_000) as i64 + 1;
                vec![
                    Value::Text(format!("SF-{index:07}")),
                    Value::Text("satis".to_owned()),
                    Value::Integer(customer_id),
                    Value::Text("2026-01-15".to_owned()),
                    Value::Integer(10_000),
                    Value::Integer(0),
                    Value::Integer(10_000),
                    Value::Integer(2_000),
                    Value::Integer(12_000),
                    Value::Text("kesin".to_owned()),
                ]
            })
            .collect();
        insert_batch(
            &transaction,
            "invoices",
            "invoice_number, invoice_type, customer_id, invoice_date, subtotal, \
             discount_total, net_total, vat_total, grand_total, status",
            &invoice_rows,
        );

        let payment_rows: Vec<Vec<Value>> = (0..chunk)
            .map(|offset| {
                let index = inserted + offset + 1;
                let customer_id = ((index - 1) % 1_000_000) as i64 + 1;
                vec![
                    Value::Text("tahsilat".to_owned()),
                    Value::Integer(customer_id),
                    Value::Integer(cash_account_id),
                    Value::Text("2026-01-15".to_owned()),
                    Value::Integer(1_000),
                    Value::Text("stress ödemesi".to_owned()),
                    Value::Text("kesin".to_owned()),
                ]
            })
            .collect();
        insert_batch(
            &transaction,
            "payments",
            "payment_type, customer_id, cash_account_id, payment_date, amount, \
             description, status",
            &payment_rows,
        );

        inserted += chunk;
    }

    transaction
        .commit()
        .expect("fatura/ödeme işlemi commit edilmeli");
}

#[test]
#[ignore = "boş veritabanı açılış süresi ölçülmeli"]
fn bos_veritabani_acilis_suresi() {
    for run in 1..=3 {
        let directory = tempfile::tempdir().expect("geçici dizin");
        let path = directory.path().join(format!("bos-{run}.db"));
        let start = Instant::now();
        let database = Database::open(&path).expect("boş veritabanı açılmalı");
        let elapsed = start.elapsed().as_millis();
        println!("[STAGE12] boş veritabanı açılışı {run}: {elapsed} ms");
        assert_eq!(database.schema_version().expect("şema sürümü"), 7);
        assert!(elapsed < 2_000, "boş veritabanı açılışı 2 sn'yi aştı");
    }
}

#[test]
#[ignore = "1.000.000 cari/ürün + 100.000 fatura/ödeme ile arama 500 ms altında mı?"]
fn bir_milyon_cari_arama_500ms_altinda() {
    let directory = tempfile::tempdir().expect("geçici dizin");
    let database = build_test_db(&directory.path().join("arama.db"));

    let seed_start = Instant::now();
    seed_bulk(&database, 1_000_000, 1_000_000, 0, 0, 0);
    seed_invoices_and_payments(&database, 100_000);
    let connection = raw_connection(&database);
    connection
        .execute(
            "INSERT INTO customers (code, name, kind) VALUES ('OZT-0001', 'Öztürk İnşaat', 'musteri')",
            [],
        )
        .expect("Türkçe karakterli cari eklenmeli");
    println!(
        "[STAGE12] 1.000.000 cari + 1.000.000 ürün + 100.000 fatura/ödeme tohumu: {} ms",
        seed_start.elapsed().as_millis()
    );

    let source_bytes = fs::metadata(database.path().expect("dosya veritabanı"))
        .expect("veritabanı boyutu")
        .len();
    let backup_path = directory.path().join("stage12.onm-backup");
    let backup_start = Instant::now();
    on_muhasebe::backup::create_backup(&database, &backup_path).expect("yedek alınmalı");
    let backup_ms = backup_start.elapsed().as_millis();
    let backup_bytes = fs::metadata(&backup_path).expect("yedek boyutu").len();
    println!(
        "[STAGE12] yedek: kaynak {:.1} MB, hedef {:.1} MB, {backup_ms} ms",
        source_bytes as f64 / (1024.0 * 1024.0),
        backup_bytes as f64 / (1024.0 * 1024.0)
    );
    assert!(
        source_bytes >= 100 * 1024 * 1024,
        "yedek testi en az 100 MB veritabanı bekler; ölçülen: {source_bytes} bayt"
    );
    assert!(backup_bytes > 0, "yedek dosyası boş olmamalı");

    for query in ["cari", "urun", "ozturk", "istanbul", "SF-0000"] {
        let start = Instant::now();
        let hits = database.global_search(query).expect("arama çalışmalı");
        let elapsed = start.elapsed().as_millis();
        println!(
            "[STAGE12] '{query}' araması: {elapsed} ms ({} sonuç)",
            hits.len()
        );
        assert!(
            elapsed < 500,
            "'{query}' araması 500 ms altında olmalı; ölçülen: {elapsed} ms"
        );
        if query != "istanbul" {
            assert!(!hits.is_empty(), "'{query}' için sonuç bulunmalı");
        }
    }

    let ozturk = database
        .global_search("ozturk")
        .expect("Türkçe arama çalışmalı");
    assert!(
        ozturk.iter().any(|hit| hit.title.contains("Öztürk")),
        "'ozturk' araması 'Öztürk İnşaat' kaydını bulmalı"
    );
}

#[test]
#[ignore = "1.000.000 cari için sanal liste aralığı hesaplanmalı"]
fn bir_milyon_cari_sanal_kaydirma() {
    let start = Instant::now();
    for page in 0..10_000 {
        let offset = page as f32 * 92.0;
        let range = visible_range(1_000_000, 92.0, offset, 800.0, 5);
        assert!(range.start < range.end);
        assert!(range.end <= 1_000_000);
    }
    let elapsed = start.elapsed().as_millis();
    println!("[STAGE12] 1.000.000 cari sanal kaydırma (10.000 sayfa): {elapsed} ms");

    let last = visible_range(1_000_000, 92.0, 92.0 * 999_999.0, 800.0, 5);
    assert_eq!(last.end, 1_000_000);
}
