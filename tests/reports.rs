// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Raporlar aşamasının uçtan uca (gerçek SQLite) testleri.
//!
//! Testler yalnızca rapor okuma yollarını kullanır; her rapor çağrısından sonra
//! cari/kasa bakiyelerinin değişmediğini doğrulayarak raporların salt okunur
//! olduğunu da denetler.

use eframe::egui::Context;
use on_muhasebe::app::{OnMuhasebeApp, View};
use on_muhasebe::cash::{AccountType, ValidatedCashAccount};
use on_muhasebe::config::AppConfig;
use on_muhasebe::customer::{CustomerType, ValidatedCustomer};
use on_muhasebe::db::Database;
use on_muhasebe::income_expense::{EntryFilter, EntryType, ValidatedCategory, ValidatedEntry};
use on_muhasebe::invoice::{InvoiceType, ValidatedInvoice, ValidatedInvoiceLine, calculate_line};
use on_muhasebe::money::Money;
use on_muhasebe::payment::{PaymentType, ValidatedPayment};
use on_muhasebe::product::{MovementType, Unit, ValidatedMovement, ValidatedProduct};
use on_muhasebe::quantity::Quantity;
use on_muhasebe::report::{
    CustomerBalanceReportFilter, CustomerBalanceSortColumn, ReportKind, SalesPurchaseBreakdown,
    SalesPurchaseGrouping, StockReportFilter, StockStatus, TREND_MONTHS, percentage_label,
};

fn veritabani() -> Database {
    Database::open_in_memory().expect("bellek içi veritabanı açılmalı")
}

fn cari(database: &Database, code: &str, name: &str, kind: CustomerType, active: bool) -> i64 {
    database
        .customer_insert(&ValidatedCustomer {
            code: code.to_owned(),
            name: name.to_owned(),
            kind,
            phone: None,
            email: None,
            address: None,
            tax_office: None,
            tax_number: None,
            national_id: None,
            contact_person: None,
            notes: None,
            active,
        })
        .expect("cari eklenmeli")
}

#[allow(clippy::too_many_arguments)]
fn urun(
    database: &Database,
    code: &str,
    name: &str,
    starting_stock: i64,
    min_stock: i64,
    purchase_price: i64,
    sale_price: i64,
    active: bool,
) -> i64 {
    database
        .product_insert(&ValidatedProduct {
            code: code.to_owned(),
            barcode: None,
            name: name.to_owned(),
            unit: Unit::Adet,
            category: None,
            purchase_price: Money::from_kurus(purchase_price),
            sale_price: Money::from_kurus(sale_price),
            vat_rate: 0,
            min_stock: Quantity::from_milli(min_stock),
            notes: None,
            active,
            starting_stock: Quantity::from_milli(starting_stock),
        })
        .expect("ürün eklenmeli")
}

fn kasa(database: &Database, name: &str, opening: i64, kind: AccountType) -> i64 {
    database
        .cash_account_insert(&ValidatedCashAccount {
            name: name.to_owned(),
            account_type: kind,
            bank_name: None,
            iban: None,
            opening_balance: Money::from_kurus(opening),
            notes: None,
            active: true,
        })
        .expect("kasa hesabı eklenmeli")
}

#[allow(clippy::too_many_arguments)]
fn fatura(
    database: &Database,
    kind: InvoiceType,
    number: &str,
    customer_id: i64,
    product_id: i64,
    date: &str,
    quantity: i64,
    unit_price: i64,
) -> i64 {
    let product = database
        .product_get(product_id)
        .expect("ürün okunmalı")
        .expect("ürün bulunmalı");
    let quantity = Quantity::from_milli(quantity);
    let amounts =
        calculate_line(quantity, Money::from_kurus(unit_price), 0, 0).expect("satır hesaplanmalı");
    let lines = vec![ValidatedInvoiceLine {
        product_id,
        product_code: product.code,
        product_name: product.name,
        unit: product.unit,
        description: None,
        quantity,
        unit_price: Money::from_kurus(unit_price),
        discount_rate: 0,
        vat_rate: 0,
        amounts,
    }];
    let totals =
        on_muhasebe::invoice::InvoiceTotals::from_lines(&lines).expect("toplamlar hesaplanmalı");
    database
        .invoice_insert(&ValidatedInvoice {
            number: number.to_owned(),
            invoice_type: kind,
            customer_id,
            invoice_date: date.to_owned(),
            due_date: None,
            notes: None,
            lines,
            totals,
        })
        .expect("fatura eklenmeli")
}

