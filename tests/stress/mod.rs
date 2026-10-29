// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Ağır test altyapısı.
//!
//! Bu modül üretim koduna test kancası eklemeden yalnızca genel API'yi ve
//! SQLite'ın kendi bütünlük araçlarını kullanır. Ağır testler `#[ignore]`
//! işaretlidir; normal `cargo test --all-targets` bunları çalıştırmaz.

#![allow(dead_code)]

pub mod bulk_data;
pub mod failure_injection;
pub mod fuzz;
pub mod integrity;
pub mod migration_chain;
pub mod real_world;
pub mod recovery;
pub mod scale_up;
pub mod soak;
pub mod stage11_stress;
pub mod stage12_stress;
pub mod ui_health;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use on_muhasebe::db::Database;
use rusqlite::types::Value;
use rusqlite::{Connection, params_from_iter};

/// Test veritabanındaki temel kayıtların kimlikleri.
#[derive(Debug, Clone, Copy)]
pub struct SeedIds {
    pub customer_id: i64,
    pub product_id: i64,
    pub cash_account_id: i64,
    pub income_category_id: i64,
    pub expense_category_id: i64,
}

/// Geçici bir dosya veritabanını açar ve şemayı doğrular.
pub fn build_test_db(path: &Path) -> Database {
    let database = Database::open(path).expect("test veritabanı açılmalı");
    assert_eq!(
        database.schema_version().expect("şema sürümü"),
        7,
        "stress testleri şema v7 bekler"
    );
    database
}

/// Uygulamanın bağlantısına dokunmadan aynı dosyaya ikinci bir SQLite bağlantısı açar.
pub fn raw_connection(database: &Database) -> Connection {
    let path = database.path().expect("dosya veritabanı beklenir");
    let connection = Connection::open(path).expect("ikinci SQLite bağlantısı açılmalı");
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 30000;",
        )
        .expect("SQLite ayarları uygulanmalı");
    connection
}

/// Birden fazla satırı tek bir parametreli INSERT ile yazar.
pub(crate) fn insert_batch(
    connection: &Connection,
    table: &str,
    columns: &str,
    rows: &[Vec<Value>],
) {
    if rows.is_empty() {
        return;
    }
    let width = rows[0].len();
    assert!(
        rows.iter().all(|row| row.len() == width),
        "toplu INSERT satır genişliği tutarsız"
    );

    let mut sql = format!("INSERT INTO {table} ({columns}) VALUES ");
    sql.reserve(rows.len() * width * 5);
    for (row_index, _) in rows.iter().enumerate() {
        if row_index > 0 {
            sql.push(',');
        }
        sql.push('(');
        for column_index in 0..width {
            if column_index > 0 {
                sql.push(',');
            }
            let parameter = row_index * width + column_index + 1;
            sql.push('?');
            sql.push_str(&parameter.to_string());
        }
        sql.push(')');
    }

    let values: Vec<Value> = rows.iter().flat_map(|row| row.iter().cloned()).collect();
    connection
        .execute(&sql, params_from_iter(values))
        .expect("toplu INSERT başarısız");
}

/// 1 cari, 1 ürün, 1 kasa ve iki kategori oluşturur.
pub fn seed_minimal(database: &Database) -> SeedIds {
    let connection = raw_connection(database);
    connection
        .execute_batch(
            "BEGIN IMMEDIATE;
             INSERT INTO customers (code, name, kind) VALUES ('CAR-0001', 'Test Cari', 'musteri');
             INSERT INTO products (code, name, unit, purchase_price, sale_price, vat_rate)
                 VALUES ('URN-0001', 'Test Ürün', 'adet', 10000, 12000, 20);
             INSERT INTO cash_accounts (name, account_type, opening_balance, current_balance)
                 VALUES ('KASA-1', 'kasa', 100000, 100000);
             INSERT INTO expense_categories (name, category_type) VALUES ('Satış Geliri', 'gelir');
             INSERT INTO expense_categories (name, category_type) VALUES ('Kira Gideri', 'gider');
             COMMIT;",
        )
        .expect("minimal veri yazılmalı");

    let customer_id = connection
        .query_row(
            "SELECT id FROM customers WHERE code = 'CAR-0001'",
            [],
            |row| row.get(0),
        )
        .expect("cari kimliği");
    let product_id = connection
        .query_row(
            "SELECT id FROM products WHERE code = 'URN-0001'",
            [],
            |row| row.get(0),
        )
        .expect("ürün kimliği");
    let cash_account_id = connection
        .query_row(
            "SELECT id FROM cash_accounts WHERE name = 'KASA-1'",
            [],
            |row| row.get(0),
        )
        .expect("kasa kimliği");
    let income_category_id = connection
        .query_row(
            "SELECT id FROM expense_categories WHERE name = 'Satış Geliri'",
            [],
            |row| row.get(0),
        )
        .expect("gelir kategorisi");
    let expense_category_id = connection
        .query_row(
            "SELECT id FROM expense_categories WHERE name = 'Kira Gideri'",
            [],
            |row| row.get(0),
        )
        .expect("gider kategorisi");

    SeedIds {
        customer_id,
        product_id,
        cash_account_id,
        income_category_id,
        expense_category_id,
    }
}

