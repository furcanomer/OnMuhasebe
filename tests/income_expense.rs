// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Gelir/gider modülünün uçtan uca testleri.
//!
//! Kategori benzersizliği, pasife alma, atomik kayıt/iptal, kasa bakiyesi,
//! cari/fatura değişmezliği, negatif bakiye, beş terimli kasa bağıntısı,
//! dönem özeti/yüzdeler, migration v6 sonrası kalıcılık ve uygulama akışı
//! gerçek veritabanı üzerinden doğrulanır.

use egui::Context;
use on_muhasebe::app::{
    CashScreen, CashTab, EntryOrigin, IncomeExpenseScreen, IncomeExpenseTab, MovementTab,
    OnMuhasebeApp, View,
};
use on_muhasebe::cash::AccountType;
use on_muhasebe::config::AppConfig;
use on_muhasebe::customer::CustomerType;
use on_muhasebe::db::{Database, latest_version};
use on_muhasebe::income_expense::{
    CategoryField, CategoryFilter, EntryField, EntryFilter, EntryStatus, EntryType, PeriodSummary,
    ValidatedCategory, ValidatedEntry,
};
use on_muhasebe::invoice::InvoiceType;
use on_muhasebe::money::Money;
use on_muhasebe::payment::{PaymentStatus, PaymentType};
use tempfile::TempDir;

/// Geçici veri dizini ve yapılandırma üretir.
fn temporary_config() -> (TempDir, AppConfig) {
    let directory = tempfile::tempdir().expect("geçici dizin oluşturulmalı");
    let config = AppConfig::with_data_dir(directory.path().join("veri"));
    (directory, config)
}

/// Yapılandırmaya bağlı uygulama durumu üretir.
fn open_app(config: &AppConfig) -> (Context, OnMuhasebeApp) {
    let context = Context::default();
    let app = OnMuhasebeApp::with_config(&context, config.clone());
    (context, app)
}

/// Form üzerinden kasa hesabı ekler.
fn kasa_ekle(app: &mut OnMuhasebeApp, name: &str, opening: &str) -> i64 {
    app.set_view(View::CashAndBank);
    app.open_cash_account_new_form();
    {
        let state = app.cash_state_mut();
        state.account_draft.name = name.to_owned();
        state.account_draft.account_type = AccountType::Kasa;
        state.account_draft.opening_balance = opening.to_owned();
    }
    app.submit_cash_account_form();
    app.cash_state()
        .account_list
        .iter()
        .find(|account| account.name == name)
        .map(|account| account.id)
        .expect("kasa hesabı oluşmalı")
}

/// Form üzerinden cari hesap ekler.
fn musteri_ekle(app: &mut OnMuhasebeApp, code: &str, name: &str, kind: CustomerType) -> i64 {
    app.set_view(View::Customers);
    app.open_customer_new_form();
    {
        let state = app.customer_state_mut();
        state.draft.code = code.to_owned();
        state.draft.name = name.to_owned();
        state.draft.kind = kind;
    }
    app.submit_customer_form();
    app.customer_state().detail_id.expect("cari oluşmalı")
}

/// Form üzerinden ürün ekler (başlangıç stoğuyla).
fn urun_ekle(app: &mut OnMuhasebeApp, code: &str, name: &str) -> i64 {
    app.set_view(View::Inventory);
    app.open_product_new_form();
    {
        let state = app.product_state_mut();
        state.draft.code = code.to_owned();
        state.draft.name = name.to_owned();
        state.draft.purchase_price = "10,00".to_owned();
        state.draft.sale_price = "10,00".to_owned();
        state.draft.vat_rate = "20".to_owned();
        state.draft.starting_stock = "100".to_owned();
    }
    app.submit_product_form();
    app.product_state().detail_id.expect("ürün oluşmalı")
}