fn odeme(
    database: &Database,
    kind: PaymentType,
    customer_id: i64,
    account_id: i64,
    date: &str,
    amount: i64,
) -> i64 {
    database
        .payment_insert(&ValidatedPayment {
            payment_type: kind,
            customer_id,
            cash_account_id: account_id,
            payment_date: date.to_owned(),
            amount: Money::from_kurus(amount),
            description: None,
            allocations: Vec::new(),
        })
        .expect("tahsilat/ödeme eklenmeli")
}

fn kategori(database: &Database, name: &str, kind: EntryType) -> i64 {
    database
        .expense_category_insert(&ValidatedCategory {
            name: name.to_owned(),
            category_type: kind,
            description: None,
            active: true,
        })
        .expect("kategori eklenmeli")
}

fn gelir_gider(
    database: &Database,
    kind: EntryType,
    category_id: i64,
    account_id: i64,
    date: &str,
    amount: i64,
) -> i64 {
    database
        .income_expense_insert(&ValidatedEntry {
            entry_type: kind,
            category_id,
            cash_account_id: account_id,
            entry_date: date.to_owned(),
            amount: Money::from_kurus(amount),
            description: None,
            document_ref: None,
        })
        .expect("gelir/gider eklenmeli")
}

fn gecici_config() -> (tempfile::TempDir, AppConfig) {
    let directory = tempfile::tempdir().expect("geçici dizin oluşturulmalı");
    let config = AppConfig::with_data_dir(directory.path().join("veri"));
    (directory, config)
}

#[test]
fn cari_ekstre_donem_basi_kapanis_ve_tam_bakiye_korelasyonu() {
    let database = veritabani();
    let musteri = cari(
        &database,
        "CAR-0001",
        "Öztürk Ticaret",
        CustomerType::Customer,
        true,
    );
    let urun_id = urun(&database, "URN-1", "Kalem", 100_000, 0, 5_000, 10_000, true);
    let hesap = kasa(&database, "Merkez Kasa", 0, AccountType::Kasa);

    fatura(
        &database,
        InvoiceType::Sales,
        "SAT-1",
        musteri,
        urun_id,
        "2026-09-15",
        2_000,
        10_000,
    );
    fatura(
        &database,
        InvoiceType::Purchase,
        "ALI-1",
        musteri,
        urun_id,
        "2026-09-20",
        1_000,
        5_000,
    );
    odeme(
        &database,
        PaymentType::Tahsilat,
        musteri,
        hesap,
        "2026-10-05",
        3_000,
    );
    odeme(
        &database,
        PaymentType::Odeme,
        musteri,
        hesap,
        "2026-10-10",
        1_000,
    );
    let iptal_fatura = fatura(
        &database,
        InvoiceType::Sales,
        "SAT-IPTAL",
        musteri,
        urun_id,
        "2026-10-15",
        5_000,
        10_000,
    );
    database
        .invoice_cancel(iptal_fatura, "test iptali")
        .expect("fatura iptal edilmeli");
    let iptal_odeme = odeme(
        &database,
        PaymentType::Tahsilat,
        musteri,
        hesap,
        "2026-10-20",
        9_999,
    );
    database
        .payment_cancel(iptal_odeme, "test iptali")
        .expect("ödeme iptal edilmeli");
    fatura(
        &database,
        InvoiceType::Sales,
        "SAT-2",
        musteri,
        urun_id,
        "2026-11-01",
        1_000,
        10_000,
    );

    let ekstre = database
        .customer_statement(musteri, "2026-10-01", "2026-10-31")
        .expect("ekstre okunmalı")
        .expect("cari bulunmalı");
    assert_eq!(ekstre.opening_balance.kurus(), 15_000);
    assert_eq!(ekstre.rows.len(), 2);
    assert_eq!(ekstre.total_debit.kurus(), 1_000);
    assert_eq!(ekstre.total_credit.kurus(), 3_000);
    assert_eq!(ekstre.closing_balance.kurus(), 13_000);
    assert_eq!(
        ekstre.rows[0].kind,
        on_muhasebe::report::LedgerKind::Collection
    );
    assert_eq!(ekstre.rows[0].balance.kurus(), 12_000);
    assert_eq!(ekstre.rows[1].balance.kurus(), 13_000);

    let tam = database
        .customer_statement(musteri, "", "")
        .expect("tam ekstre okunmalı")
        .expect("cari bulunmalı");
    assert_eq!(tam.rows.len(), 5, "iptaller ekstreye girmemeli");
    assert_eq!(tam.opening_balance, Money::ZERO);
    let cari_kaydi = database
        .customer_get(musteri)
        .expect("cari okunmalı")
        .expect("cari bulunmalı");
    assert_eq!(
        tam.closing_balance,
        cari_kaydi.balance(),
        "tam ekstre kapanışı cari bakiyesiyle aynı olmalı"
    );
    assert_eq!(tam.closing_balance.kurus(), 23_000);
}