/// Testi belirtilen süre içinde tamamlanmaya zorlar; süre aşılırsa net mesajla panikler.
///
/// Test paniklerini yutmaz: iş parçacığı paniklediyse aynı panik ana test
/// iş parçacığına taşınır.
pub fn run_with_timeout<T, F>(f: F, seconds: u64) -> T
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (sender, receiver) = mpsc::channel();
    let handle = thread::spawn(move || {
        let value = f();
        let _ = sender.send(());
        value
    });

    match receiver.recv_timeout(Duration::from_secs(seconds)) {
        Ok(()) => handle.join().expect("test iş parçacığı başarısız"),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            panic!("test {seconds} saniye içinde tamamlanmadı");
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => match handle.join() {
            Ok(_) => panic!("test iş parçacığı sonuç üretmeden bitti"),
            Err(payload) => std::panic::resume_unwind(payload),
        },
    }
}

/// Varsayılan cari filtresi.
pub fn all_customers() -> on_muhasebe::customer::CustomerFilter {
    on_muhasebe::customer::CustomerFilter::default()
}

/// Varsayılan ürün filtresi.
pub fn all_products() -> on_muhasebe::product::ProductFilter {
    on_muhasebe::product::ProductFilter::default()
}

/// Veritabanının WAL günlüğünde olduğunu doğrular.
pub fn assert_wal(connection: &Connection) {
    let mode: String = connection
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .expect("journal_mode okunmalı");
    assert_eq!(
        mode.to_ascii_lowercase(),
        "wal",
        "journal_mode WAL olmalı, gelen: {mode}"
    );
}