/// Form üzerinden fatura ekler ve kimliği ile genel toplamı döndürür.
fn fatura_ekle(
    app: &mut OnMuhasebeApp,
    invoice_type: InvoiceType,
    customer_id: i64,
    product_id: i64,
    quantity: &str,
    unit_price: &str,
    vat: &str,
) -> (i64, Money) {
    app.set_view(match invoice_type {
        InvoiceType::Purchase => View::PurchaseInvoices,
        InvoiceType::Sales => View::SalesInvoices,
    });
    app.open_invoice_new_form();
    app.apply_invoice_line_product(0, product_id);
    {
        let state = app.invoice_state_mut();
        state.draft.customer_id = Some(customer_id);
        let line = &mut state.draft.lines[0];
        line.quantity = quantity.to_owned();
        line.unit_price = unit_price.to_owned();
        line.discount_rate = "0".to_owned();
        line.vat_rate = vat.to_owned();
    }
    app.submit_invoice_form();
    assert!(
        app.invoice_state().errors.is_empty(),
        "fatura doğrulaması geçmeli: {:?}",
        app.invoice_state().errors
    );
    app.confirm_invoice_save();
    assert!(
        app.invoice_state().general_error.is_none(),
        "fatura kaydında hata olmamalı: {:?}",
        app.invoice_state().general_error
    );
    let id = app.invoice_state().detail_id.expect("fatura oluşmalı");
    let total = app
        .invoice_state()
        .selected
        .as_ref()
        .expect("fatura ayrıntısı açılmalı")
        .totals
        .grand_total;
    (id, total)
}

/// Form üzerinden gelir/gider kategorisi ekler.
fn kategori_ekle(app: &mut OnMuhasebeApp, name: &str, category_type: EntryType) -> i64 {
    app.set_view(View::IncomeExpense);
    app.open_income_expense_category_new_form();
    {
        let state = app.income_expense_state_mut();
        state.category_draft.name = name.to_owned();
        state.category_draft.category_type = category_type;
    }
    app.submit_income_expense_category_form();
    assert!(
        app.income_expense_state().category_errors.is_empty(),
        "kategori kaydedilmeli: {:?}",
        app.income_expense_state().category_errors
    );
    app.income_expense_state()
        .categories
        .iter()
        .find(|category| category.name == name)
        .map(|category| category.id)
        .expect("kategori listede bulunmalı")
}

/// Form üzerinden gelir/gider hareketi ekler (onay dahil).
fn gelir_gider_ekle(
    app: &mut OnMuhasebeApp,
    entry_type: EntryType,
    category_id: i64,
    account_id: i64,
    amount: &str,
) -> i64 {
    app.open_income_expense_entry_form(entry_type);
    app.set_income_expense_category(Some(category_id));
    app.set_income_expense_cash_account(Some(account_id));
    app.income_expense_state_mut().entry_draft.amount = amount.to_owned();
    app.submit_income_expense_entry_form();
    assert!(
        app.income_expense_state().entry_errors.is_empty(),
        "gelir/gider doğrulaması geçmeli: {:?}",
        app.income_expense_state().entry_errors
    );
    assert!(
        app.income_expense_state().entry_pending.is_some(),
        "kaydetmeden önce onay istenmeli"
    );
    app.confirm_income_expense_entry_save();
    assert!(
        app.income_expense_state().general_error.is_none(),
        "gelir/gider kaydında hata olmamalı: {:?}",
        app.income_expense_state().general_error
    );
    app.income_expense_state()
        .entry_detail_id
        .expect("gelir/gider ayrıntısı açılmalı")
}

/// Uygulamadaki kasa hesabının güncel bakiyesini okur.
fn hesap_bakiyesi(app: &OnMuhasebeApp, account_id: i64) -> Money {
    app.cash_state()
        .account_list
        .iter()
        .find(|account| account.id == account_id)
        .expect("kasa hesabı listede olmalı")
        .current_balance
}

/// Doğrudan veritabanına kategori ekler.
fn db_kategori(database: &Database, name: &str, category_type: EntryType) -> i64 {
    database
        .expense_category_insert(&ValidatedCategory {
            name: name.to_owned(),
            category_type,
            description: Some(format!("{name} açıklaması")),
            active: true,
        })
        .expect("kategori eklenmeli")
}

/// Doğrudan veritabanına kasa hesabı ekler.
fn db_hesap(database: &Database, name: &str, opening_kurus: i64) -> i64 {
    use on_muhasebe::cash::CashAccountDraft;

    let validated = CashAccountDraft {
        name: name.to_owned(),
        account_type: AccountType::Kasa,
        opening_balance: Money::from_kurus(opening_kurus).format_turkish(),
        ..CashAccountDraft::default()
    }
    .validate()
    .expect("kasa hesabı doğrulanmalı");
    database
        .cash_account_insert(&validated)
        .expect("kasa hesabı eklenmeli")
}

/// Doğrudan veritabanına gelir/gider hareketi ekler.
fn db_hareket(
    database: &Database,
    entry_type: EntryType,
    category_id: i64,
    account_id: i64,
    date: &str,
    amount_kurus: i64,
) -> i64 {
    database
        .income_expense_insert(&ValidatedEntry {
            entry_type,
            category_id,
            cash_account_id: account_id,
            entry_date: date.to_owned(),
            amount: Money::from_kurus(amount_kurus),
            description: Some(format!("{entry_type:?} test kaydı")),
            document_ref: Some("FIS-TEST".to_owned()),
        })
        .expect("gelir/gider eklenmeli")
}