#[test]
fn cari_bakiye_ozeti_arama_filtre_siralama_ve_korelasyon() {
    let database = veritabani();
    let urun_id = urun(&database, "URN-1", "Kalem", 100_000, 0, 5_000, 10_000, true);

    let ozturk = cari(
        &database,
        "CAR-0001",
        "Öztürk Ticaret",
        CustomerType::Customer,
        true,
    );
    let beta = cari(
        &database,
        "CAR-0002",
        "Beta Tedarik",
        CustomerType::Supplier,
        true,
    );
    let pasif = cari(
        &database,
        "CAR-0003",
        "Pasif Müşteri",
        CustomerType::Customer,
        false,
    );

    fatura(
        &database,
        InvoiceType::Sales,
        "SAT-1",
        ozturk,
        urun_id,
        "2026-09-10",
        1_000,
        10_000,
    );
    fatura(
        &database,
        InvoiceType::Sales,
        "SAT-2",
        ozturk,
        urun_id,
        "2026-10-10",
        1_000,
        4_000,
    );
    fatura(
        &database,
        InvoiceType::Purchase,
        "ALI-1",
        beta,
        urun_id,
        "2026-10-11",
        1_000,
        5_000,
    );
    fatura(
        &database,
        InvoiceType::Sales,
        "SAT-3",
        pasif,
        urun_id,
        "2026-10-12",
        1_000,
        7_000,
    );

    let filtre = CustomerBalanceReportFilter {
        search: "ozturk".to_owned(),
        kind: Some(CustomerType::Customer),
        active: Some(true),
        only_nonzero: true,
    };
    let rapor = database
        .customer_balance_report("", "", &filtre)
        .expect("bakiye özeti okunmalı");
    assert_eq!(rapor.rows.len(), 1);
    assert_eq!(rapor.rows[0].customer_id, ozturk);
    assert_eq!(rapor.rows[0].balance.kurus(), 14_000);
    assert_eq!(rapor.total_receivable.kurus(), 14_000);

    let tumu = database
        .customer_balance_report("", "", &CustomerBalanceReportFilter::default())
        .expect("bakiye özeti okunmalı");
    assert_eq!(tumu.rows.len(), 3);
    assert_eq!(tumu.rows[0].customer_id, ozturk, "mutlak bakiye azalan");
    assert_eq!(tumu.total_payable.kurus(), 5_000);
    assert_eq!(tumu.net_balance.kurus(), 16_000);
    for row in &tumu.rows {
        let cari_kaydi = database
            .customer_get(row.customer_id)
            .expect("cari okunmalı")
            .expect("cari bulunmalı");
        assert_eq!(row.balance, cari_kaydi.balance(), "bakiye korelasyonu");
    }

    let mut siralanmis = tumu.clone();
    siralanmis.sort_by(CustomerBalanceSortColumn::Code, true);
    assert_eq!(siralanmis.rows[0].code, "CAR-0001");
    siralanmis.sort_by(CustomerBalanceSortColumn::Code, false);
    assert_eq!(siralanmis.rows[0].code, "CAR-0003");

    let donem = database
        .customer_balance_report(
            "2026-10-01",
            "2026-10-31",
            &CustomerBalanceReportFilter::default(),
        )
        .expect("dönem bakiyesi okunmalı");
    let ozturk_satiri = donem
        .rows
        .iter()
        .find(|row| row.customer_id == ozturk)
        .expect("cari dönemde bulunmalı");
    assert_eq!(ozturk_satiri.sales_total.kurus(), 4_000);
    assert_eq!(ozturk_satiri.balance.kurus(), 4_000);
}

