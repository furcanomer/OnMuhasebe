// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Tahsilat/ödeme (kasa-banka hareketleri) modülünün uçtan uca testleri.
//!
//! Gerçek veritabanı ve gerçek uygulama durumu üzerinden tam dağıtım, kısmi
//! dağıtım, çoklu fatura dağıtımı, avans, aşırı dağıtımın reddi, iptal (ters
//! kayıt), fatura iptal koruması, cari bakiyesi, kalıcılık ve bağıntılar
//! (kasa bakiyesi ile fatura ödenen tutarı) doğrulanır.

use egui::Context;
use on_muhasebe::app::{CashScreen, InvoiceScreen, OnMuhasebeApp, View};
use on_muhasebe::config::AppConfig;
use on_muhasebe::customer::CustomerType;
use on_muhasebe::db::Database;
use on_muhasebe::income_expense::EntryType;
use on_muhasebe::invoice::{InvoiceType, PaymentState};
use on_muhasebe::money::Money;
use on_muhasebe::payment::{PaymentStatus, PaymentType};
use tempfile::TempDir;

/// Geçici veri dizini ve bu dizine bağlı yapılandırma üretir.
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

/// Form üzerinden örnek bir cari hesap ekler.
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

/// Form üzerinden örnek bir ürün ekler (başlangıç stoğuyla).
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

/// Form üzerinden fatura ekler ve genel toplamıyla birlikte kimliğini döndürür.
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

    let invoice = app
        .invoice_state()
        .selected
        .clone()
        .expect("fatura kaydedilmeli");
    let total = invoice.totals.grand_total;
    (invoice.id, total)
}

/// Form üzerinden kasa hesabı ekler.
fn kasa_ekle(app: &mut OnMuhasebeApp, name: &str, opening: &str) -> i64 {
    app.set_view(View::CashAndBank);
    app.open_cash_account_new_form();
    {
        let state = app.cash_state_mut();
        state.account_draft.name = name.to_owned();
        state.account_draft.opening_balance = opening.to_owned();
    }
    app.submit_cash_account_form();
    assert!(
        app.cash_state().account_errors.is_empty(),
        "kasa hesabı kaydedilmeli: {:?}",
        app.cash_state().account_errors
    );
    app.cash_state()
        .account_list
        .iter()
        .find(|account| account.name == name)
        .map(|account| account.id)
        .expect("hesap listede bulunmalı")
}

/// Gelir/gider kategorisi ekler ve kimliğini verir.
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

/// Gelir/gider hareketi ekler ve kimliğini verir (onay dahil).
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
    app.confirm_income_expense_entry_save();
    app.income_expense_state()
        .entry_detail_id
        .expect("gelir/gider ayrıntısı açılmalı")
}

// Müşteri + ürün + satış faturası hazırlayıp fatura kimliğini ve toplamı verir.
fn satis_senaryosu(app: &mut OnMuhasebeApp, quantity: &str) -> (i64, Money) {
    let customer_id = musteri_ekle(app, "CAR-0001", "Öztürk İnşaat", CustomerType::Both);
    let product_id = urun_ekle(app, "URN-0001", "Çelik Vida");
    fatura_ekle(
        app,
        InvoiceType::Sales,
        customer_id,
        product_id,
        quantity,
        "10,00",
        "20",
    )
}

