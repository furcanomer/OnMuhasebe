// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Büyük veri, korelasyon ve uzun belge testleri.

use std::time::Instant;

use on_muhasebe::customer::CustomerType;
use on_muhasebe::error::AppError;
use on_muhasebe::invoice::{
    InvoiceDraft, InvoiceFilter, InvoiceLineDraft, InvoiceType, calculate_line,
};
use on_muhasebe::money::Money;
use on_muhasebe::product::Unit;
use on_muhasebe::quantity::Quantity;
use on_muhasebe::report::StockReportFilter;

use super::bulk_data::{BulkSeed, seed_bulk, seed_customers_only, seed_stock_movements};
use super::{
    all_customers, all_products, assert_correlations, build_test_db, run_with_timeout, seed_minimal,
};

fn elapsed_ms(start: Instant) -> u128 {
    start.elapsed().as_millis()
}

fn report(label: &str, millis: u128) {
    println!("[STRESS] {label}: {millis} ms");
}

/// i64::MIN para/miktar değerlerinin metne ve geriye kayıpsız dönüşü.
///
/// Fuzz sırasında bulunan taşma sınırı için kalıcı regresyon testi.
#[test]
fn i64_min_degerleri_bicimden_geri_okunur() {
    let money = Money::from_kurus(i64::MIN);
    let text = money.format_turkish();
    assert_eq!(
        Money::parse(&text).expect("i64::MIN tutarı okunabilmeli"),
        money,
        "para biçimi geri okunamadı: {text}"
    );

    let quantity = Quantity::from_milli(i64::MIN);
    let text = quantity.format_turkish();
    assert_eq!(
        Quantity::parse(&text).expect("i64::MIN miktarı okunabilmeli"),
        quantity,
        "miktar biçimi geri okunamadı: {text}"
    );
}

/// Minimal veride korelasyon yardımcısının kendisi çalışıyor mu?
#[test]
fn korelasyon_yardimcisi_minimal_veride_gecer() {
    let directory = tempfile::tempdir().expect("geçici dizin");
    let database = build_test_db(&directory.path().join("minimal.db"));
    seed_minimal(&database);
    assert_correlations(&database);
}

/// Senaryo A: 1.000 cari, 1.000 ürün, 10.000 fatura, 10.000 ödeme, 5.000 gelir/gider.
#[test]
#[ignore = "ağır test: cargo test --release -- --ignored"]
fn senaryo_a_10k_fatura() {
    let directory = tempfile::tempdir().expect("geçici dizin");
    let database = build_test_db(&directory.path().join("a.db"));

    let start = Instant::now();
    let seed: BulkSeed = seed_bulk(&database, 1_000, 1_000, 10_000, 10_000, 5_000);
    report(
        "A seed (1000 cari, 1000 urun, 10k fatura, 10k odeme, 5k gelir/gider)",
        elapsed_ms(start),
    );
    assert_eq!(database.customer_count().unwrap(), 1_000);
    assert_eq!(database.product_count().unwrap(), 1_000);
    assert_eq!(database.invoice_count().unwrap(), seed.invoice_count);

    let start = Instant::now();
    let customers = database.customer_list(&all_customers()).unwrap();
    assert_eq!(customers.len(), 1_000);
    let customer_ms = elapsed_ms(start);
    report("A cari listesi filtresiz", customer_ms);
    assert!(
        customer_ms < 2_000,
        "cari listesi 2 sn'yi aştı: {customer_ms} ms"
    );

    let start = Instant::now();
    let products = database.product_list(&all_products()).unwrap();
    assert_eq!(products.len(), 1_000);
    let product_ms = elapsed_ms(start);
    report("A urun listesi filtresiz", product_ms);
    assert!(
        product_ms < 2_000,
        "urun listesi 2 sn'yi aştı: {product_ms} ms"
    );

    let start = Instant::now();
    let sales = database
        .invoice_list(InvoiceType::Sales, &InvoiceFilter::default())
        .unwrap();
    let purchases = database
        .invoice_list(InvoiceType::Purchase, &InvoiceFilter::default())
        .unwrap();
    let invoice_ms = elapsed_ms(start);
    report("A fatura listesi (satis+alis, filtresiz)", invoice_ms);
    assert_eq!(sales.len() + purchases.len(), seed.invoice_count);
    assert!(
        invoice_ms < 2_000,
        "fatura listesi 2 sn'yi aştı: {invoice_ms} ms"
    );

    let start = Instant::now();
    let balance = database
        .customer_balance_report("", "", &Default::default())
        .unwrap();
    let balance_ms = elapsed_ms(start);
    report("A cari bakiye ozeti raporu", balance_ms);
    assert_eq!(balance.rows.len(), 1_000);
    assert!(
        balance_ms < 2_000,
        "bakiye raporu 2 sn'yi aştı: {balance_ms} ms"
    );

    let start = Instant::now();
    let stock = database
        .stock_status_report(&StockReportFilter::default())
        .unwrap();
    let stock_ms = elapsed_ms(start);
    report("A stok raporu", stock_ms);
    assert_eq!(stock.rows.len(), 1_000);
    assert!(stock_ms < 2_000, "stok raporu 2 sn'yi aştı: {stock_ms} ms");

    let start = Instant::now();
    let _income = database.income_expense_report("", "").unwrap();
    let income_ms = elapsed_ms(start);
    report("A gelir/gider ozeti", income_ms);
    assert!(
        income_ms < 2_000,
        "gelir/gider raporu 2 sn'yi aştı: {income_ms} ms"
    );

    let start = Instant::now();
    assert_correlations(&database);
    report("A korelasyon ve integrity", elapsed_ms(start));
}