#[test]
fn stok_durumu_filtreleri_ve_alt_bilgi_sayilari() {
    let database = veritabani();
    let yeterli = urun(
        &database,
        "URN-1",
        "Çelik Vida",
        10_000,
        5_000,
        1_000,
        1_500,
        true,
    );
    let dusuk = urun(
        &database,
        "URN-2",
        "Düşük Stok",
        2_000,
        5_000,
        1_000,
        1_500,
        true,
    );
    let sifir = urun(&database, "URN-3", "Sıfır Stok", 0, 0, 1_000, 1_500, true);
    let eksi = urun(&database, "URN-4", "Eksi Stok", 0, 0, 1_000, 1_500, true);
    let pasif = urun(
        &database,
        "URN-5",
        "Pasif Ürün",
        10_000,
        0,
        1_000,
        1_500,
        false,
    );
    database
        .movement_insert(
            eksi,
            &ValidatedMovement {
                movement_type: MovementType::Out,
                quantity_delta: Quantity::from_milli(-3_000),
                movement_date: "2026-10-01".to_owned(),
                description: Some("test çıkışı".to_owned()),
            },
        )
        .expect("stok çıkışı eklenmeli");

    let varsayilan = database
        .stock_status_report(&StockReportFilter::default())
        .expect("stok raporu okunmalı");
    assert_eq!(varsayilan.rows.len(), 4, "pasif ürün varsayılan gizli");
    assert_eq!(varsayilan.stats.active_products, 4);
    assert_eq!(varsayilan.stats.low_stock, 1);
    assert_eq!(varsayilan.stats.zero_stock, 1);
    assert_eq!(varsayilan.stats.negative_stock, 1);
    assert_eq!(varsayilan.stats.purchase_value.kurus(), 9_000);
    assert_eq!(varsayilan.stats.sale_value.kurus(), 13_500);
    assert_eq!(
        varsayilan
            .rows
            .iter()
            .find(|row| row.product_id == yeterli)
            .expect("ürün bulunmalı")
            .status,
        StockStatus::Yeterli
    );
    assert_eq!(
        varsayilan
            .rows
            .iter()
            .find(|row| row.product_id == dusuk)
            .expect("ürün bulunmalı")
            .status,
        StockStatus::Dusuk
    );
    assert_eq!(
        varsayilan
            .rows
            .iter()
            .find(|row| row.product_id == sifir)
            .expect("ürün bulunmalı")
            .status,
        StockStatus::Sifir
    );
    assert_eq!(
        varsayilan
            .rows
            .iter()
            .find(|row| row.product_id == eksi)
            .expect("ürün bulunmalı")
            .status,
        StockStatus::Eksi
    );

    let arama = StockReportFilter {
        search: "celik".to_owned(),
        status: None,
        only_active: true,
    };
    let arama_raporu = database
        .stock_status_report(&arama)
        .expect("stok raporu okunmalı");
    assert_eq!(arama_raporu.rows.len(), 1);
    assert_eq!(arama_raporu.rows[0].product_id, yeterli);
    assert_eq!(
        arama_raporu.stats.active_products, 4,
        "alt bilgi aramadan bağımsız"
    );

    let durum = StockReportFilter {
        search: String::new(),
        status: Some(StockStatus::Dusuk),
        only_active: true,
    };
    let durum_raporu = database
        .stock_status_report(&durum)
        .expect("stok raporu okunmalı");
    assert_eq!(durum_raporu.rows.len(), 1);
    assert_eq!(durum_raporu.rows[0].product_id, dusuk);

    let tum_urunler = StockReportFilter {
        search: String::new(),
        status: None,
        only_active: false,
    };
    let tum_rapor = database
        .stock_status_report(&tum_urunler)
        .expect("stok raporu okunmalı");
    assert_eq!(tum_rapor.rows.len(), 5);
    assert!(tum_rapor.rows.iter().any(|row| row.product_id == pasif));
}