#[test]
fn tam_dagitim_ile_tahsilat_kesinlesir_paid_amount_ve_kasa_guncellenir() {
    let (_directory, config) = temporary_config();
    let context = Context::default();

    let (invoice_id, account_id, payment_id, total) = {
        let mut app = OnMuhasebeApp::with_config(&context, config.clone());

        let (invoice_id, total) = satis_senaryosu(&mut app, "2");
        assert_eq!(total.kurus(), 2_400, "2x10,00 + %20 KDV = 24,00");

        let account_id = kasa_ekle(&mut app, "Merkez Kasa", "0,00");

        // Fatura ayrıntısından "Tahsilat Al" ile form açılır.
        app.open_payment_form_for_invoice(invoice_id);
        assert_eq!(app.cash_state().screen, CashScreen::MovementForm);
        assert_eq!(
            app.cash_state().payment_draft.customer_id,
            Some(1),
            "cari faturanın carisine ayarlanmalı"
        );
        assert_eq!(
            app.cash_state().payment_draft.cash_account_id,
            Some(account_id),
            "varsayılan kasa hesabı seçilmeli"
        );
        assert_eq!(app.cash_state().payment_draft.allocations.len(), 1);
        assert_eq!(
            app.cash_state().payment_draft.allocations[0].amount,
            "24,00",
            "kalan tutar önceden dağıtılmalı"
        );
        assert_eq!(app.cash_state().payment_draft.amount, "24,00");

        app.submit_payment_form();
        assert!(
            app.cash_state().payment_errors.is_empty(),
            "doğrulama hatası olmamalı: {:?}",
            app.cash_state().payment_errors
        );
        assert!(app.cash_state().payment_pending.is_some());

        app.confirm_payment_save();
        assert!(
            app.cash_state().general_error.is_none(),
            "kayıtta hata olmamalı: {:?}",
            app.cash_state().general_error
        );
        assert_eq!(
            app.view(),
            View::SalesInvoices,
            "fatura kaynaklı akış faturaya dönmeli"
        );

        let invoice = app.invoice_state().selected.as_ref().expect("fatura açık");
        assert_eq!(invoice.paid_amount.kurus(), 2_400);
        assert_eq!(invoice.remaining(), Money::ZERO);
        assert_eq!(invoice.payment_state(), Some(PaymentState::Paid));

        let payment_id = app
            .cash_state()
            .movement_list
            .first()
            .map(|payment| payment.id)
            .expect("hareket listesinde kayıt olmalı");

        (invoice_id, account_id, payment_id, total)
    };

    // Yeniden açılan veritabanı üzerinden kalıcılık ve bağıntılar.
    let database = Database::open(config.database_path()).expect("veritabanı açılmalı");

    let payment = database.payment_get(payment_id).unwrap().unwrap();
    assert_eq!(payment.payment_type, PaymentType::Tahsilat);
    assert_eq!(payment.status, PaymentStatus::Kesin);
    assert_eq!(payment.amount, total);
    assert_eq!(payment.allocated_total, total);
    assert!(!payment.is_advance());

    let allocations = database.payment_allocations(payment_id).unwrap();
    assert_eq!(allocations.len(), 1);
    assert_eq!(allocations[0].invoice_id, invoice_id);
    assert_eq!(allocations[0].amount, total);

    let invoice = database.invoice_get(invoice_id).unwrap().unwrap();
    assert_eq!(invoice.paid_amount, total);
    assert_eq!(invoice.remaining(), Money::ZERO);

    let account = database.cash_account_get(account_id).unwrap().unwrap();
    assert_eq!(account.current_balance, total);
    assert_eq!(account.payment_count, 1);
}

#[test]
fn kismi_dagitim_faturayi_kismi_odenmis_yapar() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    let (invoice_id, _) = satis_senaryosu(&mut app, "2");
    let account_id = kasa_ekle(&mut app, "Merkez Kasa", "0,00");

    app.open_payment_form_for_invoice(invoice_id);
    app.cash_state_mut().payment_draft.amount = "10,00".to_owned();
    app.cash_state_mut().payment_draft.allocations[0].amount = "10,00".to_owned();
    app.submit_payment_form();
    app.confirm_payment_save();

    let invoice = app.invoice_state().selected.as_ref().unwrap();
    assert_eq!(invoice.paid_amount.kurus(), 1_000);
    assert_eq!(invoice.remaining().kurus(), 1_400);
    assert_eq!(invoice.payment_state(), Some(PaymentState::Partial));

    let database = Database::open(config.database_path()).expect("veritabanı açılmalı");
    assert_eq!(
        database
            .invoice_get(invoice_id)
            .unwrap()
            .unwrap()
            .paid_amount
            .kurus(),
        1_000
    );
    assert_eq!(
        database
            .cash_account_get(account_id)
            .unwrap()
            .unwrap()
            .current_balance
            .kurus(),
        1_000
    );
}