#[test]
fn kategori_benzersizligi_pasiflik_ve_pasif_kategori_reddi() {
    let (_directory, config) = temporary_config();
    let database = Database::open(config.database_path()).expect("veritabanı açılmalı");

    let gider_id = db_kategori(&database, "Kira", EntryType::Gider);
    assert!(
        database
            .expense_category_name_exists("kira", EntryType::Gider, None)
            .unwrap(),
        "ad karşılaştırması büyük/küçük harf duyarsız olmalı"
    );

    let error = database
        .expense_category_insert(&ValidatedCategory {
            name: "KIRA".to_owned(),
            category_type: EntryType::Gider,
            description: None,
            active: true,
        })
        .expect_err("aynı türde aynı ad reddedilmeli");
    assert!(matches!(
        error,
        on_muhasebe::error::AppError::ExpenseCategoryNameTaken(_)
    ));

    // Aynı ad farklı türde kullanılabilir.
    let gelir_id = db_kategori(&database, "Kira", EntryType::Gelir);
    assert_ne!(gider_id, gelir_id);

    // Kendi kaydı düzenlenirken ad çakışması sayılmaz.
    database
        .expense_category_update(
            gider_id,
            &ValidatedCategory {
                name: "kira".to_owned(),
                category_type: EntryType::Gider,
                description: None,
                active: true,
            },
        )
        .expect("kendi adıyla güncelleme geçmeli");

    // Pasife alma: kayıt silinmez, listede kalır, yeni harekette seçilemez.
    database
        .expense_category_set_active(gider_id, false)
        .expect("pasife alınmalı");
    let category = database
        .expense_category_get(gider_id)
        .unwrap()
        .expect("kategori korunmalı");
    assert!(!category.is_active());
    assert_eq!(category.status_label(), "Pasif");

    let all = database
        .expense_category_list(&CategoryFilter::default())
        .unwrap();
    assert!(all.iter().any(|category| category.id == gider_id));
    let passive = database
        .expense_category_list(&CategoryFilter {
            active: Some(false),
            ..CategoryFilter::default()
        })
        .unwrap();
    assert!(passive.iter().any(|category| category.id == gider_id));
    let active = database
        .expense_category_list(&CategoryFilter {
            active: Some(true),
            ..CategoryFilter::default()
        })
        .unwrap();
    assert!(!active.iter().any(|category| category.id == gider_id));

    let account_id = db_hesap(&database, "Merkez Kasa", 10_000);
    let error = database
        .income_expense_insert(&ValidatedEntry {
            entry_type: EntryType::Gider,
            category_id: gider_id,
            cash_account_id: account_id,
            entry_date: "2026-10-18".to_owned(),
            amount: Money::from_kurus(1_000),
            description: None,
            document_ref: None,
        })
        .expect_err("pasif kategoriyle hareket reddedilmeli");
    assert!(matches!(
        error,
        on_muhasebe::error::AppError::ExpenseCategoryInactive(_)
    ));
    assert_eq!(
        database
            .cash_account_get(account_id)
            .unwrap()
            .unwrap()
            .current_balance,
        Money::from_kurus(10_000),
        "reddedilen kayıt bakiyeyi değiştirmemeli"
    );
}