#[test]
fn satis_alis_ozeti_gruplama_ve_iptal_haricligi() {
    let database = veritabani();
    let alfa = cari(&database, "CAR-1", "Alfa", CustomerType::Customer, true);
    let beta = cari(&database, "CAR-2", "Beta", CustomerType::Customer, true);
    let urun1 = urun(&database, "URN-1", "Ürün Bir", 100_000, 0, 500, 1_000, true);
    let urun2 = urun(&database, "URN-2", "Ürün İki", 100_000, 0, 500, 3_000, true);

    fatura(
        &database,
        InvoiceType::Sales,
        "SAT-1",
        alfa,
        urun1,
        "2026-10-02",
        2_000,
        1_000,
    );
    fatura(
        &database,
        InvoiceType::Sales,
        "SAT-2",
        alfa,
        urun2,
        "2026-10-03",
        1_000,
        3_000,
    );
    fatura(
        &database,
        InvoiceType::Purchase,
        "ALI-1",
        beta,
        urun1,
        "2026-10-04",
        4_000,
        500,
    );
    fatura(
        &database,
        InvoiceType::Sales,
        "SAT-3",
        beta,
        urun2,
        "2026-10-05",
        3_000,
        1_000,
    );
    let iptal = fatura(
        &database,
        InvoiceType::Sales,
        "SAT-IPTAL",
        alfa,
        urun1,
        "2026-10-06",
        9_000,
        1_000,
    );
    database
        .invoice_cancel(iptal, "test iptali")
        .expect("fatura iptal edilmeli");

    let cari_raporu = database
        .sales_purchase_report("2026-10-01", "2026-10-31", SalesPurchaseGrouping::Customer)
        .expect("satış/alış raporu okunmalı");
    assert_eq!(cari_raporu.sales_count, 3);
    assert_eq!(cari_raporu.purchase_count, 1);
    assert_eq!(cari_raporu.invoice_count(), 4);
    assert_eq!(cari_raporu.sales_total.kurus(), 8_000);
    assert_eq!(cari_raporu.purchase_total.kurus(), 2_000);
    assert_eq!(cari_raporu.net().kurus(), 6_000);
    let SalesPurchaseBreakdown::Customer(rows) = &cari_raporu.breakdown else {
        panic!("cari dökümü bekleniyordu");
    };
    assert_eq!(rows.len(), 2);
    let alfa_satiri = rows
        .iter()
        .find(|row| row.customer_id == alfa)
        .expect("Alfa");
    assert_eq!(alfa_satiri.sales_count, 2);
    assert_eq!(alfa_satiri.sales_total.kurus(), 5_000);

    let urun_raporu = database
        .sales_purchase_report("2026-10-01", "2026-10-31", SalesPurchaseGrouping::Product)
        .expect("ürün dökümü okunmalı");
    let SalesPurchaseBreakdown::Product(rows) = &urun_raporu.breakdown else {
        panic!("ürün dökümü bekleniyordu");
    };
    let bir = rows
        .iter()
        .find(|row| row.product_id == urun1)
        .expect("Ürün 1");
    assert_eq!(bir.sold_quantity, Quantity::from_milli(2_000));
    assert_eq!(bir.purchased_quantity, Quantity::from_milli(4_000));
    assert_eq!(bir.sales_total.kurus(), 2_000);
    assert_eq!(bir.purchase_total.kurus(), 2_000);
    let iki = rows
        .iter()
        .find(|row| row.product_id == urun2)
        .expect("Ürün 2");
    assert_eq!(iki.sold_quantity, Quantity::from_milli(4_000));
    assert_eq!(iki.sales_total.kurus(), 6_000);

    let dar = database
        .sales_purchase_report("2026-10-04", "2026-10-05", SalesPurchaseGrouping::Customer)
        .expect("dar dönem raporu okunmalı");
    assert_eq!(dar.sales_count, 1);
    assert_eq!(dar.sales_total.kurus(), 3_000);
    assert_eq!(dar.purchase_count, 1);
    assert_eq!(dar.purchase_total.kurus(), 2_000);

    let bos = database
        .sales_purchase_report("2025-01-01", "2025-01-31", SalesPurchaseGrouping::Customer)
        .expect("boş dönem okunmalı");
    assert_eq!(bos.invoice_count(), 0);
    assert!(bos.breakdown.is_empty());
}