#[test]
fn birden_cok_faturaya_dagitim_yapilabilir() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    let customer_id = musteri_ekle(&mut app, "CAR-0001", "Öztürk İnşaat", CustomerType::Both);
    let product_id = urun_ekle(&mut app, "URN-0001", "Çelik Vida");
    let (first_id, _) = fatura_ekle(
        &mut app,
        InvoiceType::Sales,
        customer_id,
        product_id,
        "2",
        "10,00",
        "20",
    );
    let (second_id, _) = fatura_ekle(
        &mut app,
        InvoiceType::Sales,
        customer_id,
        product_id,
        "1",
        "10,00",
        "20",
    );
    let account_id = kasa_ekle(&mut app, "Merkez Kasa", "0,00");

    app.open_payment_form(PaymentType::Tahsilat);
    app.set_payment_customer(Some(1));
    assert_eq!(
        app.cash_state().payment_draft.allocations.len(),
        2,
        "iki uygun fatura listelenmeli"
    );
    app.cash_state_mut().payment_draft.amount = "36,00".to_owned();
    app.allocate_payment_all();

    app.submit_payment_form();
    assert!(
        app.cash_state().payment_errors.is_empty(),
        "çoklu dağıtım geçerli olmalı: {:?}",
        app.cash_state().payment_errors
    );
    app.confirm_payment_save();

    let payment_id = app.cash_state().detail_id.expect("ayrıntı açılmalı");
    let detail = app.cash_state().detail.as_ref().unwrap();
    assert_eq!(detail.allocated_total.kurus(), 3_600);
    assert_eq!(app.cash_state().detail_allocations.len(), 2);

    let database = Database::open(config.database_path()).expect("veritabanı açılmalı");
    assert_eq!(
        database
            .invoice_get(first_id)
            .unwrap()
            .unwrap()
            .paid_amount
            .kurus(),
        2_400
    );
    assert_eq!(
        database
            .invoice_get(second_id)
            .unwrap()
            .unwrap()
            .paid_amount
            .kurus(),
        1_200
    );
    assert_eq!(
        database
            .cash_account_get(account_id)
            .unwrap()
            .unwrap()
            .current_balance
            .kurus(),
        3_600
    );
    assert_eq!(database.payment_allocations(payment_id).unwrap().len(), 2);
}

#[test]
fn tam_avans_odeme_kasa_bakiyesini_negatife_dusurebilir() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    musteri_ekle(&mut app, "CAR-0001", "Tedarikçi A.Ş.", CustomerType::Both);
    let account_id = kasa_ekle(&mut app, "Merkez Kasa", "0,00");

    app.open_payment_form(PaymentType::Odeme);
    app.set_payment_customer(Some(1));
    app.cash_state_mut().payment_draft.amount = "30,00".to_owned();
    assert!(
        app.cash_state().payment_draft.allocations.is_empty(),
        "uygun alış faturası yoksa dağıtım boş olmalı"
    );

    app.submit_payment_form();
    assert!(
        app.cash_state().payment_errors.is_empty(),
        "tam avans kabul edilmeli: {:?}",
        app.cash_state().payment_errors
    );
    app.confirm_payment_save();

    let payment_id = app.cash_state().detail_id.expect("ayrıntı açılmalı");
    let detail = app.cash_state().detail.as_ref().unwrap();
    assert!(detail.is_advance());
    assert_eq!(detail.remainder().kurus(), 3_000);

    let database = Database::open(config.database_path()).expect("veritabanı açılmalı");
    let account = database.cash_account_get(account_id).unwrap().unwrap();
    assert_eq!(
        account.current_balance.kurus(),
        -3_000,
        "kasa eksiye düşebilir"
    );
    assert_eq!(account.opening_balance, Money::ZERO);
    let _ = payment_id;
}