/// Senaryo B: 1.000.000 cari; filtresiz liste, arama, tür ve durum filtreleri.
#[test]
#[ignore = "ağır test: cargo test --release -- --ignored"]
fn senaryo_b_1m_cari() {
    let directory = tempfile::tempdir().expect("geçici dizin");
    let path = directory.path().join("b.db");

    run_with_timeout(
        move || {
            let database = build_test_db(&path);
            let start = Instant::now();
            let ids = seed_customers_only(&database, 1_000_000);
            let seed_ms = elapsed_ms(start);
            report("B seed (1.000.000 cari)", seed_ms);
            assert_eq!(ids.len(), 1_000_000);
            assert_eq!(database.customer_count().unwrap(), 1_000_000);

            let start = Instant::now();
            let all = database.customer_list(&all_customers()).unwrap();
            let all_ms = elapsed_ms(start);
            report("B cari listesi filtresiz", all_ms);
            assert_eq!(all.len(), 1_000_000);
            drop(all);
            assert!(
                all_ms < 5_000,
                "1M cari filtresiz liste 5 sn'yi aştı: {all_ms} ms"
            );

            let start = Instant::now();
            let found = database
                .customer_list(&on_muhasebe::customer::CustomerFilter {
                    search: "C0999999".to_owned(),
                    ..Default::default()
                })
                .unwrap();
            let search_ms = elapsed_ms(start);
            report("B cari arama", search_ms);
            assert_eq!(found.len(), 1);
            drop(found);
            assert!(
                search_ms < 5_000,
                "1M cari arama 5 sn'yi aştı: {search_ms} ms"
            );

            let start = Instant::now();
            let kind = database
                .customer_list(&on_muhasebe::customer::CustomerFilter {
                    kind: Some(CustomerType::Customer),
                    ..Default::default()
                })
                .unwrap();
            let kind_ms = elapsed_ms(start);
            report("B cari tur filtresi", kind_ms);
            assert!(!kind.is_empty());
            drop(kind);
            assert!(
                kind_ms < 5_000,
                "1M cari tür filtresi 5 sn'yi aştı: {kind_ms} ms"
            );

            let start = Instant::now();
            let active = database
                .customer_list(&on_muhasebe::customer::CustomerFilter {
                    active: Some(true),
                    ..Default::default()
                })
                .unwrap();
            let active_ms = elapsed_ms(start);
            report("B cari durum filtresi", active_ms);
            assert!(!active.is_empty());
            drop(active);
            assert!(
                active_ms < 5_000,
                "1M cari durum filtresi 5 sn'yi aştı: {active_ms} ms"
            );

            let start = Instant::now();
            assert_correlations(&database);
            report("B korelasyon ve integrity", elapsed_ms(start));
        },
        900,
    );
}