/// Tüm muhasebe korelasyonlarını ve SQLite bütünlüğünü doğrular.
///
/// Bu fonksiyon üretim kodundaki okuma yollarını ve bağımsız SQL toplamlarını
/// birlikte kullanır; bir tutarsızlıkta açıklayıcı mesajla panikler.
pub fn assert_correlations(database: &Database) {
    let connection = raw_connection(database);
    assert_wal(&connection);

    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .expect("integrity_check çalışmalı");
    assert_eq!(
        integrity, "ok",
        "SQLite integrity_check başarısız: {integrity}"
    );

    let foreign_key_violations: i64 = connection
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .expect("foreign_key_check çalışmalı");
    assert_eq!(
        foreign_key_violations, 0,
        "foreign_key_check ihlali: {foreign_key_violations}"
    );

    // current_stock == SUM(quantity_delta)
    let stock_bad: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM (
                 SELECT p.id
                 FROM products p
                 LEFT JOIN product_stock_movements m ON m.product_id = p.id
                 GROUP BY p.id, p.current_stock
                 HAVING p.current_stock <> COALESCE(SUM(m.quantity_delta), 0)
             )",
            [],
            |row| row.get(0),
        )
        .expect("stok korelasyonu");
    assert_eq!(stock_bad, 0, "{stock_bad} üründe stok toplamı tutarsız");

    // invoices.paid_amount == yalnızca kesin ödemelerin dağıtım toplamı
    let paid_bad: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM (
                 SELECT i.id
                 FROM invoices i
                 LEFT JOIN payment_allocations pa ON pa.invoice_id = i.id
                 LEFT JOIN payments p ON p.id = pa.payment_id
                 WHERE i.status = 'kesin'
                 GROUP BY i.id, i.paid_amount
                 HAVING i.paid_amount <> COALESCE(SUM(
                     CASE WHEN p.status = 'kesin' THEN pa.amount ELSE 0 END), 0)
             )",
            [],
            |row| row.get(0),
        )
        .expect("fatura ödenen tutar korelasyonu");
    assert_eq!(
        paid_bad, 0,
        "{paid_bad} faturada paid_amount dağıtım toplamıyla uyuşmuyor"
    );

    // İptal edilmiş faturaların ödenen tutarı sıfır olmalı.
    let cancelled_paid: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM invoices WHERE status = 'iptal' AND paid_amount <> 0",
            [],
            |row| row.get(0),
        )
        .expect("iptal fatura ödenen tutarı");
    assert_eq!(
        cancelled_paid, 0,
        "{cancelled_paid} iptal faturada paid_amount sıfır değil"
    );

    // Kasa bakiyesi: açılış + kesin tahsilat - kesin ödeme + kesin gelir - kesin gider
    let cash_bad: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM (
                 SELECT ca.id
                 FROM cash_accounts ca
                 WHERE ca.current_balance <> ca.opening_balance
                     + COALESCE((SELECT SUM(p.amount) FROM payments p
                         WHERE p.cash_account_id = ca.id AND p.status = 'kesin'
                           AND p.payment_type = 'tahsilat'), 0)
                     - COALESCE((SELECT SUM(p.amount) FROM payments p
                         WHERE p.cash_account_id = ca.id AND p.status = 'kesin'
                           AND p.payment_type = 'odeme'), 0)
                     + COALESCE((SELECT SUM(e.amount) FROM income_expense_entries e
                         WHERE e.cash_account_id = ca.id AND e.status = 'kesin'
                           AND e.entry_type = 'gelir'), 0)
                     - COALESCE((SELECT SUM(e.amount) FROM income_expense_entries e
                         WHERE e.cash_account_id = ca.id AND e.status = 'kesin'
                           AND e.entry_type = 'gider'), 0)
             )",
            [],
            |row| row.get(0),
        )
        .expect("kasa bakiyesi korelasyonu");
    assert_eq!(
        cash_bad, 0,
        "{cash_bad} kasa hesabında bakiye formülü tutarsız"
    );

    // Cari bakiyesi için bağımsız toplamlar (cari başına alt sorgu yerine tek tarama).
    let mut sales: HashMap<i64, i64> = HashMap::new();
    let mut purchases: HashMap<i64, i64> = HashMap::new();
    let mut collections: HashMap<i64, i64> = HashMap::new();
    let mut payments: HashMap<i64, i64> = HashMap::new();

    {
        let mut statement = connection
            .prepare(
                "SELECT customer_id, SUM(grand_total) FROM invoices
                 WHERE status = 'kesin' AND invoice_type = 'satis'
                 GROUP BY customer_id",
            )
            .expect("satış toplamı sorgusu");
        let rows = statement
            .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))
            .expect("satış toplamı satırları");
        for row in rows {
            let (id, total) = row.expect("satış toplamı okunmalı");
            sales.insert(id, total);
        }
    }
    {
        let mut statement = connection
            .prepare(
                "SELECT customer_id, SUM(grand_total) FROM invoices
                 WHERE status = 'kesin' AND invoice_type = 'alis'
                 GROUP BY customer_id",
            )
            .expect("alış toplamı sorgusu");
        let rows = statement
            .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))
            .expect("alış toplamı satırları");
        for row in rows {
            let (id, total) = row.expect("alış toplamı okunmalı");
            purchases.insert(id, total);
        }
    }
    {
        let mut statement = connection
            .prepare(
                "SELECT customer_id, SUM(amount) FROM payments
                 WHERE status = 'kesin' AND payment_type = 'tahsilat'
                 GROUP BY customer_id",
            )
            .expect("tahsilat toplamı sorgusu");
        let rows = statement
            .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))
            .expect("tahsilat toplamı satırları");
        for row in rows {
            let (id, total) = row.expect("tahsilat toplamı okunmalı");
            collections.insert(id, total);
        }
    }
    {
        let mut statement = connection
            .prepare(
                "SELECT customer_id, SUM(amount) FROM payments
                 WHERE status = 'kesin' AND payment_type = 'odeme'
                 GROUP BY customer_id",
            )
            .expect("ödeme toplamı sorgusu");
        let rows = statement
            .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))
            .expect("ödeme toplamı satırları");
        for row in rows {
            let (id, total) = row.expect("ödeme toplamı okunmalı");
            payments.insert(id, total);
        }
    }

    let customers = database
        .customer_list(&all_customers())
        .expect("cari listesi okunmalı");
    for customer in customers {
        let expected = sales.get(&customer.id).copied().unwrap_or(0) as i128
            - purchases.get(&customer.id).copied().unwrap_or(0) as i128
            - collections.get(&customer.id).copied().unwrap_or(0) as i128
            + payments.get(&customer.id).copied().unwrap_or(0) as i128;
        let actual = i128::from(customer.balance().kurus());
        assert_eq!(
            actual, expected,
            "cari {} ({}) bakiyesi tutarsız",
            customer.code, customer.id
        );
    }

    // Fatura başlığı ile satır toplamları.
    let line_bad: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM (
                 SELECT i.id
                 FROM invoices i
                 LEFT JOIN invoice_lines il ON il.invoice_id = i.id
                 GROUP BY i.id, i.subtotal, i.discount_total, i.net_total, i.vat_total, i.grand_total
                 HAVING COUNT(il.id) = 0
                     OR i.subtotal <> COALESCE(SUM(il.line_gross), 0)
                     OR i.discount_total <> COALESCE(SUM(il.line_discount), 0)
                     OR i.net_total <> COALESCE(SUM(il.line_net), 0)
                     OR i.vat_total <> COALESCE(SUM(il.line_vat), 0)
                     OR i.grand_total <> COALESCE(SUM(il.line_total), 0)
             )",
            [],
            |row| row.get(0),
        )
        .expect("fatura satır toplamı korelasyonu");
    assert_eq!(
        line_bad, 0,
        "{line_bad} faturada başlık ve satır toplamları uyuşmuyor"
    );

    // Zorunlu alanlarda NULL olmamalı.
    let required: &[(&str, &[&str])] = &[
        ("app_metadata", &["key", "value"]),
        ("schema_migrations", &["version", "name"]),
        (
            "customers",
            &[
                "id",
                "code",
                "name",
                "kind",
                "active",
                "created_at",
                "updated_at",
            ],
        ),
        (
            "products",
            &[
                "id",
                "code",
                "name",
                "unit",
                "purchase_price",
                "sale_price",
                "vat_rate",
                "min_stock",
                "current_stock",
                "active",
                "created_at",
                "updated_at",
            ],
        ),
        (
            "product_stock_movements",
            &[
                "id",
                "product_id",
                "movement_type",
                "quantity_delta",
                "movement_date",
                "created_at",
            ],
        ),
        (
            "invoices",
            &[
                "id",
                "invoice_number",
                "invoice_type",
                "customer_id",
                "invoice_date",
                "subtotal",
                "discount_total",
                "net_total",
                "vat_total",
                "grand_total",
                "paid_amount",
                "status",
                "created_at",
                "updated_at",
            ],
        ),
        (
            "invoice_lines",
            &[
                "id",
                "invoice_id",
                "product_id",
                "line_no",
                "product_code",
                "product_name",
                "unit",
                "quantity",
                "unit_price",
                "discount_rate",
                "vat_rate",
                "line_gross",
                "line_discount",
                "line_net",
                "line_vat",
                "line_total",
                "created_at",
            ],
        ),
        (
            "cash_accounts",
            &[
                "id",
                "name",
                "account_type",
                "opening_balance",
                "current_balance",
                "active",
                "created_at",
                "updated_at",
            ],
        ),
        (
            "payments",
            &[
                "id",
                "payment_type",
                "customer_id",
                "cash_account_id",
                "payment_date",
                "amount",
                "status",
                "created_at",
                "updated_at",
            ],
        ),
        (
            "payment_allocations",
            &["id", "payment_id", "invoice_id", "amount", "created_at"],
        ),
        (
            "expense_categories",
            &[
                "id",
                "name",
                "category_type",
                "active",
                "created_at",
                "updated_at",
            ],
        ),
        (
            "income_expense_entries",
            &[
                "id",
                "entry_type",
                "category_id",
                "cash_account_id",
                "entry_date",
                "amount",
                "status",
                "created_at",
                "updated_at",
            ],
        ),
        (
            "users",
            &[
                "id",
                "username",
                "password_hash",
                "role",
                "active",
                "created_at",
                "updated_at",
            ],
        ),
        ("app_settings", &["key", "value", "updated_at"]),
    ];

    for (table, columns) in required {
        let condition = columns
            .iter()
            .map(|column| format!("{column} IS NULL"))
            .collect::<Vec<_>>()
            .join(" OR ");
        let sql = format!("SELECT COUNT(*) FROM {table} WHERE {condition}");
        let null_count: i64 = connection
            .query_row(&sql, [], |row| row.get(0))
            .expect("NULL kontrolü çalışmalı");
        assert_eq!(null_count, 0, "{table} tablosunda zorunlu alan NULL");
    }
}