#[test]
fn asiri_dagitim_ve_kalan_asan_dagitim_reddedilir() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    let (invoice_id, _) = satis_senaryosu(&mut app, "2");
    kasa_ekle(&mut app, "Merkez Kasa", "0,00");

    app.open_payment_form_for_invoice(invoice_id);

    // Dağıtılan tutar ödeme tutarını aşıyor.
    app.cash_state_mut().payment_draft.amount = "10,00".to_owned();
    app.submit_payment_form();
    assert!(app.cash_state().payment_pending.is_none());
    assert!(
        app.cash_state()
            .payment_errors
            .get(on_muhasebe::payment::Field::Allocations)
            .is_some(),
        "ödeme tutarını aşan dağıtım reddedilmeli"
    );

    // Dağıtım fatura kalanını aşıyor.
    app.cash_state_mut().payment_draft.amount = "30,00".to_owned();
    app.cash_state_mut().payment_draft.allocations[0].amount = "30,00".to_owned();
    app.submit_payment_form();
    assert!(
        app.cash_state().payment_errors.allocation(0).is_some(),
        "fatura kalanını aşan dağıtım reddedilmeli"
    );
    assert_eq!(app.cash_state().screen, CashScreen::MovementForm);

    // Hiçbir kayıt yazılmamış olmalı.
    let database = Database::open(config.database_path()).expect("veritabanı açılmalı");
    assert_eq!(database.payment_count().unwrap(), 0);
    assert_eq!(
        database
            .invoice_get(invoice_id)
            .unwrap()
            .unwrap()
            .paid_amount,
        Money::ZERO
    );
}

#[test]
fn tutar_sifir_veya_negatif_olamaz() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    satis_senaryosu(&mut app, "2");
    kasa_ekle(&mut app, "Merkez Kasa", "0,00");

    app.open_payment_form(PaymentType::Tahsilat);
    app.set_payment_customer(Some(1));

    for value in ["0,00", "-5,00", "abc"] {
        app.cash_state_mut().payment_draft.amount = value.to_owned();
        app.submit_payment_form();
        assert!(
            app.cash_state()
                .payment_errors
                .get(on_muhasebe::payment::Field::Amount)
                .is_some(),
            "'{value}' tutarı reddedilmeli"
        );
        assert!(app.cash_state().payment_pending.is_none());
    }
}

#[test]
fn iptal_edilen_tahsilat_paid_amount_ve_kasayi_geri_alir() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    let (invoice_id, _) = satis_senaryosu(&mut app, "2");
    let account_id = kasa_ekle(&mut app, "Merkez Kasa", "0,00");
    app.open_payment_form_for_invoice(invoice_id);
    app.submit_payment_form();
    app.confirm_payment_save();

    let payment_id = app
        .cash_state()
        .movement_list
        .first()
        .map(|payment| payment.id)
        .expect("kayıt olmalı");

    app.open_payment_detail(payment_id);
    assert_eq!(app.cash_state().screen, CashScreen::MovementDetail);

    app.start_payment_cancel();
    app.submit_payment_cancel();
    assert!(
        app.cash_state().cancel_error.is_some(),
        "boş iptal nedeni reddedilmeli"
    );

    app.cash_state_mut().cancel_reason = "Müşteri iade etti".to_owned();
    app.submit_payment_cancel();
    assert!(
        app.cash_state().cancel_error.is_none(),
        "geçerli nedenle iptal edilmeli"
    );

    let detail = app.cash_state().detail.as_ref().unwrap();
    assert_eq!(detail.status, PaymentStatus::Iptal);
    assert!(detail.cancelled_at.is_some());
    assert_eq!(detail.cancel_reason.as_deref(), Some("Müşteri iade etti"));

    let database = Database::open(config.database_path()).expect("veritabanı açılmalı");
    assert_eq!(
        database
            .invoice_get(invoice_id)
            .unwrap()
            .unwrap()
            .paid_amount,
        Money::ZERO
    );
    assert_eq!(
        database
            .cash_account_get(account_id)
            .unwrap()
            .unwrap()
            .current_balance,
        Money::ZERO
    );

    // Cari bakiyesinden düşülür: iptal edilen tahsilat hesaba katılmaz.
    let customer = database.customer_get(1).unwrap().unwrap();
    assert_eq!(customer.collection_total, Money::ZERO);
    assert_eq!(customer.payment_count, 0);
    assert_eq!(
        customer.balance().kurus(),
        2_400,
        "yalnızca satış faturası kalır"
    );
}

