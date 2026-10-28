// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Fuzz testleri: para/miktar ayrıştırma, CSV ve yedek dosyası.

use std::time::Instant;

use on_muhasebe::app::CUSTOMER_TEMPLATE_HEADERS;
use on_muhasebe::app::OnMuhasebeApp;
use on_muhasebe::config::AppConfig;
use on_muhasebe::csv;
use on_muhasebe::db::Database;
use on_muhasebe::money::Money;
use on_muhasebe::quantity::Quantity;

use super::{assert_correlations, build_test_db, raw_connection, seed_minimal};

/// Deterministik xorshift64* üreteci (yeni bağımlılık gerektirmez).
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    fn next_u64(&mut self) -> u64 {
        let mut value = self.0;
        value ^= value >> 12;
        value ^= value << 25;
        value ^= value >> 27;
        self.0 = value;
        value.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, bound: usize) -> usize {
        if bound == 0 {
            0
        } else {
            (self.next_u64() as usize) % bound
        }
    }
}

const ALPHABET: &[char] = &[
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '.', ',', '+', '-', '_', ' ', '\t', '\n',
    '\'', 'e', 'E', 'N', 'a', 'n', 'i', 'f', 'k', 'g', '₺', '$', '€', 'I', 'İ', 'ş', 'ğ', 'ü', '٣',
    '٤', '１', '�', '/', ':', ';', '"', '%',
];

fn random_text(rng: &mut Rng, max_len: usize) -> String {
    let length = rng.below(max_len + 1);
    let mut text = String::with_capacity(length);
    for _ in 0..length {
        text.push(ALPHABET[rng.below(ALPHABET.len())]);
    }
    text
}

fn curated_inputs() -> Vec<String> {
    let mut values = vec![
        String::new(),
        " ".to_owned(),
        "+".to_owned(),
        "-".to_owned(),
        ".".to_owned(),
        ",".to_owned(),
        "1e5".to_owned(),
        "NaN".to_owned(),
        "inf".to_owned(),
        "-inf".to_owned(),
        "1_000".to_owned(),
        "1 000".to_owned(),
        "1.234,56".to_owned(),
        "1,234.56".to_owned(),
        "1..2".to_owned(),
        "1,,2".to_owned(),
        "１.２３４".to_owned(),
        "١٢٣".to_owned(),
        "999999999999999999999999999999".to_owned(),
        "-92233720368547758,08".to_owned(),
        "92233720368547758,07".to_owned(),
        "00000000000000000000000000000000000000000000000001".to_owned(),
    ];
    values.push("9".repeat(10_000));
    values.push("1".repeat(10_000) + "," + &"2".repeat(10_000));
    values
}

/// 1.000.000 rastgele girdi: parse ya Ok ya Err; panik yok.
#[test]
#[ignore = "ağır test: cargo test --release -- --ignored"]
fn money_quantity_milyon_girdi() {
    let start = Instant::now();
    let curated = curated_inputs();
    let mut rng = Rng::new(0xA5A5_1234_5678_9ABC);
    let mut money_ok = 0usize;
    let mut money_err = 0usize;
    let mut quantity_ok = 0usize;
    let mut quantity_err = 0usize;

    for index in 0..1_000_000 {
        let text = if index < curated.len() {
            curated[index].clone()
        } else if index.is_multiple_of(10_000) {
            "7".repeat(1_000)
        } else {
            random_text(&mut rng, 40)
        };

        match Money::parse(&text) {
            Ok(money) => {
                money_ok += 1;
                let formatted = money.format_turkish();
                assert_eq!(
                    Money::parse(&formatted).expect("para biçimi okunmalı"),
                    money,
                    "para geri okunamadı: {text:?} -> {formatted:?}"
                );
            }
            Err(_) => money_err += 1,
        }

        match Quantity::parse(&text) {
            Ok(quantity) => {
                quantity_ok += 1;
                let formatted = quantity.format_turkish();
                assert_eq!(
                    Quantity::parse(&formatted).expect("miktar biçimi okunmalı"),
                    quantity,
                    "miktar geri okunamadı: {text:?} -> {formatted:?}"
                );
            }
            Err(_) => quantity_err += 1,
        }
    }

    println!(
        "[FUZZ] para: {money_ok} Ok / {money_err} Err; miktar: {quantity_ok} Ok / {quantity_err} Err; süre {} ms",
        start.elapsed().as_millis()
    );
}

/// CSV yapısal fuzz: 1.000 rastgele metin; parse ya tablo ya hata döndürür.
#[test]
fn csv_yapisal_fuzz() {
    let mut rng = Rng::new(0x1234_5678_9ABC_DEF0);
    let mut ok = 0usize;
    let mut err = 0usize;
    for _ in 0..1_000 {
        let text = random_text(&mut rng, 120);
        match csv::parse(&text) {
            Ok(table) => {
                ok += 1;
                for row in &table.rows {
                    for cell in row {
                        let _ = cell.len();
                    }
                }
            }
            Err(_) => err += 1,
        }
    }
    println!("[FUZZ] CSV yapısal: {ok} Ok / {err} Err");
}

fn safe_random_cell(rng: &mut Rng) -> String {
    const SAFE: &[char] = &[
        'A', 'B', 'C', 'a', 'b', 'c', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', ' ', '.',
        ',', '-', '+', '_', 'I', 'İ', 'ş', 'ğ', 'ü',
    ];
    let length = rng.below(18);
    let mut text = String::with_capacity(length);
    for _ in 0..length {
        text.push(SAFE[rng.below(SAFE.len())]);
    }
    text
}