#[test]
fn kasa_banka_hareket_ozeti_korelasyon_ve_iptal_haricligi() {
    let database = veritabani();
    let musteri = cari(&database, "CAR-1", "Alfa", CustomerType::Customer, true);
    let kasa_id = kasa(&database, "Merkez Kasa", 100_000, AccountType::Kasa);
    let banka_id = kasa(&database, "Ziraat", 50_000, AccountType::Banka);

    odeme(
        &database,
        PaymentType::Tahsilat,
        musteri,
        kasa_id,
        "2026-09-20",
        20_000,
    );
    odeme(
        &database,
        PaymentType::Odeme,
        musteri,
        banka_id,
        "2026-09-21",
        5_000,
    );
    odeme(
        &database,
        PaymentType::Tahsilat,
        musteri,
        kasa_id,
        "2026-10-05",
        30_000,
    );
    odeme(
        &database,
        PaymentType::Odeme,
        musteri,
        banka_id,
        "2026-10-06",
        10_000,
    );
    let gelir_kategori = kategori(&database, "Satış Geliri", EntryType::Gelir);
    let gider_kategori = kategori(&database, "Kira", EntryType::Gider);
    gelir_gider(
        &database,
        EntryType::Gelir,
        gelir_kategori,
        kasa_id,
        "2026-10-07",
        40_000,
    );
    gelir_gider(
        &database,
        EntryType::Gider,
        gider_kategori,
        banka_id,
        "2026-10-08",
        15_000,
    );
    let iptal_odeme = odeme(
        &database,
        PaymentType::Tahsilat,
        musteri,
        kasa_id,
        "2026-10-09",
        99_999,
    );
    database
        .payment_cancel(iptal_odeme, "test iptali")
        .expect("ödeme iptal edilmeli");
    let iptal_gelir = gelir_gider(
        &database,
        EntryType::Gelir,
        gelir_kategori,
        kasa_id,
        "2026-10-10",
        88_888,
    );
    database
        .income_expense_cancel(iptal_gelir, "test iptali")
        .expect("gelir iptal edilmeli");

    let rapor = database
        .cash_movement_report("2026-10-01", "2026-10-31", None)
        .expect("kasa raporu okunmalı")
        .expect("rapor olmalı");
    assert_eq!(rapor.opening_balance.kurus(), 165_000);
    assert_eq!(rapor.collections.kurus(), 30_000);
    assert_eq!(rapor.payments.kurus(), 10_000);
    assert_eq!(rapor.incomes.kurus(), 40_000);
    assert_eq!(rapor.expenses.kurus(), 15_000);
    assert_eq!(rapor.closing_balance.kurus(), 210_000);
    assert_eq!(rapor.accounts.len(), 2);
    assert_eq!(rapor.movements.len(), 4, "iptaller hareketlere girmemeli");
    assert!(rapor.has_movements());

    for satir in &rapor.accounts {
        let hesap = database
            .cash_account_get(satir.account_id)
            .expect("hesap okunmalı")
            .expect("hesap bulunmalı");
        assert_eq!(
            satir.closing_balance, hesap.current_balance,
            "hesap kapanışı güncel bakiye ile aynı olmalı"
        );
    }
    let kasa_satiri = rapor
        .accounts
        .iter()
        .find(|row| row.account_id == kasa_id)
        .expect("kasa bulunmalı");
    assert_eq!(kasa_satiri.opening_balance.kurus(), 120_000);
    assert_eq!(kasa_satiri.closing_balance.kurus(), 190_000);
    let banka_satiri = rapor
        .accounts
        .iter()
        .find(|row| row.account_id == banka_id)
        .expect("banka bulunmalı");
    assert_eq!(banka_satiri.opening_balance.kurus(), 45_000);
    assert_eq!(banka_satiri.closing_balance.kurus(), 20_000);

    let tek = database
        .cash_movement_report("2026-10-01", "2026-10-31", Some(kasa_id))
        .expect("kasa raporu okunmalı")
        .expect("rapor olmalı");
    assert_eq!(tek.accounts.len(), 1);
    assert_eq!(tek.closing_balance.kurus(), 190_000);
    assert_eq!(tek.movements.len(), 2);
    assert!(
        tek.movements
            .iter()
            .all(|movement| movement.account_id == kasa_id)
    );
}