#[test]
fn odenen_fatura_iptal_edilemez() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    let (invoice_id, _) = satis_senaryosu(&mut app, "2");
    kasa_ekle(&mut app, "Merkez Kasa", "0,00");
    app.open_payment_form_for_invoice(invoice_id);
    app.submit_payment_form();
    app.confirm_payment_save();

    app.open_invoice_cancel(invoice_id);
    app.invoice_state_mut().cancel_reason = "Vazgeçildi".to_owned();
    app.submit_invoice_cancel();

    assert_eq!(
        app.invoice_state().cancel_error.as_deref(),
        Some("Bu faturaya bağlı tahsilat/ödeme bulunuyor. Önce ilgili ödemeleri iptal edin."),
        "ödenen fatura iptal edilememeli"
    );
    let database = Database::open(config.database_path()).expect("veritabanı açılmalı");
    assert!(
        !database
            .invoice_get(invoice_id)
            .unwrap()
            .unwrap()
            .is_cancelled(),
        "fatura kesin kalmalı"
    );

    // Ödeme iptal edildikten sonra fatura iptal edilebilir.
    let payment_id = app
        .cash_state()
        .movement_list
        .first()
        .map(|payment| payment.id)
        .expect("kayıt olmalı");
    app.open_payment_detail(payment_id);
    app.start_payment_cancel();
    app.cash_state_mut().cancel_reason = "Yanlış fatura".to_owned();
    app.submit_payment_cancel();

    app.open_invoice_cancel(invoice_id);
    app.invoice_state_mut().cancel_reason = "Vazgeçildi".to_owned();
    app.submit_invoice_cancel();
    assert!(
        app.invoice_state().cancel_error.is_none(),
        "ödeme iptalinden sonra fatura iptal edilebilmeli: {:?}",
        app.invoice_state().cancel_error
    );
    let database = Database::open(config.database_path()).expect("veritabanı açılmalı");
    assert!(
        database
            .invoice_get(invoice_id)
            .unwrap()
            .unwrap()
            .is_cancelled()
    );
}

