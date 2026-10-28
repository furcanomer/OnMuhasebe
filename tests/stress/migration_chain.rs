// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! v1-v5 şemalarından v6'ya yükseltme ve v5 yedeğini geri yükleme testleri.

use std::path::Path;

use on_muhasebe::db::{APP_DB_NAME, Database, MIGRATIONS};
use rusqlite::Connection;

use super::{assert_correlations, build_test_db, raw_connection};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LegacyStats {
    customers: i64,
    products: i64,
    movements: i64,
    invoices: i64,
    stock_sum: i64,
    paid_sum: i64,
}

fn prepare_legacy_db(path: &Path, version: i32) -> LegacyStats {
    let connection = Connection::open(path).expect("eski veritabanı açılmalı");
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE schema_migrations (
                 version    INTEGER PRIMARY KEY NOT NULL,
                 name       TEXT NOT NULL,
                 applied_at TEXT NOT NULL DEFAULT (datetime('now'))
             );",
        )
        .expect("migration tablosu");

    for migration in MIGRATIONS.iter().filter(|m| m.version <= version) {
        connection
            .execute_batch(migration.sql)
            .expect("eski migration uygulanmalı");
        connection
            .execute(
                "INSERT INTO schema_migrations (version, name) VALUES (?1, ?2)",
                rusqlite::params![migration.version, migration.name],
            )
            .expect("migration kaydı");
    }

    connection
        .execute(
            "INSERT INTO app_metadata (key, value) VALUES ('app_name', ?1)",
            [APP_DB_NAME],
        )
        .expect("uygulama adı");
    connection
        .execute(
            "INSERT INTO app_metadata (key, value) VALUES ('created_at', datetime('now'))",
            [],
        )
        .expect("oluşturma zamanı");

    let mut customer_id = 0;
    if version >= 2 {
        connection
            .execute(
                "INSERT INTO customers (code, name, kind) VALUES ('ESKI-1', 'Eski Cari', 'musteri')",
                [],
            )
            .expect("eski cari");
        customer_id = connection.last_insert_rowid();
    }

    if version == 3 {
        connection
            .execute(
                "INSERT INTO products
                     (code, name, unit, purchase_price, sale_price, vat_rate, current_stock)
                 VALUES ('ESKI-P1', 'Eski Ürün', 'adet', 10000, 12000, 20, 5000)",
                [],
            )
            .expect("eski ürün");
        let product_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO product_stock_movements
                     (product_id, movement_type, quantity_delta, movement_date, description)
                 VALUES (?1, 'ilk_stok', 5000, '2026-01-01', 'eski stok')",
                [product_id],
            )
            .expect("eski stok hareketi");
    }

    if version >= 4 {
        connection
            .execute(
                "INSERT INTO products
                     (code, name, unit, purchase_price, sale_price, vat_rate, current_stock)
                 VALUES ('ESKI-P1', 'Eski Ürün', 'adet', 10000, 12000, 20, -1000)",
                [],
            )
            .expect("eski ürün");
        let product_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO invoices
                     (invoice_number, invoice_type, customer_id, invoice_date, notes,
                      subtotal, discount_total, net_total, vat_total, grand_total, status)
                 VALUES ('SF-ESKI-1', 'satis', ?1, '2026-01-02', 'eski fatura',
                         10000, 0, 10000, 2000, 12000, 'kesin')",
                [customer_id],
            )
            .expect("eski fatura");
        let invoice_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO invoice_lines
                     (invoice_id, product_id, line_no, product_code, product_name, unit,
                      quantity, unit_price, discount_rate, vat_rate, line_gross,
                      line_discount, line_net, line_vat, line_total)
                 VALUES (?1, ?2, 1, 'ESKI-P1', 'Eski Ürün', 'adet',
                         1000, 10000, 0, 20, 10000, 0, 10000, 2000, 12000)",
                rusqlite::params![invoice_id, product_id],
            )
            .expect("eski fatura satırı");
        connection
            .execute(
                "INSERT INTO product_stock_movements
                     (product_id, movement_type, quantity_delta, movement_date, description,
                      invoice_id)
                 VALUES (?1, 'satis_faturasi', -1000, '2026-01-02', 'eski fatura stoğu', ?2)",
                rusqlite::params![product_id, invoice_id],
            )
            .expect("eski fatura stoğu");

        if version >= 5 {
            connection
                .execute(
                    "INSERT INTO cash_accounts
                         (name, account_type, opening_balance, current_balance)
                     VALUES ('ESKI KASA', 'kasa', 1000, 2000)",
                    [],
                )
                .expect("eski kasa");
            let cash_id = connection.last_insert_rowid();
            connection
                .execute(
                    "INSERT INTO payments
                         (payment_type, customer_id, cash_account_id, payment_date, amount,
                          description, status)
                     VALUES ('tahsilat', ?1, ?2, '2026-01-03', 1000, 'eski tahsilat', 'kesin')",
                    rusqlite::params![customer_id, cash_id],
                )
                .expect("eski tahsilat");
            let payment_id = connection.last_insert_rowid();
            connection
                .execute(
                    "INSERT INTO payment_allocations (payment_id, invoice_id, amount)
                     VALUES (?1, ?2, 1000)",
                    rusqlite::params![payment_id, invoice_id],
                )
                .expect("eski dağıtım");
            connection
                .execute(
                    "UPDATE invoices SET paid_amount = 1000 WHERE id = ?1",
                    [invoice_id],
                )
                .expect("eski ödenen tutar");
        }
    }

    let customers = if version >= 2 { 1 } else { 0 };
    let products = if version >= 3 { 1 } else { 0 };
    let movements = if version >= 3 { 1 } else { 0 };
    let invoices = if version >= 4 { 1 } else { 0 };
    let stock_sum = if version == 3 {
        5000
    } else if version >= 4 {
        -1000
    } else {
        0
    };
    let paid_sum = if version >= 5 { 1000 } else { 0 };

    LegacyStats {
        customers,
        products,
        movements,
        invoices,
        stock_sum,
        paid_sum,
    }
}