#[test]
fn gelir_gider_atomik_olarak_kasa_bakiyesini_gunceller_ve_negatif_bakiye_kabul_edilir() {
    let (_directory, config) = temporary_config();
    let database = Database::open(config.database_path()).expect("veritabanı açılmalı");

    let account_id = db_hesap(&database, "Merkez Kasa", 10_000);
    let gelir = db_kategori(&database, "Kira Geliri", EntryType::Gelir);
    let gider = db_kategori(&database, "Kira", EntryType::Gider);

    let income_id = db_hareket(
        &database,
        EntryType::Gelir,
        gelir,
        account_id,
        "2026-10-18",
        5_000,
    );
    assert_eq!(
        database
            .cash_account_get(account_id)
            .unwrap()
            .unwrap()
            .current_balance,
        Money::from_kurus(15_000),
        "gelir bakiyeyi artırmalı"
    );

    let expense_id = db_hareket(
        &database,
        EntryType::Gider,
        gider,
        account_id,
        "2026-10-18",
        20_000,
    );
    let account = database.cash_account_get(account_id).unwrap().unwrap();
    assert_eq!(
        account.current_balance,
        Money::from_kurus(-5_000),
        "negatif bakiye sessizce kabul edilmeli"
    );
    assert_eq!(account.entry_count, 2);
    assert!(account.has_movements());

    for id in [income_id, expense_id] {
        let entry = database.income_expense_get(id).unwrap().unwrap();
        assert_eq!(entry.status, EntryStatus::Kesin);
        assert_eq!(
            entry.category_id,
            if id == income_id { gelir } else { gider }
        );
    }
    assert_eq!(database.income_expense_count().unwrap(), 2);

    // Kategori türü uyuşmazlığında ne kayıt ne bakiye değişir.
    let error = database
        .income_expense_insert(&ValidatedEntry {
            entry_type: EntryType::Gelir,
            category_id: gider,
            cash_account_id: account_id,
            entry_date: "2026-10-18".to_owned(),
            amount: Money::from_kurus(1_000),
            description: None,
            document_ref: None,
        })
        .expect_err("tür uyuşmazlığı reddedilmeli");
    assert!(matches!(
        error,
        on_muhasebe::error::AppError::ExpenseCategoryTypeMismatch(_)
    ));
    assert_eq!(
        database
            .cash_account_get(account_id)
            .unwrap()
            .unwrap()
            .current_balance,
        Money::from_kurus(-5_000)
    );

    // Pasif hesap da reddedilir.
    database
        .cash_account_set_active(account_id, false)
        .expect("hesap pasife alınmalı");
    let error = database
        .income_expense_insert(&ValidatedEntry {
            entry_type: EntryType::Gelir,
            category_id: gelir,
            cash_account_id: account_id,
            entry_date: "2026-10-18".to_owned(),
            amount: Money::from_kurus(1_000),
            description: None,
            document_ref: None,
        })
        .expect_err("pasif hesap reddedilmeli");
    assert!(matches!(
        error,
        on_muhasebe::error::AppError::CashAccountInactive(_)
    ));
    assert_eq!(database.income_expense_count().unwrap(), 2);
}

#[test]
fn iptal_kasa_bakiyesini_geri_alir_cari_ve_faturaya_dokunmaz() {
    let (_directory, config) = temporary_config();
    let (context, mut app) = open_app(&config);

    let customer_id = musteri_ekle(&mut app, "CAR-0001", "Öztürk İnşaat", CustomerType::Both);
    let product_id = urun_ekle(&mut app, "URN-0001", "Çelik Vida");
    let (invoice_id, invoice_total) = fatura_ekle(
        &mut app,
        InvoiceType::Sales,
        customer_id,
        product_id,
        "2",
        "10,00",
        "20",
    );
    assert_eq!(invoice_total, Money::from_kurus(2_400));
    let account_id = kasa_ekle(&mut app, "Merkez Kasa", "100,00");
    let gelir = kategori_ekle(&mut app, "Kira Geliri", EntryType::Gelir);
    let gider = kategori_ekle(&mut app, "Kira", EntryType::Gider);

    let income_id = gelir_gider_ekle(&mut app, EntryType::Gelir, gelir, account_id, "30,00");
    let expense_id = gelir_gider_ekle(&mut app, EntryType::Gider, gider, account_id, "10,00");
    assert_eq!(hesap_bakiyesi(&app, account_id), Money::from_kurus(12_000));
    assert_eq!(
        app.income_expense_state().entry_detail.as_ref().unwrap().id,
        expense_id
    );

    // İptal neden zorunludur; boş neden kaydı değiştirmez.
    app.start_income_expense_cancel();
    app.submit_income_expense_cancel();
    assert!(
        app.income_expense_state().cancel_error.is_some(),
        "boş iptal nedeni reddedilmeli"
    );
    assert_eq!(hesap_bakiyesi(&app, account_id), Money::from_kurus(12_000));
    assert_eq!(
        app.income_expense_state()
            .entry_detail
            .as_ref()
            .unwrap()
            .status,
        EntryStatus::Kesin
    );

    app.income_expense_state_mut().cancel_reason = "Sehven girildi".to_owned();
    app.submit_income_expense_cancel();
    assert!(
        app.income_expense_state().general_error.is_none(),
        "iptalde hata olmamalı: {:?}",
        app.income_expense_state().general_error
    );
    let cancelled = app
        .income_expense_state()
        .entry_detail
        .as_ref()
        .expect("ayrıntı açık kalmalı")
        .clone();
    assert_eq!(cancelled.status, EntryStatus::Iptal);
    assert_eq!(cancelled.cancel_reason.as_deref(), Some("Sehven girildi"));
    assert!(cancelled.cancelled_at.is_some());
    assert_eq!(
        hesap_bakiyesi(&app, account_id),
        Money::from_kurus(13_000),
        "iptal gider etkisini geri almalı"
    );

    // Aynı kayıt ikinci kez iptal edilemez (veritabanı düzeyinde).
    let database = Database::open(config.database_path()).expect("veritabanı açılmalı");
    let error = database
        .income_expense_cancel(expense_id, "Tekrar")
        .expect_err("ikinci iptal reddedilmeli");
    assert!(matches!(
        error,
        on_muhasebe::error::AppError::IncomeExpenseAlreadyCancelled(_)
    ));
    assert_eq!(hesap_bakiyesi(&app, account_id), Money::from_kurus(13_000));

    let _ = context;
    let invoice = database.invoice_get(invoice_id).unwrap().unwrap();
    assert_eq!(
        invoice.paid_amount,
        Money::ZERO,
        "gelir/gider fatura ödenen tutarını değiştirmemeli"
    );
    let customer = database.customer_get(customer_id).unwrap().unwrap();
    assert_eq!(
        customer.balance(),
        invoice_total,
        "gelir/gider cari bakiyesini değiştirmemeli"
    );
    assert_eq!(customer.invoice_count, 1);
    assert_eq!(customer.payment_count, 0);

    let entries = database
        .income_expense_list(&EntryFilter::default())
        .unwrap();
    assert_eq!(entries.len(), 2, "iptal edilen kayıt da korunmalı");
    assert!(entries.iter().any(|entry| entry.id == income_id));
    assert!(
        entries
            .iter()
            .any(|entry| entry.id == expense_id && entry.is_cancelled())
    );
    assert_eq!(
        database
            .expense_category_get(gider)
            .unwrap()
            .unwrap()
            .usage_count,
        1,
        "iptal edilen kayıt kullanım sayısına dahil olmalı"
    );
}