#[test]
fn bagintilar_kasa_bakiyesi_ve_fatura_odeneni_tutarlidir() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    musteri_ekle(&mut app, "CAR-0001", "Öztürk İnşaat", CustomerType::Both);
    let product_id = urun_ekle(&mut app, "URN-0001", "Çelik Vida");
    let (first_id, _) = fatura_ekle(
        &mut app,
        InvoiceType::Sales,
        1,
        product_id,
        "2",
        "10,00",
        "20",
    );
    let (second_id, _) = fatura_ekle(
        &mut app,
        InvoiceType::Sales,
        1,
        product_id,
        "1",
        "10,00",
        "20",
    );
    let account_id = kasa_ekle(&mut app, "Merkez Kasa", "100,00");

    // 1) İlk faturaya tam tahsilat.
    app.open_payment_form_for_invoice(first_id);
    app.submit_payment_form();
    app.confirm_payment_save();
    let kept_payment = app
        .cash_state()
        .movement_list
        .first()
        .map(|payment| payment.id)
        .unwrap();

    // 2) İkinci faturaya kısmi tahsilat, sonra iptal (iptaller bağıntıya girmez).
    app.open_payment_form_for_invoice(second_id);
    app.cash_state_mut().payment_draft.amount = "5,00".to_owned();
    app.cash_state_mut().payment_draft.allocations[0].amount = "5,00".to_owned();
    app.submit_payment_form();
    app.confirm_payment_save();
    let cancelled_payment = app
        .cash_state()
        .movement_list
        .first()
        .map(|payment| payment.id)
        .unwrap();

    app.open_payment_detail(cancelled_payment);
    app.start_payment_cancel();
    app.cash_state_mut().cancel_reason = "Sehven".to_owned();
    app.submit_payment_cancel();

    // 3) Kasadan avans ödeme (hiçbir faturaya dağıtılmaz).
    app.open_payment_form(PaymentType::Odeme);
    app.set_payment_customer(Some(1));
    app.cash_state_mut().payment_draft.amount = "20,00".to_owned();
    app.submit_payment_form();
    app.confirm_payment_save();

    // 4) Cariye bağlı olmayan gelir/gider kayıtları (biri iptal).
    let income_category = kategori_ekle(&mut app, "Kira Geliri", EntryType::Gelir);
    let expense_category = kategori_ekle(&mut app, "Kira", EntryType::Gider);
    let _income_entry = gelir_gider_ekle(
        &mut app,
        EntryType::Gelir,
        income_category,
        account_id,
        "30,00",
    );
    let _expense_entry = gelir_gider_ekle(
        &mut app,
        EntryType::Gider,
        expense_category,
        account_id,
        "10,00",
    );
    let cancelled_entry = gelir_gider_ekle(
        &mut app,
        EntryType::Gider,
        expense_category,
        account_id,
        "5,00",
    );
    app.start_income_expense_cancel();
    app.income_expense_state_mut().cancel_reason = "Sehven".to_owned();
    app.submit_income_expense_cancel();
    assert_eq!(
        app.income_expense_state()
            .entry_detail
            .as_ref()
            .expect("ayrıntı açık kalmalı")
            .status,
        on_muhasebe::income_expense::EntryStatus::Iptal,
        "iptal edilen gider kesinleşmemeli"
    );

    let database = Database::open(config.database_path()).expect("veritabanı açılmalı");

    // Bağıntı 1: current == opening + Σ kesin tahsilat - Σ kesin ödeme
    //                         + Σ kesin gelir - Σ kesin gider
    let account = database.cash_account_get(account_id).unwrap().unwrap();
    let payments = database
        .payment_list(&on_muhasebe::payment::PaymentFilter::default())
        .unwrap();
    let entries = database
        .income_expense_list(&on_muhasebe::income_expense::EntryFilter::default())
        .unwrap();
    let mut expected = account.opening_balance.kurus();
    for payment in &payments {
        if payment.status != PaymentStatus::Kesin {
            continue;
        }
        match payment.payment_type {
            PaymentType::Tahsilat => expected += payment.amount.kurus(),
            PaymentType::Odeme => expected -= payment.amount.kurus(),
        }
    }
    for entry in &entries {
        if entry.status != on_muhasebe::income_expense::EntryStatus::Kesin {
            continue;
        }
        match entry.entry_type {
            EntryType::Gelir => expected += entry.amount.kurus(),
            EntryType::Gider => expected -= entry.amount.kurus(),
        }
    }
    assert_eq!(
        account.current_balance.kurus(),
        expected,
        "kasa bakiyesi kesin tahsilat/ödeme ve gelir/giderle tutarlı olmalı"
    );
    assert_eq!(
        expected,
        account.opening_balance.kurus() + 2_400 - 2_000 + 3_000 - 1_000,
        "iptal edilen 5,00 gider bağıntıya girmemeli"
    );
    assert!(entries.iter().any(|entry| entry.id == cancelled_entry));

    // Bağıntı 2: invoice.paid_amount == Σ (kesin ödemelerin dağıtımları)
    for invoice_id in [first_id, second_id] {
        let mut sum = 0;
        for payment in &payments {
            if payment.status != PaymentStatus::Kesin {
                continue;
            }
            for allocation in database.payment_allocations(payment.id).unwrap() {
                if allocation.invoice_id == invoice_id {
                    sum += allocation.amount.kurus();
                }
            }
        }
        assert_eq!(
            database
                .invoice_get(invoice_id)
                .unwrap()
                .unwrap()
                .paid_amount
                .kurus(),
            sum,
            "fatura ödenen tutarı kesin dağıtımlarla tutarlı olmalı"
        );
    }

    // İptal edilen kayıt cari bakiyesine girmez.
    let customer = database.customer_get(1).unwrap().unwrap();
    assert_eq!(customer.payment_count, 2, "iki kesin kayıt kalmalı");
    assert!(
        !payments
            .iter()
            .any(|p| p.id == kept_payment && p.is_cancelled())
    );
}

