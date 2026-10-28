// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! 100 turluk soak testi: üretim yollarıyla sürekli yazma, yedekleme ve doğrulama.

use std::time::Instant;

use on_muhasebe::db::Database;
use on_muhasebe::income_expense::{EntryDraft, EntryType};
use on_muhasebe::invoice::{InvoiceDraft, InvoiceLineDraft, InvoiceType};
use on_muhasebe::money::Money;
use on_muhasebe::payment::{AllocationDraft, PaymentDraft, PaymentType};
use on_muhasebe::product::Unit;

use super::{assert_correlations, build_test_db, seed_minimal};

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
    let validated = draft.validate().expect("soak faturası doğrulanmalı");
    let grand = validated.totals.grand_total;
    let id = database
        .invoice_insert(&validated)
        .expect("soak faturası yazılmalı");
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
        description: "soak ödemesi".to_owned(),
        allocations: vec![AllocationDraft {
            invoice_id,
            invoice_number: number.to_owned(),
            invoice_date: date.to_owned(),
            grand_total: grand,
            paid_amount: Money::ZERO,
            amount: half.format_turkish(),
        }],
    };
    let validated = draft.validate().expect("soak ödemesi doğrulanmalı");
    database
        .payment_insert(&validated)
        .expect("soak ödemesi yazılmalı");
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
        description: "soak gelir/gider".to_owned(),
        document_ref: String::new(),
    };
    let validated = draft.validate().expect("soak gelir/gider doğrulanmalı");
    database
        .income_expense_insert(&validated)
        .expect("soak gelir/gider yazılmalı");
}

/// 100 tur × (100 fatura + 100 ödeme + 50 gelir/gider + 1 yedek + tam doğrulama).
#[test]
#[ignore = "ağır test: cargo test --release -- --ignored"]
fn soak_100_tur() {
    let directory = tempfile::tempdir().expect("geçici dizin");
    let path = directory.path().join("soak.db");
    let backup_path = directory.path().join("soak-yedek.onm-backup");
    let mut database = build_test_db(&path);
    let seed = seed_minimal(&database);

    let total_start = Instant::now();
    for round in 0..100 {
        let round_start = Instant::now();
        let date = format!("2026-{:02}-{:02}", (round % 12) + 1, (round % 28) + 1);
        let mut invoices = Vec::with_capacity(100);

        for index in 0..100usize {
            let sales = index.is_multiple_of(2);
            let number = format!("SOAK-{round:03}-{index:03}");
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
        assert_eq!(invoices.len(), 100);

        for index in 0..50usize {
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

        database.checkpoint().expect("soak checkpoint");
        std::fs::copy(database.path().unwrap(), &backup_path).expect("soak yedeği");
        Database::validate_backup(&backup_path).expect("soak yedeği geçerli olmalı");

        drop(database);
        database = build_test_db(&path);
        assert_correlations(&database);

        if round % 10 == 9 {
            println!(
                "[SOAK] tur {} tamamlandı (tur {} ms, toplam {} ms)",
                round + 1,
                round_start.elapsed().as_millis(),
                total_start.elapsed().as_millis()
            );
        }
    }

    assert_eq!(database.invoice_count().unwrap(), 10_000);
    assert_eq!(database.payment_count().unwrap(), 10_000);
    assert_eq!(database.income_expense_count().unwrap(), 5_000);
    assert_correlations(&database);
    println!(
        "[SOAK] toplam süre: {} ms",
        total_start.elapsed().as_millis()
    );
}