/// Testin çalıştığı makinedeki uygulama yürütülebilir dosyasını bulur.
///
/// Cargo, entegrasyon testlerinde `CARGO_BIN_EXE_OnMuhasebe` değişkenini
/// tanımlar. Değişken yoksa (örneğin test ikilisi doğrudan çalıştırıldıysa)
/// `target/{debug,release}/OnMuhasebe.exe` yolu denenir.
pub fn app_executable() -> PathBuf {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_OnMuhasebe") {
        let path = PathBuf::from(path);
        if path.exists() {
            return path;
        }
    }

    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(profile)
        .join("OnMuhasebe.exe");
    assert!(
        path.exists(),
        "uygulama yürütülebilir dosyası bulunamadı: {} (önce cargo build --release)",
        path.display()
    );
    path
}

/// Uygulamayı ayrı bir süreç olarak başlatır.
///
/// Veri dizini `ON_MUHASEBE_DATA_DIR` ile geçici test dizinine yönlendirilir;
/// gerçek `%LOCALAPPDATA%\OnMuhasebe` verisine dokunulmaz. Pencere açılışı
/// test ortamında görünür olabilir; süreç test sonunda zorla kapatılır.
pub fn spawn_app_process(env_dir: impl AsRef<Path>) -> Child {
    let executable = app_executable();
    Command::new(executable)
        .env("ON_MUHASEBE_DATA_DIR", env_dir.as_ref())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("uygulama süreci başlatılmalı")
}