/// 1.000 bozuk/rastgele iş satırı: her satır Hazır veya Hata olmalı.
#[test]
fn csv_ice_aktarma_bin_satir_fuzz() {
    let directory = tempfile::tempdir().expect("geçici dizin");
    let data_dir = directory.path().join("veri");
    let config = AppConfig::with_data_dir(&data_dir);
    let context = egui::Context::default();
    let mut app = OnMuhasebeApp::with_config(&context, config.clone());

    let mut rng = Rng::new(0xDEAD_BEEF_1234_5678);
    let mut rows: Vec<Vec<String>> = vec![
        CUSTOMER_TEMPLATE_HEADERS
            .iter()
            .map(|s| (*s).to_owned())
            .collect(),
    ];
    for index in 0..1_000 {
        let mut row = vec![String::new(); CUSTOMER_TEMPLATE_HEADERS.len()];
        if index % 3 == 0 {
            row[0] = format!("FZ{index:05}");
            row[1] = format!("Fuzz Cari {index}");
            row[2] = "musteri".to_owned();
            row[11] = "Evet".to_owned();
        } else {
            for cell in &mut row {
                *cell = safe_random_cell(&mut rng);
            }
        }
        rows.push(row);
    }
    let path = directory.path().join("fuzz.csv");
    csv::write_rows(&path, &rows).expect("CSV yazılmalı");
    app.preview_customer_import(&path)
        .expect("CSV önizlemesi oluşmalı");

    let (ready, errors) = {
        let preview = app
            .transfer_state()
            .import_preview
            .as_ref()
            .expect("önizleme olmalı");
        assert_eq!(preview.rows.len(), 1_000, "her veri satırı temsil edilmeli");
        for row in &preview.rows {
            assert!(
                row.status.is_ready() || row.status.reason().is_some(),
                "satır Hazır veya Hata olmalı"
            );
        }
        (preview.ready_count(), preview.error_count())
    };
    assert_eq!(ready + errors, 1_000);
    assert!(ready > 0, "en az bir geçerli satır olmalı");

    app.confirm_import().expect("hazır satırlar aktarılmalı");
    let result = app
        .transfer_state()
        .import_result
        .as_ref()
        .expect("içe aktarma sonucu");
    let inserted = result.inserted;
    assert_eq!(
        inserted + result.skipped.len(),
        1_000,
        "her satır ya eklenmeli ya atlanmalı"
    );
    assert_eq!(inserted, ready, "hazır satırların tümü eklenmeli");

    drop(app);
    let database = Database::open(config.database_path()).expect("veritabanı açılmalı");
    assert_eq!(database.customer_count().unwrap(), inserted);
    assert_correlations(&database);
    println!("[FUZZ] CSV iş satırı: {ready} hazır / {errors} hata");
}

/// Geçerli bir yedeğin ilk 100 baytını bozan 50 varyant.
#[test]
fn yedek_ilk_100_bayt_fuzz() {
    let directory = tempfile::tempdir().expect("geçici dizin");
    let database = build_test_db(&directory.path().join("backup-source.db"));
    seed_minimal(&database);
    database.checkpoint().expect("checkpoint");
    let backup_path = directory.path().join("gecerli.onm-backup");
    std::fs::copy(database.path().unwrap(), &backup_path).expect("geçerli yedek");

    let original = std::fs::read(&backup_path).expect("yedek okunmalı");
    let mut rng = Rng::new(0x0BAD_F00D_CAFE_1234);
    let mut rejected = 0usize;
    let before = database.customer_count().unwrap();

    for index in 0..50usize {
        let mut bytes = original.clone();
        // SQLite sihirli başlığı ilk 16 bayttır. Her varyantta bu bölümden
        // bir baytı, ayrıca yarısında 17-100 aralığından bir baytı boz.
        let header_position = rng.below(16);
        bytes[header_position] ^= (rng.next_u64() as u8) | 1;
        if index.is_multiple_of(2) {
            let upper = 100.min(bytes.len());
            let position = 16 + rng.below(upper - 16);
            bytes[position] ^= (rng.next_u64() as u8) | 1;
        }
        let variant = directory
            .path()
            .join(format!("bozuk-{index:02}.onm-backup"));
        std::fs::write(&variant, &bytes).expect("varyant yazılmalı");

        let result = Database::validate_backup(&variant);
        assert!(
            result.is_err(),
            "bozuk başlıklı yedek kabul edilmemeli: {index}"
        );
        rejected += 1;
    }

    assert_eq!(rejected, 50);
    assert_eq!(
        database.customer_count().unwrap(),
        before,
        "mevcut veritabanı değişmemeli"
    );
    assert_correlations(&database);
    println!("[FUZZ] yedek başlığı: {rejected} / 50 reddedildi, mevcut DB değişmedi");
}

/// Şema sürümü sahte biçimde yükseltilirse SchemaTooNew; veri korunur.
#[test]
fn sahte_yuksek_migration_surumu() {
    let directory = tempfile::tempdir().expect("geçici dizin");
    let path = directory.path().join("newer.db");
    let database = build_test_db(&path);
    seed_minimal(&database);
    database.checkpoint().expect("checkpoint");
    drop(database);

    let connection = raw_connection(&Database::open(&path).expect("yeniden açılmalı"));
    connection
        .execute(
            "INSERT INTO schema_migrations (version, name) VALUES (8, 'sahte')",
            [],
        )
        .expect("sahte sürüm yazılmalı");
    drop(connection);

    let result = Database::open(&path);
    assert!(matches!(
        result,
        Err(on_muhasebe::error::AppError::SchemaTooNew {
            found: 8,
            supported: 7
        })
    ));

    let connection = rusqlite::Connection::open(&path).expect("veri dosyası açılmalı");
    let customers: i64 = connection
        .query_row("SELECT COUNT(*) FROM customers", [], |row| row.get(0))
        .expect("cari sayısı");
    assert_eq!(customers, 1);
}
