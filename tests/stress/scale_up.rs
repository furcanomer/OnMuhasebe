// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Ölçek büyütme testleri: G2 (10M cari / 100M stok) ve G3 (1GB+ yedek).
//!
//! Bu testler `#[ignore]` işaretlidir; release modunda tek iş parçacığıyla
//! çalıştırılmalıdır. Disk yetersizse G2b/G3 yanıltıcı biçimde "geçti"
//! yazmaz; açıkça `SKIP` mesajı basar.

use std::fs;
use std::time::Instant;

use on_muhasebe::backup;
use on_muhasebe::db::Database;
use on_muhasebe::ui::virtual_list::visible_range;
use rusqlite::types::Value;

use super::bulk_data::{seed_bulk, seed_customers_only, seed_stock_movements};
use super::{assert_correlations, build_test_db, disk_free_mb, insert_batch, raw_connection};

fn assert_customer_correlations(database: &Database) {
    let connection = raw_connection(database);
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM customers", [], |row| row.get(0))
        .expect("cari sayısı");
    let nulls: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM customers
             WHERE code IS NULL OR name IS NULL OR kind IS NULL OR active IS NULL",
            [],
            |row| row.get(0),
        )
        .expect("cari NULL kontrolü");
    assert_eq!(nulls, 0, "cari zorunlu alanlarında NULL var");
    let duplicates: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM (
                 SELECT code FROM customers GROUP BY code HAVING COUNT(*) > 1
             )",
            [],
            |row| row.get(0),
        )
        .expect("cari kod tekilliği");
    assert_eq!(duplicates, 0, "yinelenen cari kodu var");
    println!("[G2a] cari korelasyonu: {count} kayıt, NULL=0, yinelenen kod=0");
}

/// Mevcut ürünlerin ilk `product_count` tanesi için stok hareketi üretir.
///
/// `seed_bulk` ile 1.000.000 ürün oluşturulduktan sonra `seed_stock_movements`
/// çağrılırsa ürün kodları çakışır. Bu yardımcı yalnızca hareket ekler.
fn seed_movements_for_existing_products(
    database: &Database,
    product_count: usize,
    movements_per_product: usize,
) -> Vec<i64> {
    let connection = raw_connection(database);
    connection
        .execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = OFF;
             PRAGMA temp_store = MEMORY;
             PRAGMA cache_size = -200000;",
        )
        .expect("toplu stok ayarları");
    let product_ids: Vec<i64> = {
        let mut statement = connection
            .prepare("SELECT id FROM products ORDER BY id LIMIT ?1")
            .expect("ürün kimlikleri");
        let rows = statement
            .query_map([product_count as i64], |row| row.get::<_, i64>(0))
            .expect("ürün kimlik satırları");
        rows.map(|row| row.expect("ürün kimliği")).collect()
    };
    assert_eq!(product_ids.len(), product_count);

    let transaction = connection
        .unchecked_transaction()
        .expect("stok işlemi başlatılmalı");
    const BATCH: usize = 1000;
    for (product_index, product_id) in product_ids.iter().enumerate() {
        let mut inserted = 0;
        while inserted < movements_per_product {
            let chunk = (movements_per_product - inserted).min(BATCH);
            let rows: Vec<Vec<Value>> = (0..chunk)
                .map(|offset| {
                    let sequence = inserted + offset + product_index;
                    let positive = sequence.is_multiple_of(2);
                    vec![
                        Value::Integer(*product_id),
                        Value::Text(if positive { "giris" } else { "cikis" }.to_owned()),
                        Value::Integer(if positive { 1000 } else { -1000 }),
                        Value::Text(format!("2026-01-{:02}", (sequence % 28) + 1)),
                        Value::Text("G3 stok hareketi".to_owned()),
                    ]
                })
                .collect();
            insert_batch(
                &transaction,
                "product_stock_movements",
                "product_id, movement_type, quantity_delta, movement_date, description",
                &rows,
            );
            inserted += chunk;
        }
    }
    transaction
        .execute(
            "UPDATE products
                SET current_stock = COALESCE(
                    (SELECT SUM(quantity_delta) FROM product_stock_movements m
                     WHERE m.product_id = products.id), 0)",
            [],
        )
        .expect("stok toplamları kurulmalı");
    transaction.commit().expect("stok işlemi commit edilmeli");
    product_ids
}