/// Süreci işletim sistemi düzeyinde zorla öldürür.
///
/// Windows'ta `taskkill /F /PID` kullanılır; bu, uygulamanın WAL'ı düzgün
/// kapatmasına fırsat vermez ve gerçek güç kesintisine en yakın testtir.
pub fn kill_process_hard(mut child: Child) {
    #[cfg(windows)]
    {
        let pid = child.id().to_string();
        let _ = Command::new("taskkill")
            .args(["/F", "/PID", &pid])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(not(windows))]
    {
        let _ = child.kill();
    }
    let _ = child.wait();
}

/// Mevcut test sürecinin RSS (çalışma kümesi) değerini MB cinsinden ölçer.
pub fn measure_memory_mb() -> u64 {
    let pid = sysinfo::Pid::from_u32(std::process::id());
    let mut system = sysinfo::System::new_all();
    system.refresh_all();
    system
        .process(pid)
        .map(|process| process.memory() / (1024 * 1024))
        .unwrap_or(0)
}

/// Verilen yolun bulunduğu diskte kullanılabilir boş alanı MB cinsinden döndürür.
pub fn disk_free_mb(path: impl AsRef<Path>) -> u64 {
    let path = path.as_ref();
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let mut best: Option<(usize, u64)> = None;
    for disk in &disks {
        let mount = disk.mount_point();
        if path.starts_with(mount) {
            let depth = mount.components().count();
            if best.is_none_or(|(best_depth, _)| depth >= best_depth) {
                best = Some((depth, disk.available_space()));
            }
        }
    }
    best.map(|(_, bytes)| bytes / (1024 * 1024)).unwrap_or(0)
}

/// Koşul sağlanana kadar kısa aralıklarla bekler.
///
/// `timeout_ms` içinde koşul `true` olursa `true`, süre dolarsa `false` döner.
pub fn wait_for_condition(mut condition: impl FnMut() -> bool, timeout_ms: u64) -> bool {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        if condition() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(50));
    }
}