#[test]
fn kasa_bakiyesi_bes_terimli_bagintiyla_tutarlidir() {
    let (_directory, config) = temporary_config();
    let (context, mut app) = open_app(&config);

    let customer_id = musteri_ekle(&mut app, "CAR-0001", "Öztürk İnşaat", CustomerType::Both);
    let product_id = urun_ekle(&mut app, "URN-0001", "Çelik Vida");
    let (invoice_id, invoice_total) = fatura_ekle(
        &mut app,
        InvoiceType::Sales,
        customer_id,
        product_id,
        "1",
        "1.000,00",
        "20",
    );
    assert_eq!(invoice_total, Money::from_kurus(120_000));
    let account_id = kasa_ekle(&mut app, "Merkez Kasa", "100,00");
    let _ = context;

    let database = Database::open(config.database_path()).expect("veritabanı açılmalı");
    let gelir = db_kategori(&database, "Kira Geliri", EntryType::Gelir);
    let gider = db_kategori(&database, "Kira", EntryType::Gider);

    database
        .payment_insert(&on_muhasebe::payment::ValidatedPayment {
            payment_type: PaymentType::Tahsilat,
            customer_id,
            cash_account_id: account_id,
            payment_date: "2026-10-18".to_owned(),
            amount: Money::from_kurus(40_000),
            description: None,
            allocations: vec![on_muhasebe::payment::ValidatedAllocation {
                invoice_id,
                amount: Money::from_kurus(40_000),
            }],
        })
        .expect("tahsilat eklenmeli");
    database
        .payment_insert(&on_muhasebe::payment::ValidatedPayment {
            payment_type: PaymentType::Odeme,
            customer_id,
            cash_account_id: account_id,
            payment_date: "2026-10-18".to_owned(),
            amount: Money::from_kurus(20_000),
            description: None,
            allocations: Vec::new(),
        })
        .expect("ödeme eklenmeli");
    db_hareket(
        &database,
        EntryType::Gelir,
        gelir,
        account_id,
        "2026-10-18",
        30_000,
    );
    let cancelled_expense = db_hareket(
        &database,
        EntryType::Gider,
        gider,
        account_id,
        "2026-10-18",
        5_000,
    );
    db_hareket(
        &database,
        EntryType::Gider,
        gider,
        account_id,
        "2026-10-18",
        10_000,
    );
    database
        .income_expense_cancel(cancelled_expense, "Yanlış kayıt")
        .expect("iptal edilmeli");

    let account = database.cash_account_get(account_id).unwrap().unwrap();
    let payments = database
        .payment_list(&on_muhasebe::payment::PaymentFilter::default())
        .unwrap();
    let entries = database
        .income_expense_list(&EntryFilter::default())
        .unwrap();

    let mut expected = account.opening_balance.kurus();
    for payment in &payments {
        if payment.status != PaymentStatus::Kesin {
            continue;
        }
        expected += payment.amount.kurus() * payment.payment_type.cash_sign();
    }
    for entry in &entries {
        if entry.status != EntryStatus::Kesin {
            continue;
        }
        expected += entry.amount.kurus() * entry.entry_type.cash_sign();
    }

    assert_eq!(
        account.current_balance.kurus(),
        expected,
        "current == opening + Σ kesin tahsilat - Σ kesin ödeme + Σ kesin gelir - Σ kesin gider"
    );
    assert_eq!(expected, 10_000 + 40_000 - 20_000 + 30_000 - 10_000);
    assert_eq!(
        database
            .invoice_get(invoice_id)
            .unwrap()
            .unwrap()
            .paid_amount,
        Money::from_kurus(40_000),
        "fatura ödenen tutarı yalnızca tahsilattan gelmeli"
    );

    let customer = database.customer_get(customer_id).unwrap().unwrap();
    assert_eq!(
        customer.payment_count, 2,
        "gelir/gider cari hareket sayılmaz"
    );
    assert_eq!(
        customer.balance().kurus(),
        invoice_total.kurus() - 40_000 + 20_000,
        "cari bakiyesi yalnızca tahsilat/ödeme ile değişmeli"
    );
}