/// Senaryo C: 100 ürün × 100.000 stok hareketi (10.000.000 satır).
#[test]
#[ignore = "ağır test: cargo test --release -- --ignored"]
fn senaryo_c_10m_stok_hareketi() {
    let directory = tempfile::tempdir().expect("geçici dizin");
    let database = build_test_db(&directory.path().join("c.db"));

    let start = Instant::now();
    let product_ids = seed_stock_movements(&database, 100, 100_000);
    report(
        "C seed (100 urun x 100.000 hareket = 10M satir)",
        elapsed_ms(start),
    );
    assert_eq!(product_ids.len(), 100);
    assert_eq!(database.product_count().unwrap(), 100);
    let movement_count: i64 = super::raw_connection(&database)
        .query_row("SELECT COUNT(*) FROM product_stock_movements", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(movement_count, 10_000_000);

    let start = Instant::now();
    assert_correlations(&database);
    report("C stok korelasyonu + integrity", elapsed_ms(start));

    let start = Instant::now();
    let products = database.product_list(&all_products()).unwrap();
    let product_ms = elapsed_ms(start);
    report("C urun listesi", product_ms);
    assert_eq!(products.len(), 100);
    assert!(
        product_ms < 2_000,
        "C urun listesi 2 sn'yi aştı: {product_ms} ms"
    );

    let start = Instant::now();
    let stock = database
        .stock_status_report(&StockReportFilter::default())
        .unwrap();
    let stock_ms = elapsed_ms(start);
    report("C stok raporu", stock_ms);
    assert_eq!(stock.rows.len(), 100);
    assert!(
        stock_ms < 2_000,
        "C stok raporu 2 sn'yi aştı: {stock_ms} ms"
    );
}

/// Senaryo D: tek cariye 50.000 fatura + 50.000 tahsilat/ödeme.
#[test]
#[ignore = "ağır test: cargo test --release -- --ignored"]
fn senaryo_d_50k_ekstre() {
    let directory = tempfile::tempdir().expect("geçici dizin");
    let database = build_test_db(&directory.path().join("d.db"));

    let start = Instant::now();
    let seed = seed_bulk(&database, 1, 1, 50_000, 50_000, 0);
    report(
        "D seed (50k fatura + 50k odeme, tek cari)",
        elapsed_ms(start),
    );

    let start = Instant::now();
    let full = database
        .customer_statement(seed.customer_id, "", "")
        .unwrap()
        .expect("cari ekstresi");
    let full_ms = elapsed_ms(start);
    report("D cari ekstre araliksiz (100k hareket)", full_ms);
    assert!(full.rows.len() >= 100_000);
    assert!(
        full_ms < 2_000,
        "aralıksız ekstre 2 sn'yi aştı: {full_ms} ms"
    );

    let start = Instant::now();
    let monthly = database
        .customer_statement(seed.customer_id, "2026-01-01", "2026-01-31")
        .unwrap()
        .expect("ocak ekstresi");
    let monthly_ms = elapsed_ms(start);
    report("D cari ekstre tarih aralikli (Ocak)", monthly_ms);
    assert!(!monthly.rows.is_empty());
    assert!(
        monthly_ms < 2_000,
        "aralıklı ekstre 2 sn'yi aştı: {monthly_ms} ms"
    );

    let start = Instant::now();
    assert_correlations(&database);
    report("D korelasyon ve integrity", elapsed_ms(start));
}

/// Senaryo E: 1.000 satırlı tek fatura; KDV, iskonto, toplam ve taşma kontrolü.
#[test]
#[ignore = "ağır test: cargo test --release -- --ignored"]
fn senaryo_e_1000_satirli_fatura() {
    let directory = tempfile::tempdir().expect("geçici dizin");
    let database = build_test_db(&directory.path().join("e.db"));
    let seed = seed_minimal(&database);

    let overflow = calculate_line(
        Quantity::from_milli(i64::MAX),
        Money::from_kurus(i64::MAX),
        0,
        0,
    );
    assert!(matches!(overflow, Err(AppError::AmountOverflow)));

    let mut draft = InvoiceDraft::new(InvoiceType::Sales);
    draft.number = "SF-UZUN-0001".to_owned();
    draft.customer_id = Some(seed.customer_id);
    draft.invoice_date = "2026-10-23".to_owned();
    draft.lines = (0..1_000)
        .map(|_| InvoiceLineDraft {
            product_id: Some(seed.product_id),
            product_code: "URN-0001".to_owned(),
            product_name: "Test Ürün".to_owned(),
            unit: Unit::Adet,
            description: String::new(),
            quantity: "1".to_owned(),
            unit_price: "1.000,00".to_owned(),
            discount_rate: "10".to_owned(),
            vat_rate: "20".to_owned(),
        })
        .collect();

    let validated = draft.validate().expect("1000 satırlı fatura doğrulanmalı");
    assert_eq!(validated.line_count(), 1_000);
    assert_eq!(validated.totals.subtotal.kurus(), 100_000_000);
    assert_eq!(validated.totals.discount_total.kurus(), 10_000_000);
    assert_eq!(validated.totals.net_total.kurus(), 90_000_000);
    assert_eq!(validated.totals.vat_total.kurus(), 18_000_000);
    assert_eq!(validated.totals.grand_total.kurus(), 108_000_000);

    let start = Instant::now();
    let invoice_id = database.invoice_insert(&validated).unwrap();
    report("E 1000 satirli fatura kaydi", elapsed_ms(start));

    let invoice = database.invoice_get(invoice_id).unwrap().unwrap();
    assert_eq!(invoice.totals, validated.totals);
    let lines = database.invoice_lines(invoice_id).unwrap();
    assert_eq!(lines.len(), 1_000);
    let mut total = Money::ZERO;
    for line in &lines {
        total = total.checked_add(line.amounts.total).unwrap();
    }
    assert_eq!(total, invoice.totals.grand_total);
    assert_correlations(&database);
}