#[test]
fn gelir_gider_ozeti_yuzde_trend_ve_iptal_haricligi() {
    let database = veritabani();
    let hesap = kasa(&database, "Merkez Kasa", 0, AccountType::Kasa);
    let satis = kategori(&database, "Satış Geliri", EntryType::Gelir);
    let kira = kategori(&database, "Kira", EntryType::Gider);
    let market = kategori(&database, "Market", EntryType::Gider);

    gelir_gider(
        &database,
        EntryType::Gelir,
        satis,
        hesap,
        "2026-10-05",
        10_000,
    );
    gelir_gider(
        &database,
        EntryType::Gider,
        kira,
        hesap,
        "2026-10-06",
        4_000,
    );
    gelir_gider(
        &database,
        EntryType::Gider,
        market,
        hesap,
        "2026-10-07",
        1_000,
    );
    let iptal = gelir_gider(
        &database,
        EntryType::Gelir,
        satis,
        hesap,
        "2026-10-08",
        5_000,
    );
    database
        .income_expense_cancel(iptal, "test iptali")
        .expect("gelir iptal edilmeli");
    gelir_gider(
        &database,
        EntryType::Gelir,
        satis,
        hesap,
        "2026-06-01",
        2_000,
    );
    gelir_gider(&database, EntryType::Gider, kira, hesap, "2026-08-01", 500);
    gelir_gider(
        &database,
        EntryType::Gelir,
        satis,
        hesap,
        "2026-09-01",
        1_000,
    );

    let rapor = database
        .income_expense_report("2026-10-01", "2026-10-31")
        .expect("gelir/gider raporu okunmalı");
    assert_eq!(rapor.summary.income_total.kurus(), 10_000);
    assert_eq!(rapor.summary.expense_total.kurus(), 5_000);
    assert_eq!(rapor.summary.net().kurus(), 5_000);
    assert_eq!(rapor.summary.record_count, 4, "iptal kayıt sayısına dahil");
    assert_eq!(rapor.summary.confirmed_count, 3);
    assert_eq!(rapor.summary.categories.len(), 3);

    let kira_satiri = rapor
        .summary
        .categories
        .iter()
        .find(|row| row.category_name == "Kira")
        .expect("Kira kategorisi");
    assert_eq!(kira_satiri.percentage, Some(80));
    assert_eq!(percentage_label(kira_satiri.percentage), "%80");
    let market_satiri = rapor
        .summary
        .categories
        .iter()
        .find(|row| row.category_name == "Market")
        .expect("Market kategorisi");
    assert_eq!(market_satiri.percentage, Some(20));

    assert_eq!(rapor.trend.len(), TREND_MONTHS);
    assert_eq!(rapor.trend[0].label(), "Mayıs 2026");
    assert_eq!(rapor.trend[5].label(), "Ekim 2026");
    assert_eq!(rapor.trend[1].income_total.kurus(), 2_000, "Haziran");
    assert_eq!(rapor.trend[3].expense_total.kurus(), 500, "Ağustos");
    assert_eq!(rapor.trend[5].income_total.kurus(), 10_000);
    assert_eq!(rapor.trend[5].expense_total.kurus(), 5_000);
    assert!(
        rapor
            .trend
            .iter()
            .all(|month| month.income_total.kurus() != 5_000),
        "iptal gelir trende girmemeli"
    );

    let bos_taban = Money::ZERO.percentage_of(Money::ZERO);
    assert_eq!(bos_taban, None);
    assert_eq!(percentage_label(bos_taban), "—");

    let kayitlar = database
        .income_expense_list(&EntryFilter {
            date_from: "2026-10-01".to_owned(),
            date_to: "2026-10-31".to_owned(),
            ..EntryFilter::default()
        })
        .expect("kayıtlar okunmalı");
    assert_eq!(kayitlar.len(), 4, "rapor kayıt silmemeli/eklememeli");
}