#[test]
fn donem_ozeti_toplamlari_kategori_yuzdeleri_ve_sifir_taban() {
    let (_directory, config) = temporary_config();
    let database = Database::open(config.database_path()).expect("veritabanı açılmalı");

    let account_id = db_hesap(&database, "Merkez Kasa", 0);
    let kira_geliri = db_kategori(&database, "Kira Geliri", EntryType::Gelir);
    let faiz = db_kategori(&database, "Faiz Geliri", EntryType::Gelir);
    let kira = db_kategori(&database, "Kira", EntryType::Gider);

    db_hareket(
        &database,
        EntryType::Gelir,
        kira_geliri,
        account_id,
        "2026-10-02",
        6_000,
    );
    db_hareket(
        &database,
        EntryType::Gelir,
        faiz,
        account_id,
        "2026-10-03",
        4_000,
    );
    db_hareket(
        &database,
        EntryType::Gider,
        kira,
        account_id,
        "2026-10-04",
        2_500,
    );
    let cancelled = db_hareket(
        &database,
        EntryType::Gelir,
        kira_geliri,
        account_id,
        "2026-10-05",
        9_999,
    );
    database
        .income_expense_cancel(cancelled, "Test iptali")
        .expect("iptal edilmeli");
    // Dönem dışı kayıt özete girmemeli.
    db_hareket(
        &database,
        EntryType::Gelir,
        kira_geliri,
        account_id,
        "2026-11-01",
        50_000,
    );

    let period_entries = database
        .income_expense_list(&EntryFilter {
            date_from: "2026-10-01".to_owned(),
            date_to: "2026-10-31".to_owned(),
            ..EntryFilter::default()
        })
        .expect("dönem kayıtları okunmalı");
    let summary = PeriodSummary::from_entries("2026-10-01", "2026-10-31", period_entries.iter());
    assert_eq!(summary.income_total, Money::from_kurus(10_000));
    assert_eq!(summary.expense_total, Money::from_kurus(2_500));
    assert_eq!(summary.net(), Money::from_kurus(7_500));
    assert_eq!(summary.record_count, 4);
    assert_eq!(summary.confirmed_count, 3);
    assert!(!summary.is_empty());
    assert!(summary.has_confirmed());

    let kira_geliri_row = summary
        .categories
        .iter()
        .find(|row| row.category_id == kira_geliri)
        .expect("Kira Geliri dağılımda olmalı");
    assert_eq!(kira_geliri_row.total, Money::from_kurus(6_000));
    assert_eq!(kira_geliri_row.percentage, Some(60));
    let faiz_row = summary
        .categories
        .iter()
        .find(|row| row.category_id == faiz)
        .expect("Faiz Geliri dağılımda olmalı");
    assert_eq!(faiz_row.percentage, Some(40));
    let kira_row = summary
        .categories
        .iter()
        .find(|row| row.category_id == kira)
        .expect("Kira dağılımda olmalı");
    assert_eq!(kira_row.percentage, Some(100));

    let account_row = summary
        .accounts
        .iter()
        .find(|row| row.cash_account_id == account_id)
        .expect("hesap dağılımda olmalı");
    assert_eq!(account_row.income_total, Money::from_kurus(10_000));
    assert_eq!(account_row.expense_total, Money::from_kurus(2_500));
    assert_eq!(account_row.net(), Money::from_kurus(7_500));

    // Yalnızca iptal kayıt varsa özet boş sayılmaz; yüzdeler üretilmez.
    let cancelled_entries = database
        .income_expense_list(&EntryFilter {
            date_from: "2026-10-05".to_owned(),
            date_to: "2026-10-05".to_owned(),
            ..EntryFilter::default()
        })
        .expect("iptal kayıtları okunmalı");
    let only_cancelled =
        PeriodSummary::from_entries("2026-10-05", "2026-10-05", cancelled_entries.iter());
    assert_eq!(only_cancelled.record_count, 1);
    assert_eq!(only_cancelled.confirmed_count, 0);
    assert!(!only_cancelled.is_empty());
    assert!(!only_cancelled.has_confirmed());
    assert_eq!(only_cancelled.income_total, Money::ZERO);
    assert_eq!(only_cancelled.expense_total, Money::ZERO);
    assert!(only_cancelled.categories.is_empty());

    // Sıfır taban yüzde üretmez; arayüz bunu "—" olarak gösterir.
    assert_eq!(Money::from_kurus(100).percentage_of(Money::ZERO), None);
    assert_eq!(
        Money::from_kurus(0).percentage_of(Money::from_kurus(1_000)),
        Some(0)
    );
    assert_eq!(
        Money::from_kurus(10_000).percentage_of(Money::from_kurus(3_000)),
        Some(100),
        "yüzde 0-100 aralığına kırpılmalı"
    );
}