/// G2a: 10.000.000 cari; tohum, sayım, sanal görünür aralık ve global arama.
#[test]
#[ignore = "ağır test: 10.000.000 cari"]
fn g2a_on_milyon_cari() {
    let directory = tempfile::tempdir().expect("geçici dizin");
    let free = disk_free_mb(directory.path());
    assert!(
        free > 5_000,
        "G2a için en az 5 GB boş disk gerekir; mevcut: {free} MB"
    );
    let database = build_test_db(&directory.path().join("cari10m.db"));

    let seed_start = Instant::now();
    let ids = seed_customers_only(&database, 10_000_000);
    let seed_ms = seed_start.elapsed().as_millis();
    assert_eq!(ids.len(), 10_000_000);
    drop(ids);
    println!("[G2a] 10.000.000 cari tohumu: {seed_ms} ms");

    let count_start = Instant::now();
    assert_eq!(database.customer_count().expect("cari sayısı"), 10_000_000);
    let count_ms = count_start.elapsed().as_millis();
    assert!(
        count_ms < 2_000,
        "filtresiz cari sayımı 2 sn'yi aştı: {count_ms} ms"
    );

    let range_start = Instant::now();
    let range = visible_range(10_000_000, 92.0, 92.0 * 9_999_999.0, 800.0, 5);
    let range_ms = range_start.elapsed().as_millis();
    assert!(range.start < range.end);
    assert!(range.end <= 10_000_000);
    assert!(range.len() < 40, "sanal görünür aralık küçük olmalı");
    assert!(range_ms < 100, "sanal aralık 100 ms'yi aştı");

    let connection = raw_connection(&database);
    let page_start = Instant::now();
    let mut statement = connection
        .prepare(
            "SELECT id, code, name FROM customers
             ORDER BY id LIMIT 100 OFFSET 5000000",
        )
        .expect("görünür sayfa sorgusu");
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .expect("görünür sayfa satırları");
    let visible = rows
        .collect::<std::result::Result<Vec<_>, _>>()
        .expect("sayfa okunmalı");
    let page_ms = page_start.elapsed().as_millis();
    assert_eq!(visible.len(), 100);
    assert!(
        page_ms < 2_000,
        "filtresiz görünür sayfa 2 sn'yi aştı: {page_ms} ms"
    );

    let search_start = Instant::now();
    let hits = database.global_search("cari").expect("global arama");
    let search_ms = search_start.elapsed().as_millis();
    assert!(!hits.is_empty(), "'cari' araması sonuç döndürmeli");
    assert!(
        search_ms < 500,
        "10M cari araması 500 ms'yi aştı: {search_ms} ms"
    );

    assert_customer_correlations(&database);
    println!(
        "[G2a] sayım={count_ms} ms, görünür aralık={range_ms} ms, sayfa={page_ms} ms, arama={search_ms} ms"
    );
}