#[test]
fn rapor_varsayilanlari_ve_veri_kaliciligi() {
    let (_directory, config) = gecici_config();

    {
        let context = Context::default();
        let mut app = OnMuhasebeApp::with_config(&context, config.clone());
        assert!(View::Reports.is_ready());
        assert!(!View::Reports.description().is_empty());

        app.set_view(View::Reports);
        {
            let state = app.reports_state();
            assert_eq!(state.tab, ReportKind::CustomerStatement);
            assert!(state.date_from.is_empty());
            assert!(state.date_to.is_empty());
            assert!(state.stock_filter.only_active);
        }

        for tab in [
            ReportKind::SalesPurchase,
            ReportKind::CashSummary,
            ReportKind::IncomeExpense,
        ] {
            app.set_report_tab(tab);
            let state = app.reports_state();
            let (from, to) = on_muhasebe::income_expense::current_month_range();
            assert_eq!(state.date_from, from);
            assert_eq!(state.date_to, to);
        }

        app.set_report_tab(ReportKind::CustomerStatement);
        app.reports_state_mut().date_from = "2026-01-01".to_owned();
        app.reports_state_mut().date_to = "2026-12-31".to_owned();
        app.reports_state_mut().stock_filter.search = "deneme".to_owned();
    }

    let context = Context::default();
    let mut app = OnMuhasebeApp::with_config(&context, config);
    app.set_view(View::Reports);
    let state = app.reports_state();
    assert_eq!(state.tab, ReportKind::CustomerStatement);
    assert!(state.date_from.is_empty());
    assert!(state.date_to.is_empty());
    assert!(state.stock_filter.search.is_empty());
    assert!(state.stock_filter.only_active);
}