#[test]
fn veritabani_kapatilip_acildiginda_kayitlar_korunur() {
    let (_directory, config) = temporary_config();

    let (category_id, account_id, entry_id) = {
        let database = Database::open(config.database_path()).expect("veritabanı açılmalı");
        assert_eq!(latest_version(), 7);
        assert_eq!(database.status().unwrap().schema_version, 7);

        let account_id = db_hesap(&database, "Merkez Kasa", 10_000);
        let category_id = db_kategori(&database, "Kira", EntryType::Gider);
        let entry_id = db_hareket(
            &database,
            EntryType::Gider,
            category_id,
            account_id,
            "2026-10-18",
            2_500,
        );
        (category_id, account_id, entry_id)
    };

    let database = Database::open(config.database_path()).expect("yeniden açılmalı");
    let entry = database.income_expense_get(entry_id).unwrap().unwrap();
    assert_eq!(entry.entry_type, EntryType::Gider);
    assert_eq!(entry.category_id, category_id);
    assert_eq!(entry.amount, Money::from_kurus(2_500));
    assert_eq!(entry.status, EntryStatus::Kesin);

    let account = database.cash_account_get(account_id).unwrap().unwrap();
    assert_eq!(account.current_balance, Money::from_kurus(7_500));
    assert_eq!(account.entry_count, 1);
    let category = database.expense_category_get(category_id).unwrap().unwrap();
    assert_eq!(category.usage_count, 1);
}