/// G2b: 1000 ürün × 100.000 stok hareketi = 100.000.000 satır.
#[test]
#[ignore = "ağır test: 100.000.000 stok hareketi (disk/IO yoğun)"]
fn g2b_yuz_milyon_stok_hareketi() {
    let directory = tempfile::tempdir().expect("geçici dizin");
    let free = disk_free_mb(directory.path());
    if free < 20_000 {
        println!(
            "[G2b] SKIP: 100M stok hareketi için en az 20 GB boş disk gerekir; mevcut: {free} MB"
        );
        return;
    }
    let database = build_test_db(&directory.path().join("stok100m.db"));

    let seed_start = Instant::now();
    let products = seed_stock_movements(&database, 1_000, 100_000);
    let seed_ms = seed_start.elapsed().as_millis();
    assert_eq!(products.len(), 1_000);
    let connection = raw_connection(&database);
    let movements: i64 = connection
        .query_row("SELECT COUNT(*) FROM product_stock_movements", [], |row| {
            row.get(0)
        })
        .expect("hareket sayısı");
    assert_eq!(movements, 100_000_000);
    println!("[G2b] 100.000.000 stok hareketi tohumu: {seed_ms} ms");

    let verify_start = Instant::now();
    assert_correlations(&database);
    let verify_ms = verify_start.elapsed().as_millis();
    println!("[G2b] korelasyon + integrity: {verify_ms} ms");
    println!("[G2b] toplam: {} ms", seed_start.elapsed().as_millis());
}

/// G3: 1 GB+ veritabanında yedek alma ve geri yükleme.
#[test]
#[ignore = "ağır test: 1 GB+ veritabanı yedeği"]
fn g3_bir_gb_ustu_yedek_ve_restore() {
    let directory = tempfile::tempdir().expect("geçici dizin");
    let free = disk_free_mb(directory.path());
    if free < 10_000 {
        println!("[G3] SKIP: 1 GB+ yedek için en az 10 GB boş disk gerekir; mevcut: {free} MB");
        return;
    }
    let database = build_test_db(&directory.path().join("buyuk.db"));
    let seed_start = Instant::now();
    seed_bulk(&database, 1_000_000, 1_000_000, 0, 0, 0);
    seed_movements_for_existing_products(&database, 100, 100_000);
    println!(
        "[G3] büyük veri tohumu: {} ms",
        seed_start.elapsed().as_millis()
    );

    let source_path = database.path().expect("dosya veritabanı").to_path_buf();
    let source_bytes = fs::metadata(&source_path).expect("kaynak boyut").len();
    assert!(
        source_bytes >= 1024 * 1024 * 1024,
        "G3 en az 1 GB veritabanı bekler; ölçülen: {source_bytes} bayt"
    );
    println!(
        "[G3] kaynak DB: {:.1} MB",
        source_bytes as f64 / (1024.0 * 1024.0)
    );

    let backup_path = directory.path().join("buyuk.onm-backup");
    let backup_start = Instant::now();
    backup::create_backup(&database, &backup_path).expect("yedek alınmalı");
    let backup_ms = backup_start.elapsed().as_millis();
    let backup_bytes = fs::metadata(&backup_path).expect("yedek boyut").len();
    assert_eq!(
        backup_bytes, source_bytes,
        "yedek kaynakla aynı boyutta olmalı"
    );
    assert!(
        backup_ms < 30_000,
        "1 GB+ yedek 30 sn'yi aştı: {backup_ms} ms"
    );
    println!("[G3] yedek: {backup_ms} ms, {backup_bytes} bayt");

    let restore_path = directory.path().join("geri-yuklenen.db");
    let restore_start = Instant::now();
    Database::validate_backup(&backup_path).expect("yedek doğrulanmalı");
    fs::copy(&backup_path, &restore_path).expect("geri yükleme kopyası");
    let restored = Database::open(&restore_path).expect("geri yüklenen DB açılmalı");
    let restore_ms = restore_start.elapsed().as_millis();
    let restored_bytes = fs::metadata(&restore_path).expect("hedef boyut").len();
    assert_eq!(
        restored_bytes, source_bytes,
        "geri yüklenen DB boyutu eşit olmalı"
    );
    assert!(
        restore_ms < 30_000,
        "1 GB+ geri yükleme 30 sn'yi aştı: {restore_ms} ms"
    );
    println!("[G3] geri yükleme: {restore_ms} ms");

    let verify_start = Instant::now();
    assert_correlations(&restored);
    println!(
        "[G3] geri yükleme sonrası korelasyon + integrity: {} ms",
        verify_start.elapsed().as_millis()
    );
}