fn stats(database: &Database) -> LegacyStats {
    let connection = raw_connection(database);
    let customers = connection
        .query_row("SELECT COUNT(*) FROM customers", [], |row| row.get(0))
        .unwrap();
    let products = connection
        .query_row("SELECT COUNT(*) FROM products", [], |row| row.get(0))
        .unwrap();
    let movements = connection
        .query_row("SELECT COUNT(*) FROM product_stock_movements", [], |row| {
            row.get(0)
        })
        .unwrap();
    let invoices = connection
        .query_row("SELECT COUNT(*) FROM invoices", [], |row| row.get(0))
        .unwrap();
    let stock_sum = connection
        .query_row(
            "SELECT COALESCE(SUM(current_stock), 0) FROM products",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let paid_sum = connection
        .query_row(
            "SELECT COALESCE(SUM(paid_amount), 0) FROM invoices",
            [],
            |row| row.get(0),
        )
        .unwrap();
    LegacyStats {
        customers,
        products,
        movements,
        invoices,
        stock_sum,
        paid_sum,
    }
}

fn assert_schema_objects(database: &Database) {
    let connection = raw_connection(database);
    for table in [
        "app_metadata",
        "schema_migrations",
        "customers",
        "products",
        "product_stock_movements",
        "invoices",
        "invoice_lines",
        "cash_accounts",
        "payments",
        "payment_allocations",
        "expense_categories",
        "income_expense_entries",
        "users",
        "app_settings",
    ] {
        let exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
                [table],
                |row| row.get(0),
            )
            .unwrap();
        assert!(exists, "{table} tablosu güncel şemada yok");
    }
    for index in [
        "idx_customers_name",
        "idx_products_barcode",
        "idx_invoices_customer",
        "idx_payment_allocations_invoice",
        "idx_entries_type_date",
        "idx_users_username",
    ] {
        let exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'index' AND name = ?1)",
                [index],
                |row| row.get(0),
            )
            .unwrap();
        assert!(exists, "{index} indeksi güncel şemada yok");
    }
    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
    let fk: i64 = connection
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(fk, 0);
}

/// v1-v6 şemalarından güncel sürüme yükseltme.
#[test]
fn migration_zinciri_v1_v6_guncel() {
    for version in 1..=6 {
        let directory = tempfile::tempdir().expect("geçici dizin");
        let path = directory.path().join(format!("legacy-v{version}.db"));
        let before = prepare_legacy_db(&path, version);
        let database = build_test_db(&path);
        assert_eq!(
            database.schema_version().unwrap(),
            on_muhasebe::db::latest_version()
        );
        assert_schema_objects(&database);
        let after = stats(&database);
        assert_eq!(
            after, before,
            "v{version} -> güncel sürüm veri toplamları değişmemeli"
        );
        assert_correlations(&database);
        println!("[MIGRATION] v{version} -> güncel sürüm ok");
    }
}

/// v5 yedeği güncel sürüme yükseltildikten sonra yedek doğrulama ve geri yükleme.
#[test]
fn v5_yedegi_guncel_olarak_geri_yuklenir() {
    let directory = tempfile::tempdir().expect("geçici dizin");
    let path = directory.path().join("v5.db");
    let before = prepare_legacy_db(&path, 5);
    let database = build_test_db(&path);
    assert_eq!(stats(&database), before);

    database.checkpoint().expect("checkpoint");
    let backup = directory.path().join("v5-yedek.onm-backup");
    std::fs::copy(database.path().unwrap(), &backup).expect("yedek kopyası");
    let info =
        Database::validate_backup(&backup).expect("güncel sürüme yükseltilmiş yedek geçerli");
    assert_eq!(info.schema_version, on_muhasebe::db::latest_version());

    let restored_path = directory.path().join("restored.db");
    std::fs::copy(&backup, &restored_path).expect("geri yükleme kopyası");
    let restored = Database::open(&restored_path).expect("geri yüklenen veritabanı açılmalı");
    assert_eq!(stats(&restored), before);
    assert_schema_objects(&restored);
    assert_correlations(&restored);
    println!("[MIGRATION] v5 yedeği v6 olarak geri yüklendi");
}
