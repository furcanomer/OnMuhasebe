// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Aşama 13 kurtarma testleri: G4, G6/G10, G7 ve G14.
//!
//! G4 dosya düzeyinde güvenlik yedeğiyle kesinti simülasyonu, G6/G10 1000
//! turluk soak + RSS trendi, G7 v1→v7 tam migration matrisi ve G14 100 bozuk
//! yedek varyantıdır.

use std::fs::{self, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use on_muhasebe::backup;
use on_muhasebe::db::{Database, MIGRATIONS};
use on_muhasebe::income_expense::{EntryDraft, EntryType};
use on_muhasebe::invoice::{InvoiceDraft, InvoiceLineDraft, InvoiceType};
use on_muhasebe::money::Money;
use on_muhasebe::payment::{AllocationDraft, PaymentDraft, PaymentType};
use on_muhasebe::product::Unit;
use on_muhasebe::restore_journal::{self, RestoreJournal, RestoreStep, apply_safety_backup};
use rusqlite::{Connection, params};

use super::{
    assert_correlations, build_test_db, insert_batch, kill_process_hard, measure_memory_mb,
    raw_connection, seed_minimal, wait_for_condition,
};

fn make_invoice(
    database: &Database,
    customer_id: i64,
    product_id: i64,
    number: &str,
    date: &str,
    sales: bool,
) -> (i64, Money) {
    let mut draft = InvoiceDraft::new(if sales {
        InvoiceType::Sales
    } else {
        InvoiceType::Purchase
    });
    draft.number = number.to_owned();
    draft.customer_id = Some(customer_id);
    draft.invoice_date = date.to_owned();
    draft.lines = vec![InvoiceLineDraft {
        product_id: Some(product_id),
        product_code: "URN-0001".to_owned(),
        product_name: "Test Ürün".to_owned(),
        unit: Unit::Adet,
        description: String::new(),
        quantity: "1".to_owned(),
        unit_price: "100,00".to_owned(),
        discount_rate: String::new(),
        vat_rate: "20".to_owned(),
    }];
    let validated = draft.validate().expect("G6 faturası doğrulanmalı");
    let grand = validated.totals.grand_total;
    let id = database
        .invoice_insert(&validated)
        .expect("G6 faturası yazılmalı");
    (id, grand)
}

#[allow(clippy::too_many_arguments)]
fn make_payment(
    database: &Database,
    invoice_id: i64,
    number: &str,
    customer_id: i64,
    cash_account_id: i64,
    date: &str,
    sales: bool,
    grand: Money,
) {
    let half = Money::from_kurus(grand.kurus() / 2);
    let draft = PaymentDraft {
        payment_type: if sales {
            PaymentType::Tahsilat
        } else {
            PaymentType::Odeme
        },
        customer_id: Some(customer_id),
        cash_account_id: Some(cash_account_id),
        payment_date: date.to_owned(),
        amount: half.format_turkish(),
        description: "G6 ödemesi".to_owned(),
        allocations: vec![AllocationDraft {
            invoice_id,
            invoice_number: number.to_owned(),
            invoice_date: date.to_owned(),
            grand_total: grand,
            paid_amount: Money::ZERO,
            amount: half.format_turkish(),
        }],
    };
    let validated = draft.validate().expect("G6 ödemesi doğrulanmalı");
    database
        .payment_insert(&validated)
        .expect("G6 ödemesi yazılmalı");
}

fn make_entry(
    database: &Database,
    entry_type: EntryType,
    category_id: i64,
    cash_account_id: i64,
    date: &str,
) {
    let draft = EntryDraft {
        entry_type,
        category_id: Some(category_id),
        cash_account_id: Some(cash_account_id),
        entry_date: date.to_owned(),
        amount: if entry_type == EntryType::Gelir {
            "10,00".to_owned()
        } else {
            "5,00".to_owned()
        },
        description: "G6 gelir/gider".to_owned(),
        document_ref: String::new(),
    };
    let validated = draft.validate().expect("G6 gelir/gider doğrulanmalı");
    database
        .income_expense_insert(&validated)
        .expect("G6 gelir/gider yazılmalı");
}

fn spawn_interrupted_restore(source: &Path, temp: &Path, destination: &Path) -> Child {
    let quote = |path: &Path| path.to_string_lossy().replace('\'', "''");
    let script = format!(
        "Copy-Item -LiteralPath '{}' -Destination '{}' -Force; \
         Start-Sleep -Milliseconds 5000; \
         Copy-Item -LiteralPath '{}' -Destination '{}' -Force",
        quote(source),
        quote(temp),
        quote(temp),
        quote(destination)
    );
    Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("kesinti simülasyon süreci başlatılmalı")
}

/// G4: Geri yükleme, orijinal DB'nin üzerine yazılmadan önce kesilir.
///
/// Gerçek `restore.journal` akışı kullanılır: güvenlik yedeği alınır, günlük
/// `copying` olarak yazılır, geçici kopya oluşturulurken `reopening` adımına
/// geçilir ve süreç öldürülür. Bir sonraki açılışta yarıda kalan işlem tespit
/// edilmeli; güvenlik yedeğine dönüldüğünde veri korunmalıdır.
#[test]
#[ignore = "ağır test: geri yükleme sırasında süreç kill (3 tekrar)"]
fn g4_geri_yukleme_sirasinda_kesinti() {
    for run in 0..3 {
        let directory = tempfile::tempdir().expect("geçici dizin");
        let database_path = directory.path().join("on_muhasebe.db");
        let database = build_test_db(&database_path);
        let _seed = seed_minimal(&database);
        database.checkpoint().expect("checkpoint");

        let backup_path = directory.path().join("g4.onm-backup");
        backup::create_backup(&database, &backup_path).expect("G4 yedeği");
        let safety = directory
            .path()
            .join(format!("on_muhasebe.db.pre-restore-g4-{run}"));
        fs::copy(&database_path, &safety).expect("güvenlik yedeği");
        let temp = directory.path().join("on_muhasebe.db.restore-tmp-g4");
        drop(database);

        let journal_path = RestoreJournal::path(directory.path());
        let mut journal =
            RestoreJournal::new(backup_path.clone(), safety.clone(), database_path.clone());
        journal
            .write(&journal_path)
            .expect("günlük copying yazılmalı");

        let child = spawn_interrupted_restore(&backup_path, &temp, &database_path);
        assert!(
            wait_for_condition(|| temp.exists(), 10_000),
            "geri yükleme geçici dosyası oluşmadı"
        );
        journal
            .update_step(&journal_path, RestoreStep::Reopening)
            .expect("günlük reopening yazılmalı");
        thread::sleep(Duration::from_millis(100));
        kill_process_hard(child);
        assert!(database_path.exists(), "orijinal DB silinmemeli");

        let detected = restore_journal::detect(directory.path())
            .expect("günlük okunmalı")
            .expect("yarıda kalan restore tespit edilmeli");
        assert_eq!(detected.step, RestoreStep::Reopening);
        assert!(detected.is_interrupted());

        let reopened = build_test_db(&database_path);
        assert_eq!(reopened.customer_count().expect("cari sayısı"), 1);
        assert_correlations(&reopened);
        assert!(safety.exists(), "güvenlik yedeği korunmalı");
        Database::validate_backup(&safety).expect("güvenlik yedeği geçerli olmalı");
        drop(reopened);

        apply_safety_backup(&detected).expect("güvenlik yedeğine dönülmeli");
        RestoreJournal::remove(&journal_path).expect("günlük silinmeli");
        let recovered = build_test_db(&database_path);
        assert_eq!(recovered.customer_count().expect("cari sayısı"), 1);
        assert_correlations(&recovered);
        println!(
            "[G4] tur {}: kesinti tespit edildi, güvenlik yedeğiyle veri korundu",
            run + 1
        );
    }
    println!("[G4] 3/3 geçti");
}

/// G6 + G10: 1000 tur soak; her tur 10 fatura + 10 ödeme + 5 gelir/gider.
#[test]
#[ignore = "ağır test: 1000 tur soak + RSS trendi"]
fn g6_g10_bin_tur_soak_ve_bellek() {
    let directory = tempfile::tempdir().expect("geçici dizin");
    let path = directory.path().join("g6.db");
    let backup_path = directory.path().join("g6-yedek.onm-backup");
    let database = build_test_db(&path);
    let seed = seed_minimal(&database);

    let initial = measure_memory_mb();
    let mut trend = vec![initial];
    let total_start = Instant::now();
    for round in 0..1_000usize {
        let date = format!("2026-{:02}-{:02}", (round % 12) + 1, (round % 28) + 1);
        let mut invoices = Vec::with_capacity(10);
        for index in 0..10usize {
            let sales = index.is_multiple_of(2);
            let number = format!("G6-{round:04}-{index:02}");
            let (invoice_id, grand) = make_invoice(
                &database,
                seed.customer_id,
                seed.product_id,
                &number,
                &date,
                sales,
            );
            invoices.push((invoice_id, number, sales, grand));
        }
        for (invoice_id, number, sales, grand) in &invoices {
            make_payment(
                &database,
                *invoice_id,
                number,
                seed.customer_id,
                seed.cash_account_id,
                &date,
                *sales,
                *grand,
            );
        }
        for index in 0..5usize {
            let income = index.is_multiple_of(2);
            make_entry(
                &database,
                if income {
                    EntryType::Gelir
                } else {
                    EntryType::Gider
                },
                if income {
                    seed.income_category_id
                } else {
                    seed.expense_category_id
                },
                seed.cash_account_id,
                &date,
            );
        }

        database.checkpoint().expect("G6 checkpoint");
        backup::create_backup(&database, &backup_path).expect("G6 yedeği");
        Database::validate_backup(&backup_path).expect("G6 yedeği geçerli");

        let completed = round + 1;
        if completed.is_multiple_of(100) {
            let rss = measure_memory_mb();
            trend.push(rss);
            assert_correlations(&database);
            println!(
                "[G6] {completed} tur: RSS={rss} MB, toplam={} ms",
                total_start.elapsed().as_millis()
            );
        }
    }

    // İlk 100 tur SQLite sayfa önbelleği ve ayırıcı ısınmasını tamamlar;
    // sızıntı ölçümü bu ısınma sonrası kararlı taban çizgisine göre yapılır.
    let baseline = trend.get(1).copied().unwrap_or(initial);
    let end = measure_memory_mb();
    let limit = baseline + baseline / 5;
    assert!(
        end <= limit,
        "G10 bellek büyümesi %20'yi aştı: ısınma sonrası {baseline} MB, son {end} MB, sınır {limit} MB"
    );

    assert_eq!(database.invoice_count().expect("fatura"), 10_000);
    assert_eq!(database.payment_count().expect("ödeme"), 10_000);
    assert_eq!(database.income_expense_count().expect("gelir/gider"), 5_000);
    assert_correlations(&database);
    println!(
        "[G6] toplam süre: {} ms, tur ortalaması: {} ms",
        total_start.elapsed().as_millis(),
        total_start.elapsed().as_millis() / 1_000
    );
    let peak = trend.iter().copied().max().unwrap_or(end);
    println!(
        "[G10] RSS: ilk={initial} MB, ısınma sonrası={baseline} MB, son={end} MB, tepe={peak} MB, trend={trend:?}"
    );
}

fn prepare_legacy_db(path: &Path, version: i32) {
    let connection = Connection::open(path).expect("eski DB açılmalı");
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
    for migration in MIGRATIONS
        .iter()
        .filter(|migration| migration.version <= version)
    {
        connection
            .execute_batch(migration.sql)
            .expect("eski migration SQL'i");
        connection
            .execute(
                "INSERT INTO schema_migrations (version, name) VALUES (?1, ?2)",
                params![migration.version, migration.name],
            )
            .expect("eski migration kaydı");
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
            .expect("tablo kontrolü");
        assert!(exists, "{table} tablosu yok");
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
            .expect("indeks kontrolü");
        assert!(exists, "{index} indeksi yok");
    }
}

/// G7: v1→v7, v2→v7, ..., v6→v7 tam migration matrisi.
#[test]
fn g7_migration_tam_matrisi() {
    for version in 1..=6 {
        let directory = tempfile::tempdir().expect("geçici dizin");
        let path = directory.path().join(format!("legacy-v{version}.db"));
        prepare_legacy_db(&path, version);
        let database = Database::open(&path).expect("güncel şemaya yükseltilmeli");
        assert_eq!(database.schema_version().expect("şema sürümü"), 7);
        let applied: Vec<i32> = database
            .applied_migrations()
            .expect("migration kayıtları")
            .into_iter()
            .map(|migration| migration.version)
            .collect();
        assert_eq!(
            applied,
            (1..=7).collect::<Vec<_>>(),
            "v{version}→v7 sırası bozulmamış olmalı"
        );
        assert_schema_objects(&database);
        assert_correlations(&database);
        println!("[G7] v{version}→v7: sıra, şema, korelasyon ve integrity tamam");
    }
}

fn corrupt_byte(path: &Path, offset: u64, value: u8) {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .expect("bozuk dosya açılmalı");
    file.seek(SeekFrom::Start(offset)).expect("offset");
    file.write_all(&[value]).expect("bayt yazılmalı");
}

fn corrupt_sql(path: &Path, sql: &str) {
    let connection = Connection::open(path).expect("SQL bozma bağlantısı");
    connection.execute_batch(sql).expect("SQL bozma");
    let _ = connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
}

fn page_info(path: &Path) -> (u64, u64) {
    let connection = Connection::open(path).expect("sayfa bilgisi bağlantısı");
    let page_size = connection
        .query_row("PRAGMA page_size", [], |row| row.get::<_, i64>(0))
        .expect("page_size") as u64;
    let page_count = connection
        .query_row("PRAGMA page_count", [], |row| row.get::<_, i64>(0))
        .expect("page_count") as u64;
    (page_size, page_count)
}

/// G14: 100 bozuk yedek varyantı; 100/100 reddedilmeli.
#[test]
fn g14_yuz_bozuk_yedek_varyanti() {
    let directory = tempfile::tempdir().expect("geçici dizin");
    let database = build_test_db(&directory.path().join("mevcut.db"));
    let _seed = seed_minimal(&database);
    let connection = raw_connection(&database);
    let rows: Vec<Vec<rusqlite::types::Value>> = (0..5_000)
        .map(|index| {
            vec![
                rusqlite::types::Value::Text(format!("H-{index:05}")),
                rusqlite::types::Value::Text(format!("Dolgu Cari {index:05}")),
                rusqlite::types::Value::Text("musteri".to_owned()),
            ]
        })
        .collect();
    let transaction = connection.unchecked_transaction().expect("dolgu işlemi");
    insert_batch(&transaction, "customers", "code, name, kind", &rows);
    transaction.commit().expect("dolgu commit");
    database.checkpoint().expect("checkpoint");

    let valid = directory.path().join("gecerli.onm-backup");
    backup::create_backup(&database, &valid).expect("geçerli yedek");
    let length = fs::metadata(&valid).expect("yedek boyutu").len();
    let (page_size, pages) = page_info(&valid);
    assert!(pages > 20, "G14 için yeterli sayfa yok: {pages}");

    let mut variants = 0usize;
    let mut check = |path: &Path, label: &str| {
        let result = Database::validate_backup(path);
        assert!(
            result.is_err(),
            "G14 {label} varyantı kabul edildi; yedek reddedilmeli"
        );
        assert_correlations(&database);
        variants += 1;
    };

    // 1) SQLite başlığının ilk 16 baytı.
    for offset in 0..16u64 {
        let path = directory
            .path()
            .join(format!("bozuk-header-{offset}.onm-backup"));
        fs::copy(&valid, &path).expect("varyant kopyası");
        corrupt_byte(&path, offset, 0xFF);
        check(&path, &format!("header-{offset}"));
    }

    // 2) Başlığın kritik alanları (17-100 aralığından 20 bayt).
    //    `quick_check` tek başına bazı alanları yakalamaz; `validate_backup`
    //    ayrıca başlık alanlarını doğrular.
    for offset in [
        17u64, 18, 19, 20, 21, 22, 23, 44, 45, 46, 47, 56, 57, 58, 59, 72, 73, 74, 75, 76,
    ] {
        let path = directory
            .path()
            .join(format!("bozuk-kritik-{offset}.onm-backup"));
        fs::copy(&valid, &path).expect("varyant kopyası");
        corrupt_byte(&path, offset, 0xFF);
        check(&path, &format!("kritik-{offset}"));
    }

    // 3) Sayfa başlıkları (ortadan 20 sayfa).
    for page in 1..=20u64 {
        let path = directory
            .path()
            .join(format!("bozuk-sayfa-{page}.onm-backup"));
        fs::copy(&valid, &path).expect("varyant kopyası");
        corrupt_byte(&path, page * page_size, 0x00);
        check(&path, &format!("sayfa-{page}"));
    }

    // 4) Son sayfalardaki başlıklar (son 100 bayt bölgesi).
    for back in 0..20u64 {
        let page = pages.saturating_sub(back + 1).max(1);
        let path = directory
            .path()
            .join(format!("bozuk-son-{back}.onm-backup"));
        fs::copy(&valid, &path).expect("varyant kopyası");
        corrupt_byte(&path, page * page_size, 0x00);
        check(&path, &format!("son-{back}"));
    }

    // 5) Yapısal/uygulama düzeyi 24 varyant.
    for index in 0..24usize {
        let path = directory
            .path()
            .join(format!("bozuk-yapisal-{index}.onm-backup"));
        fs::copy(&valid, &path).expect("varyant kopyası");
        let sql = match index {
            0..=9 => {
                format!("UPDATE app_metadata SET value = 'Baska{index}' WHERE key = 'app_name';")
            }
            10 => "DELETE FROM app_metadata WHERE key = 'app_name';".to_owned(),
            11 => "DELETE FROM app_metadata;".to_owned(),
            12 => "DROP TABLE app_metadata;".to_owned(),
            13 => "DROP TABLE schema_migrations;".to_owned(),
            14 => "DELETE FROM schema_migrations;".to_owned(),
            15 => "INSERT INTO schema_migrations (version, name) VALUES (8, 'gelecek');".to_owned(),
            16 => "UPDATE schema_migrations SET version = 8 WHERE version = 7;".to_owned(),
            17 => "UPDATE schema_migrations SET version = 999 WHERE version = 7;".to_owned(),
            18 => "UPDATE app_metadata SET key = 'app_name_x' WHERE key = 'app_name';".to_owned(),
            19 => "UPDATE app_metadata SET value = '' WHERE key = 'app_name';".to_owned(),
            20 => {
                "UPDATE app_metadata SET value = 'OnMuhasebe ' WHERE key = 'app_name';".to_owned()
            }
            21 => "UPDATE app_metadata SET value = 'onmuhasebe' WHERE key = 'app_name';".to_owned(),
            22 => "INSERT INTO schema_migrations (version, name) VALUES (8, 'x');".to_owned(),
            _ => "DELETE FROM schema_migrations;".to_owned(),
        };
        corrupt_sql(&path, &sql);
        check(&path, &format!("yapisal-{index}"));
    }

    assert_eq!(variants, 100, "tam 100 varyant denenmeli");
    assert_eq!(length, fs::metadata(&valid).expect("geçerli yedek").len());
    assert_correlations(&database);
    println!("[G14] 100/100 bozuk yedek reddedildi; mevcut DB bozulmadı");
}
