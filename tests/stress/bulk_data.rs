// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Deterministik büyük veri üreticileri.

use on_muhasebe::db::Database;
use rusqlite::types::Value;

use super::{insert_batch, raw_connection};

/// Üretilen büyük veri setinin özeti.
#[derive(Debug, Clone, Copy)]
pub struct BulkSeed {
    pub customer_id: i64,
    pub product_id: i64,
    pub cash_account_id: i64,
    pub invoice_count: usize,
    pub payment_count: usize,
    pub entry_count: usize,
}

struct InvoiceSeed {
    id: i64,
    is_sales: bool,
    customer_id: i64,
    grand_total: i64,
    paid_amount: i64,
    date: String,
}

fn bulk_pragmas(connection: &rusqlite::Connection) {
    connection
        .execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = OFF;
             PRAGMA temp_store = MEMORY;
             PRAGMA cache_size = -200000;
             PRAGMA wal_autocheckpoint = 10000;",
        )
        .expect("toplu yazma ayarları");
}

fn customer_rows(start: usize, count: usize) -> Vec<Vec<Value>> {
    (0..count)
        .map(|offset| {
            let index = start + offset + 1;
            let kind = match index % 3 {
                0 => "musteri",
                1 => "tedarikci",
                _ => "ikisi",
            };
            vec![
                Value::Text(format!("C{index:07}")),
                Value::Text(format!("Cari {index:07}")),
                Value::Text(kind.to_owned()),
                Value::Integer(i64::from(!index.is_multiple_of(5))),
            ]
        })
        .collect()
}

fn insert_customers(connection: &rusqlite::Connection, count: usize) {
    const BATCH: usize = 1000;
    let mut inserted = 0;
    while inserted < count {
        let chunk = (count - inserted).min(BATCH);
        let rows = customer_rows(inserted, chunk);
        insert_batch(connection, "customers", "code, name, kind, active", &rows);
        inserted += chunk;
    }
}

/// Yalnızca cari kayıtları üretir (1.000.000 cari senaryosu).
pub fn seed_customers_only(database: &Database, count: usize) -> Vec<i64> {
    let connection = raw_connection(database);
    bulk_pragmas(&connection);
    let transaction = connection
        .unchecked_transaction()
        .expect("cari işlemi başlatılmalı");
    insert_customers(&transaction, count);
    transaction.commit().expect("cari işlemi commit edilmeli");

    let mut statement = connection
        .prepare("SELECT id FROM customers ORDER BY id")
        .expect("cari kimlik sorgusu");
    let rows = statement
        .query_map([], |row| row.get::<_, i64>(0))
        .expect("cari kimlikleri okunmalı");
    rows.map(|row| row.expect("cari kimliği")).collect()
}

fn insert_products(
    connection: &rusqlite::Connection,
    count: usize,
    starting_stock: i64,
) -> Vec<i64> {
    const BATCH: usize = 1000;
    let mut inserted = 0;
    while inserted < count {
        let chunk = (count - inserted).min(BATCH);
        let rows: Vec<Vec<Value>> = (0..chunk)
            .map(|offset| {
                let index = inserted + offset + 1;
                vec![
                    Value::Text(format!("P{index:07}")),
                    Value::Text(format!("Ürün {index:07}")),
                    Value::Text("adet".to_owned()),
                    Value::Integer(10_000),
                    Value::Integer(12_000),
                    Value::Integer(20),
                    Value::Integer(starting_stock),
                ]
            })
            .collect();
        insert_batch(
            connection,
            "products",
            "code, name, unit, purchase_price, sale_price, vat_rate, current_stock",
            &rows,
        );
        inserted += chunk;
    }

    let mut statement = connection
        .prepare("SELECT id FROM products ORDER BY id")
        .expect("ürün kimlik sorgusu");
    let rows = statement
        .query_map([], |row| row.get::<_, i64>(0))
        .expect("ürün kimlikleri okunmalı");
    rows.map(|row| row.expect("ürün kimliği")).collect()
}