#[test]
fn uygulama_akisi_kategori_hareket_iptal_ozet_ve_birlesik_kasa() {
    let (_directory, config) = temporary_config();
    let (context, mut app) = open_app(&config);

    let account_id = kasa_ekle(&mut app, "Merkez Kasa", "1.000,00");
    let gelir = kategori_ekle(&mut app, "Kira Geliri", EntryType::Gelir);
    let gider = kategori_ekle(&mut app, "Kira", EntryType::Gider);

    // Aynı türde büyük/küçük harf farkıyla aynı ad reddedilir; form korunur.
    app.open_income_expense_category_new_form();
    {
        let state = app.income_expense_state_mut();
        state.category_draft.name = "kira".to_owned();
        state.category_draft.category_type = EntryType::Gider;
    }
    app.submit_income_expense_category_form();
    assert_eq!(
        app.income_expense_state().screen,
        IncomeExpenseScreen::CategoryForm
    );
    assert_eq!(
        app.income_expense_state().category_draft.name,
        "kira",
        "hata sonrası girdi korunmalı"
    );
    assert!(
        app.income_expense_state()
            .category_errors
            .get(CategoryField::Name)
            .is_some()
    );
    app.cancel_income_expense_category_form();

    // Pasif kategori yeni hareket seçeneklerinde görünmez.
    app.toggle_income_expense_category_active(gider);
    assert!(
        app.income_expense_entry_category_options()
            .iter()
            .all(|(id, _)| *id != gider)
    );
    app.toggle_income_expense_category_active(gider);

    // Eksik tutar formu kapatmaz, girdiyi korur.
    app.open_income_expense_entry_form(EntryType::Gelir);
    app.set_income_expense_category(Some(gelir));
    app.set_income_expense_cash_account(Some(account_id));
    app.income_expense_state_mut().entry_draft.description = "Açıklama korunmalı".to_owned();
    app.submit_income_expense_entry_form();
    assert_eq!(
        app.income_expense_state().screen,
        IncomeExpenseScreen::EntryForm
    );
    assert!(
        app.income_expense_state()
            .entry_errors
            .get(EntryField::Amount)
            .is_some()
    );
    assert_eq!(
        app.income_expense_state().entry_draft.description,
        "Açıklama korunmalı"
    );

    app.income_expense_state_mut().entry_draft.amount = "5.000,00".to_owned();
    app.submit_income_expense_entry_form();
    assert!(
        app.income_expense_state().entry_pending.is_some(),
        "onay istenmeli"
    );
    app.confirm_income_expense_entry_save();
    assert_eq!(
        app.income_expense_state().notice.as_deref(),
        Some("Gelir/Gider kaydedildi.")
    );
    assert_eq!(
        app.income_expense_state().screen,
        IncomeExpenseScreen::EntryDetail
    );
    assert_eq!(hesap_bakiyesi(&app, account_id), Money::from_kurus(600_000));
    let income_id = app.income_expense_state().entry_detail_id.unwrap();

    let expense_id = gelir_gider_ekle(&mut app, EntryType::Gider, gider, account_id, "2.500,00");
    assert_eq!(hesap_bakiyesi(&app, account_id), Money::from_kurus(350_000));

    // Aylık özet ve Genel Bakış toplamları.
    let (month, has_month) = app.month_income_expense();
    assert!(has_month);
    assert_eq!(month.income_total, Money::from_kurus(500_000));
    assert_eq!(month.expense_total, Money::from_kurus(250_000));
    assert_eq!(month.net(), Money::from_kurus(250_000));

    app.set_income_expense_tab(IncomeExpenseTab::Summary);
    app.set_income_expense_summary_this_month();
    let summary = app.income_expense_state().summary.clone();
    assert_eq!(summary.net(), Money::from_kurus(250_000));
    assert_eq!(summary.categories.len(), 2);

    // Kasa/banka Tümü listesinde gelir/gider kayıtları görünür.
    app.set_view(View::CashAndBank);
    app.set_cash_tab(CashTab::Movements);
    app.set_movement_tab(MovementTab::All);
    assert_eq!(app.cash_state().movement_entries.len(), 2);
    assert_eq!(
        app.cash_movement_net(),
        Money::from_kurus(250_000),
        "birleşik net nakit etkisi gelir - gider olmalı"
    );

    // Gelir/gider ayrıntısı kasa birleşik listesinden açılıp geri dönmeli.
    app.open_income_expense_detail_from_cash(income_id);
    assert_eq!(
        app.income_expense_state().entry_origin,
        EntryOrigin::CashMovements
    );
    app.back_from_income_expense_detail();
    assert_eq!(app.view(), View::CashAndBank);
    assert_eq!(app.cash_state().screen, CashScreen::MovementList);
    assert_eq!(app.cash_state().movement_tab, MovementTab::All);

    // Gider iptal edilince bakiye geri alınır.
    app.open_income_expense_detail(expense_id);
    app.start_income_expense_cancel();
    app.income_expense_state_mut().cancel_reason = "Sehven".to_owned();
    app.submit_income_expense_cancel();
    assert_eq!(hesap_bakiyesi(&app, account_id), Money::from_kurus(600_000));
    let (month, _) = app.month_income_expense();
    assert_eq!(month.income_total, Money::from_kurus(500_000));
    assert_eq!(month.expense_total, Money::ZERO);
    assert_eq!(month.net(), Money::from_kurus(500_000));

    app.set_view(View::CashAndBank);
    app.set_cash_tab(CashTab::Movements);
    app.set_movement_tab(MovementTab::All);
    assert_eq!(app.cash_state().movement_entries.len(), 2);
    assert_eq!(app.cash_movement_net(), Money::from_kurus(500_000));

    let _ = context;
}