#[test]
fn hareket_yeniden_acilista_korunur() {
    let (_directory, config) = temporary_config();
    let context = Context::default();

    let (invoice_id, payment_id) = {
        let mut app = OnMuhasebeApp::with_config(&context, config.clone());
        let (invoice_id, _) = satis_senaryosu(&mut app, "2");
        kasa_ekle(&mut app, "Merkez Kasa", "0,00");
        app.open_payment_form_for_invoice(invoice_id);
        app.submit_payment_form();
        app.confirm_payment_save();
        let payment_id = app
            .cash_state()
            .movement_list
            .first()
            .map(|payment| payment.id)
            .unwrap();
        (invoice_id, payment_id)
    };
    let _ = invoice_id;

    let mut app = OnMuhasebeApp::with_config(&context, config.clone());
    app.set_view(View::CashAndBank);
    app.set_cash_tab(on_muhasebe::app::CashTab::Movements);
    assert_eq!(app.cash_state().movement_list.len(), 1);
    assert_eq!(app.cash_state().movement_list[0].id, payment_id);

    app.open_payment_detail(payment_id);
    let detail = app.cash_state().detail.as_ref().unwrap();
    assert_eq!(detail.amount.kurus(), 2_400);
    assert_eq!(detail.allocated_total.kurus(), 2_400);
    assert_eq!(app.cash_state().detail_allocations.len(), 1);

    let database = Database::open(config.database_path()).expect("veritabanı açılmalı");
    assert_eq!(database.payment_count().unwrap(), 1);
    assert_eq!(
        database
            .invoice_get(invoice_id)
            .unwrap()
            .unwrap()
            .payment_state(),
        Some(PaymentState::Paid)
    );
}

#[test]
fn fatura_listesi_ve_ayrintisi_odeme_durumunu_gosterir() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    let (invoice_id, _) = satis_senaryosu(&mut app, "2");
    kasa_ekle(&mut app, "Merkez Kasa", "0,00");
    app.open_payment_form_for_invoice(invoice_id);
    app.submit_payment_form();
    app.confirm_payment_save();

    // Ayrıntı: bağlı ödeme bağlantıları listelenir.
    let links = app.invoice_payments(invoice_id);
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].amount.kurus(), 2_400);
    assert_eq!(links[0].status, PaymentStatus::Kesin);

    // Liste ekranına dönünce fatura "Ödendi" durumunda görünür.
    app.open_invoice_list();
    assert_eq!(app.invoice_state().screen, InvoiceScreen::List);
    let listed = app
        .invoice_state()
        .list
        .iter()
        .find(|invoice| invoice.id == invoice_id)
        .expect("fatura listede olmalı");
    assert_eq!(listed.payment_state(), Some(PaymentState::Paid));
    assert_eq!(listed.remaining(), Money::ZERO);
}