/// 100 ürün ve ürün başına `movements_per_product` stok hareketi üretir.
///
/// 10.000.000 satırlık senaryoda satır başına ayrı UPDATE yerine işlem
/// sonunda `current_stock = SUM(quantity_delta)` tek SQL ile kurulur. Bu,
/// testin üretim kancası eklemeden makul sürede çalışmasını sağlar; sonuç
/// `assert_correlations` ile her ürün için doğrulanır.
pub fn seed_stock_movements(
    database: &Database,
    product_count: usize,
    movements_per_product: usize,
) -> Vec<i64> {
    let connection = raw_connection(database);
    bulk_pragmas(&connection);
    let transaction = connection
        .unchecked_transaction()
        .expect("stok işlemi başlatılmalı");
    let product_ids = insert_products(&transaction, product_count, 0);

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
                        Value::Text("stress stok hareketi".to_owned()),
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

/// Cari, ürün, fatura, tahsilat/ödeme ve gelir/gider için deterministik veri üretir.
///
/// Tüm yazma tek bir transaction içindedir; hata hâlinde hiçbir kayıt kalmaz.
pub fn seed_bulk(
    database: &Database,
    customers: usize,
    products: usize,
    invoices: usize,
    payments: usize,
    entries: usize,
) -> BulkSeed {
    assert!(customers > 0, "en az bir cari gerekir");
    assert!(products > 0, "en az bir ürün gerekir");

    let connection = raw_connection(database);
    bulk_pragmas(&connection);
    let transaction = connection
        .unchecked_transaction()
        .expect("büyük veri işlemi başlatılmalı");

    insert_customers(&transaction, customers);
    let customer_ids: Vec<i64> = {
        let mut statement = transaction
            .prepare("SELECT id FROM customers ORDER BY id")
            .expect("cari kimlik sorgusu");
        let rows = statement
            .query_map([], |row| row.get::<_, i64>(0))
            .expect("cari kimlikleri");
        rows.map(|row| row.expect("cari kimliği")).collect()
    };
    let product_ids = insert_products(&transaction, products, 0);

    transaction
        .execute(
            "INSERT INTO cash_accounts (name, account_type, opening_balance, current_balance)
             VALUES ('STRESS KASA', 'kasa', 100000, 100000)",
            [],
        )
        .expect("kasa hesabı");
    let cash_account_id = transaction.last_insert_rowid();

    transaction
        .execute(
            "INSERT INTO expense_categories (name, category_type)
             VALUES ('Stress Gelir', 'gelir')",
            [],
        )
        .expect("gelir kategorisi");
    let income_category_id = transaction.last_insert_rowid();
    transaction
        .execute(
            "INSERT INTO expense_categories (name, category_type)
             VALUES ('Stress Gider', 'gider')",
            [],
        )
        .expect("gider kategorisi");
    let expense_category_id = transaction.last_insert_rowid();

    const QUANTITY: i64 = 1000;
    const UNIT_PRICE: i64 = 10_000;
    const SUBTOTAL: i64 = 10_000;
    const VAT: i64 = 2_000;
    const GRAND: i64 = 12_000;

    let mut invoice_seeds = Vec::with_capacity(invoices);
    for index in 0..invoices {
        let is_sales = index.is_multiple_of(2);
        let customer_id = customer_ids[index % customer_ids.len()];
        let product_id = product_ids[index % product_ids.len()];
        let date = format!("2026-{:02}-{:02}", (index % 12) + 1, (index % 28) + 1);
        let number = format!("{}-{index:07}", if is_sales { "SF" } else { "AF" });

        transaction
            .execute(
                "INSERT INTO invoices
                     (invoice_number, invoice_type, customer_id, invoice_date, due_date, notes,
                      subtotal, discount_total, net_total, vat_total, grand_total, status)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?7, ?8, ?9, 'kesin')",
                rusqlite::params![
                    number,
                    if is_sales { "satis" } else { "alis" },
                    customer_id,
                    date,
                    Option::<String>::None,
                    "stress faturası",
                    SUBTOTAL,
                    VAT,
                    GRAND,
                ],
            )
            .expect("fatura eklenmeli");
        let invoice_id = transaction.last_insert_rowid();

        transaction
            .execute(
                "INSERT INTO invoice_lines
                     (invoice_id, product_id, line_no, product_code, product_name, unit,
                      description, quantity, unit_price, discount_rate, vat_rate, line_gross,
                      line_discount, line_net, line_vat, line_total)
                 VALUES (?1, ?2, 1, ?3, ?4, 'adet', NULL, ?5, ?6, 0, 20,
                         ?7, 0, ?7, ?8, ?9)",
                rusqlite::params![
                    invoice_id,
                    product_id,
                    format!("P{product_id:07}"),
                    format!("Ürün {product_id:07}"),
                    QUANTITY,
                    UNIT_PRICE,
                    SUBTOTAL,
                    VAT,
                    GRAND,
                ],
            )
            .expect("fatura satırı eklenmeli");

        let delta = if is_sales { -QUANTITY } else { QUANTITY };
        transaction
            .execute(
                "INSERT INTO product_stock_movements
                     (product_id, movement_type, quantity_delta, movement_date, description,
                      invoice_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    product_id,
                    if is_sales {
                        "satis_faturasi"
                    } else {
                        "alis_faturasi"
                    },
                    delta,
                    date,
                    format!("stress faturası: {number}"),
                    invoice_id,
                ],
            )
            .expect("stok hareketi eklenmeli");
        transaction
            .execute(
                "UPDATE products SET current_stock = current_stock + ?2 WHERE id = ?1",
                rusqlite::params![product_id, delta],
            )
            .expect("stok güncellenmeli");

        invoice_seeds.push(InvoiceSeed {
            id: invoice_id,
            is_sales,
            customer_id,
            grand_total: GRAND,
            paid_amount: 0,
            date,
        });
    }

    for index in 0..payments {
        if invoice_seeds.is_empty() {
            break;
        }
        let invoice_index = index % invoice_seeds.len();
        let (invoice_id, is_sales, customer_id, date, remaining) = {
            let invoice = &invoice_seeds[invoice_index];
            (
                invoice.id,
                invoice.is_sales,
                invoice.customer_id,
                invoice.date.clone(),
                invoice.grand_total - invoice.paid_amount,
            )
        };
        let allocation = remaining.clamp(1, 5_000);
        let payment_amount = allocation;

        transaction
            .execute(
                "INSERT INTO payments
                     (payment_type, customer_id, cash_account_id, payment_date, amount,
                      description, status)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'stress ödemesi', 'kesin')",
                rusqlite::params![
                    if is_sales { "tahsilat" } else { "odeme" },
                    customer_id,
                    cash_account_id,
                    date,
                    payment_amount,
                ],
            )
            .expect("ödeme eklenmeli");
        let payment_id = transaction.last_insert_rowid();

        transaction
            .execute(
                "INSERT INTO payment_allocations (payment_id, invoice_id, amount)
                 VALUES (?1, ?2, ?3)",
                rusqlite::params![payment_id, invoice_id, allocation],
            )
            .expect("dağıtım eklenmeli");
        transaction
            .execute(
                "UPDATE invoices SET paid_amount = paid_amount + ?2 WHERE id = ?1",
                rusqlite::params![invoice_id, allocation],
            )
            .expect("fatura ödenen tutarı güncellenmeli");
        let delta = if is_sales {
            payment_amount
        } else {
            -payment_amount
        };
        transaction
            .execute(
                "UPDATE cash_accounts SET current_balance = current_balance + ?2 WHERE id = ?1",
                rusqlite::params![cash_account_id, delta],
            )
            .expect("kasa bakiyesi güncellenmeli");
        invoice_seeds[invoice_index].paid_amount += allocation;
    }

    for index in 0..entries {
        let is_income = index.is_multiple_of(2);
        let amount = if is_income { 1_000 } else { 500 };
        let category_id = if is_income {
            income_category_id
        } else {
            expense_category_id
        };
        let date = format!("2026-{:02}-{:02}", (index % 12) + 1, (index % 28) + 1);
        transaction
            .execute(
                "INSERT INTO income_expense_entries
                     (entry_type, category_id, cash_account_id, entry_date, amount,
                      description, status)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'stress gelir/gider', 'kesin')",
                rusqlite::params![
                    if is_income { "gelir" } else { "gider" },
                    category_id,
                    cash_account_id,
                    date,
                    amount,
                ],
            )
            .expect("gelir/gider eklenmeli");
        let delta = if is_income { amount } else { -amount };
        transaction
            .execute(
                "UPDATE cash_accounts SET current_balance = current_balance + ?2 WHERE id = ?1",
                rusqlite::params![cash_account_id, delta],
            )
            .expect("kasa bakiyesi güncellenmeli");
    }

    // İptal edilmiş kayıtların korelasyonlara katılmadığını sınamak için birer örnek.
    let cancelled_customer = customer_ids[0];
    let cancelled_product = product_ids[0];
    let cancelled_date = "2026-12-31".to_owned();
    transaction
        .execute(
            "INSERT INTO invoices
                 (invoice_number, invoice_type, customer_id, invoice_date, subtotal,
                  discount_total, net_total, vat_total, grand_total, status,
                  cancelled_at, cancel_reason)
             VALUES ('IPT-0000001', 'satis', ?1, ?2, 10000, 0, 10000, 2000, 12000, 'iptal',
                     datetime('now'), 'stress iptali')",
            rusqlite::params![cancelled_customer, cancelled_date],
        )
        .expect("iptal faturası eklenmeli");
    let cancelled_invoice_id = transaction.last_insert_rowid();
    transaction
        .execute(
            "INSERT INTO invoice_lines
                 (invoice_id, product_id, line_no, product_code, product_name, unit,
                  quantity, unit_price, discount_rate, vat_rate, line_gross,
                  line_discount, line_net, line_vat, line_total)
             VALUES (?1, ?2, 1, ?3, ?4, 'adet', 1000, 10000, 0, 20,
                     10000, 0, 10000, 2000, 12000)",
            rusqlite::params![
                cancelled_invoice_id,
                cancelled_product,
                format!("P{cancelled_product:07}"),
                format!("Ürün {cancelled_product:07}"),
            ],
        )
        .expect("iptal fatura satırı");
    transaction
        .execute(
            "INSERT INTO product_stock_movements
                 (product_id, movement_type, quantity_delta, movement_date, description, invoice_id)
             VALUES (?1, 'satis_faturasi', -1000, ?2, 'iptal öncesi', ?3)",
            rusqlite::params![cancelled_product, cancelled_date, cancelled_invoice_id],
        )
        .expect("iptal öncesi stok");
    transaction
        .execute(
            "INSERT INTO product_stock_movements
                 (product_id, movement_type, quantity_delta, movement_date, description, invoice_id)
             VALUES (?1, 'duzeltme', 1000, ?2, 'iptal düzeltmesi', ?3)",
            rusqlite::params![cancelled_product, cancelled_date, cancelled_invoice_id],
        )
        .expect("iptal düzeltme stoğu");

    let mut cancelled_payment_created = false;
    if let Some(invoice) = invoice_seeds
        .iter()
        .find(|invoice| invoice.grand_total - invoice.paid_amount >= 1_000)
    {
        let allocation = 1_000;
        transaction
            .execute(
                "INSERT INTO payments
                     (payment_type, customer_id, cash_account_id, payment_date, amount,
                      description, status)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'iptal edilecek ödeme', 'kesin')",
                rusqlite::params![
                    if invoice.is_sales {
                        "tahsilat"
                    } else {
                        "odeme"
                    },
                    invoice.customer_id,
                    cash_account_id,
                    invoice.date,
                    allocation,
                ],
            )
            .expect("iptal edilecek ödeme");
        let payment_id = transaction.last_insert_rowid();
        transaction
            .execute(
                "INSERT INTO payment_allocations (payment_id, invoice_id, amount)
                 VALUES (?1, ?2, ?3)",
                rusqlite::params![payment_id, invoice.id, allocation],
            )
            .expect("iptal ödeme dağıtımı");
        transaction
            .execute(
                "UPDATE invoices SET paid_amount = paid_amount + ?2 WHERE id = ?1",
                rusqlite::params![invoice.id, allocation],
            )
            .expect("iptal ödeme fatura tutarı");
        let cash_delta = if invoice.is_sales {
            allocation
        } else {
            -allocation
        };
        transaction
            .execute(
                "UPDATE cash_accounts SET current_balance = current_balance + ?2 WHERE id = ?1",
                rusqlite::params![cash_account_id, cash_delta],
            )
            .expect("iptal ödeme kasa");
        transaction
            .execute(
                "UPDATE invoices SET paid_amount = paid_amount - ?2 WHERE id = ?1",
                rusqlite::params![invoice.id, allocation],
            )
            .expect("iptal ödeme geri alınmalı");
        transaction
            .execute(
                "UPDATE cash_accounts SET current_balance = current_balance - ?2 WHERE id = ?1",
                rusqlite::params![cash_account_id, cash_delta],
            )
            .expect("iptal ödeme kasa geri alınmalı");
        transaction
            .execute(
                "UPDATE payments SET status = 'iptal', cancelled_at = datetime('now'),
                     cancel_reason = 'stress iptali' WHERE id = ?1",
                rusqlite::params![payment_id],
            )
            .expect("ödeme iptal edilmeli");
        cancelled_payment_created = true;
    }

    transaction
        .execute(
            "INSERT INTO income_expense_entries
                 (entry_type, category_id, cash_account_id, entry_date, amount,
                  description, status, cancelled_at, cancel_reason)
             VALUES ('gelir', ?1, ?2, '2026-12-30', 1000, 'iptal gelir', 'iptal',
                     datetime('now'), 'stress iptali')",
            rusqlite::params![income_category_id, cash_account_id],
        )
        .expect("iptal gelir kaydı");

    transaction.commit().expect("büyük veri commit edilmeli");

    BulkSeed {
        customer_id: customer_ids[0],
        product_id: product_ids[0],
        cash_account_id,
        invoice_count: invoices + 1,
        payment_count: payments + usize::from(cancelled_payment_created),
        entry_count: entries + 1,
    }
}
