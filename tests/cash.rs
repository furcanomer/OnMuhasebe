// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Kasa/banka hesabı modülünün uçtan uca (entegrasyon) testleri.
//!
//! Bu testler gerçek bir veritabanı dosyası ve gerçek uygulama durumu
//! (`OnMuhasebeApp`) üzerinden çalışır: hesap oluşturma/düzenleme, açılış
//! bakiyesi kuralı, IBAN doğrulaması, ad benzersizliği, pasife alma, arama ve
//! filtreleme ile yeniden açılışta kalıcılık doğrulanır.

use egui::Context;
use on_muhasebe::app::{CashScreen, OnMuhasebeApp, View};
use on_muhasebe::cash::{AccountType, CashAccountFilter, Field};
use on_muhasebe::config::AppConfig;
use on_muhasebe::db::{Database, MIGRATIONS, latest_version};
use on_muhasebe::money::Money;
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

/// Form üzerinden kasa hesabı ekler ve kimliğini döndürür.
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

    assert!(
        app.cash_state().account_errors.is_empty(),
        "'{name}' kaydedilirken alan hatası olmamalı: {:?}",
        app.cash_state().account_errors
    );
    assert_eq!(app.cash_state().screen, CashScreen::AccountList);

    hesap_id(app, name)
}

/// Form üzerinden banka hesabı ekler ve kimliğini döndürür.
fn banka_ekle(app: &mut OnMuhasebeApp, name: &str, bank: &str, iban: &str) -> i64 {
    app.set_view(View::CashAndBank);
    app.open_cash_account_new_form();
    {
        let state = app.cash_state_mut();
        state.account_draft.name = name.to_owned();
        state.account_draft.account_type = AccountType::Banka;
        state.account_draft.bank_name = bank.to_owned();
        state.account_draft.iban = iban.to_owned();
    }
    app.submit_cash_account_form();

    assert!(
        app.cash_state().account_errors.is_empty(),
        "'{name}' kaydedilirken alan hatası olmamalı: {:?}",
        app.cash_state().account_errors
    );
    hesap_id(app, name)
}

/// Listede ada göre hesabın kimliğini bulur.
fn hesap_id(app: &OnMuhasebeApp, name: &str) -> i64 {
    app.cash_state()
        .account_list
        .iter()
        .find(|account| account.name == name)
        .map(|account| account.id)
        .unwrap_or_else(|| panic!("'{name}' hesabı listede bulunmalı"))
}

/// Form üzerinden örnek bir cari hesap ekler ve kimliğini döndürür.
fn musteri_ekle(app: &mut OnMuhasebeApp, code: &str, name: &str) -> i64 {
    app.set_view(View::Customers);
    app.open_customer_new_form();
    {
        let state = app.customer_state_mut();
        state.draft.code = code.to_owned();
        state.draft.name = name.to_owned();
        state.draft.kind = on_muhasebe::customer::CustomerType::Both;
    }
    app.submit_customer_form();
    app.customer_state().detail_id.expect("cari oluşmalı")
}

/// Faturaya dağıtılmayan (avans) doğrulanmış ödeme üretir; yalnızca kasa
/// hesabına hareket yazmak için kullanılır.
fn avans_odemesi(
    payment_type: on_muhasebe::payment::PaymentType,
    customer_id: i64,
    cash_account_id: i64,
) -> on_muhasebe::payment::ValidatedPayment {
    on_muhasebe::payment::ValidatedPayment {
        payment_type,
        customer_id,
        cash_account_id,
        payment_date: "2026-10-19".to_owned(),
        amount: Money::from_kurus(1_000),
        description: Some("Hareket oluşturma".to_owned()),
        allocations: Vec::new(),
    }
}

#[test]
fn hesaplar_olusturulur_toplamlar_dogru_ve_yeniden_acilista_korunur() {
    let (_directory, config) = temporary_config();
    let context = Context::default();

    let (kasa_id, banka_id) = {
        let mut app = OnMuhasebeApp::with_config(&context, config.clone());

        let kasa_id = kasa_ekle(&mut app, "Merkez Kasa", "1.000,00");
        let banka_id = banka_ekle(
            &mut app,
            "Ziraat TL",
            "Ziraat Bankası",
            "TR33 0006 1005 1978 6457 8413 26",
        );

        assert_eq!(app.cash_state().account_list.len(), 2);

        let banka = app
            .cash_state()
            .account_list
            .iter()
            .find(|account| account.id == banka_id)
            .expect("banka hesabı bulunmalı");
        assert!(banka.is_bank());
        assert_eq!(banka.bank_name.as_deref(), Some("Ziraat Bankası"));
        assert_eq!(banka.shortened_iban().as_deref(), Some("TR3300…1326"));
        assert!(banka.active);

        let totals = app.cash_totals();
        assert_eq!(totals.cash.kurus(), 100_000, "kasa toplamı açılış bakiyesi");
        assert_eq!(totals.bank.kurus(), 0, "banka toplamı 0 olmalı");
        assert_eq!(totals.total().kurus(), 100_000);
        assert_eq!(app.cash_account_count(), 2);

        (kasa_id, banka_id)
    };

    // Yeniden açılışta hesaplar ve toplamlar korunur.
    let mut app = OnMuhasebeApp::with_config(&context, config.clone());
    app.set_view(View::CashAndBank);

    assert_eq!(app.cash_state().account_list.len(), 2);
    assert_eq!(app.cash_account_count(), 2);
    assert_eq!(app.cash_totals().total().kurus(), 100_000);

    let database = Database::open(config.database_path()).expect("veritabanı açılmalı");
    let kasa = database
        .cash_account_get(kasa_id)
        .unwrap()
        .expect("kasa bulunmalı");
    assert_eq!(kasa.opening_balance.kurus(), 100_000);
    assert_eq!(kasa.current_balance.kurus(), 100_000);
    assert_eq!(kasa.account_type, AccountType::Kasa);
    assert_eq!(
        database
            .cash_account_get(banka_id)
            .unwrap()
            .expect("banka bulunmalı")
            .iban
            .as_deref(),
        Some("TR330006100519786457841326")
    );
}

#[test]
fn hesap_adi_benzersizdir_ve_form_girdiyi_korur() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    kasa_ekle(&mut app, "Merkez Kasa", "0,00");

    // Aynı adın farklı büyük/küçük harf biçimi reddedilir.
    app.open_cash_account_new_form();
    app.cash_state_mut().account_draft.name = "merkez kasa".to_owned();
    app.cash_state_mut().account_draft.notes = "korunmalı".to_owned();
    app.submit_cash_account_form();

    assert_eq!(
        app.cash_state().account_errors.get(Field::Name),
        Some("Bu ad zaten kullanılıyor.")
    );
    assert_eq!(
        app.cash_state().screen,
        CashScreen::AccountForm,
        "hata sonrası form açık kalmalı"
    );
    assert_eq!(
        app.cash_state().account_draft.notes,
        "korunmalı",
        "form girdisi korunmalı"
    );
    assert_eq!(app.cash_account_count(), 1, "ikinci kayıt yazılmamalı");
}

#[test]
fn banka_hesabinda_iban_dogrulanir() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    app.set_view(View::CashAndBank);
    app.open_cash_account_new_form();
    {
        let state = app.cash_state_mut();
        state.account_draft.name = "Hatalı IBAN".to_owned();
        state.account_draft.account_type = AccountType::Banka;
        state.account_draft.iban = "TR12".to_owned();
    }
    app.submit_cash_account_form();

    assert!(
        app.cash_state().account_errors.get(Field::Iban).is_some(),
        "kısa IBAN reddedilmeli"
    );
    assert_eq!(app.cash_account_count(), 0);

    // Geçerli IBAN ile kayıt başarılı olur.
    app.cash_state_mut().account_draft.iban = "TR330006100519786457841326".to_owned();
    app.submit_cash_account_form();
    assert!(
        app.cash_state().account_errors.is_empty(),
        "geçerli IBAN kabul edilmeli: {:?}",
        app.cash_state().account_errors
    );
    assert_eq!(app.cash_account_count(), 1);
}

#[test]
fn kasa_hesabinda_banka_bilgileri_yok_sayilir() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    app.set_view(View::CashAndBank);
    app.open_cash_account_new_form();
    {
        let state = app.cash_state_mut();
        state.account_draft.name = "Merkez Kasa".to_owned();
        state.account_draft.account_type = AccountType::Kasa;
        state.account_draft.bank_name = "Girilmemeli".to_owned();
        state.account_draft.iban = "gecersiz".to_owned();
    }
    app.submit_cash_account_form();

    assert!(
        app.cash_state().account_errors.is_empty(),
        "kasa hesabında banka alanları yok sayılmalı: {:?}",
        app.cash_state().account_errors
    );

    let database = Database::open(config.database_path()).expect("veritabanı açılmalı");
    let account = database
        .cash_account_list(&CashAccountFilter::default())
        .unwrap()
        .into_iter()
        .next()
        .expect("hesap bulunmalı");
    assert_eq!(account.bank_name, None);
    assert_eq!(account.iban, None);
}

#[test]
fn acilis_bakiyesi_yalnizca_hareketsiz_hesapta_degistirilebilir() {
    let (_directory, config) = temporary_config();
    let mut app = open_app(&config).1;

    let customer_id = musteri_ekle(&mut app, "CAR-0001", "Örnek Cari");
    let account_id = kasa_ekle(&mut app, "Merkez Kasa", "100,00");

    // Henüz hareket yok: açılış bakiyesi düzenlenebilir.
    app.open_cash_account_edit_form(account_id);
    assert!(
        app.cash_state().account_opening_editable,
        "hareketsiz hesapta açılış bakiyesi düzenlenebilmeli"
    );
    app.cash_state_mut().account_draft.opening_balance = "250,00".to_owned();
    app.submit_cash_account_form();

    let database = Database::open(config.database_path()).expect("veritabanı açılmalı");
    let account = database.cash_account_get(account_id).unwrap().unwrap();
    assert_eq!(account.opening_balance.kurus(), 25_000);
    assert_eq!(account.current_balance.kurus(), 25_000);

    // Hesaba bir hareket (avans tahsilat) eklenir.
    database
        .payment_insert(&avans_odemesi(
            on_muhasebe::payment::PaymentType::Tahsilat,
            customer_id,
            account_id,
        ))
        .expect("tahsilat eklenebilmeli");

    // Artık açılış bakiyesi düzenlenemez.
    app.open_cash_account_edit_form(account_id);
    assert!(
        !app.cash_state().account_opening_editable,
        "hareketi olan hesapta açılış bakiyesi düzenlenememeli"
    );
    app.cash_state_mut().account_draft.opening_balance = "999,00".to_owned();
    app.submit_cash_account_form();

    let account = database.cash_account_get(account_id).unwrap().unwrap();
    assert_eq!(
        account.opening_balance.kurus(),
        25_000,
        "açılış bakiyesi değişmemeli"
    );
    assert_eq!(
        account.current_balance.kurus(),
        26_000,
        "güncel bakiye hareketle birlikte 250,00 + 10,00 olmalı"
    );
}

#[test]
fn hesap_arama_ve_filtreleri_turkce_karakterlerle_calisir() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    kasa_ekle(&mut app, "Merkez Kasa", "10,00");
    banka_ekle(
        &mut app,
        "Ziraat TL",
        "Ziraat Bankası",
        "TR330006100519786457841326",
    );
    let pasif_id = kasa_ekle(&mut app, "Şube Kasası", "5,00");
    app.toggle_cash_account_active(pasif_id);

    assert_eq!(app.cash_state().account_list.len(), 3);

    // Türkçe karakter normalizasyonu: "sube" → "Şube".
    app.cash_state_mut().account_filter.search = "sube".to_owned();
    app.refresh_cash_account_list();
    assert_eq!(app.cash_state().account_list.len(), 1);
    assert_eq!(app.cash_state().account_list[0].name, "Şube Kasası");

    // Tür filtresi.
    app.cash_state_mut().account_filter = CashAccountFilter::default();
    app.cash_state_mut().account_filter.account_type = Some(AccountType::Banka);
    app.refresh_cash_account_list();
    assert_eq!(app.cash_state().account_list.len(), 1);
    assert_eq!(app.cash_state().account_list[0].name, "Ziraat TL");

    // Durum filtresi (pasif hesaplar).
    app.cash_state_mut().account_filter = CashAccountFilter::default();
    app.cash_state_mut().account_filter.active = Some(false);
    app.refresh_cash_account_list();
    assert_eq!(app.cash_state().account_list.len(), 1);
    assert!(!app.cash_state().account_list[0].active);

    // Temizleme.
    app.reset_cash_account_filter();
    assert_eq!(app.cash_state().account_list.len(), 3);
}

#[test]
fn hesap_pasife_ve_aktife_alinir_silinmez() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    let account_id = kasa_ekle(&mut app, "Merkez Kasa", "500,00");
    assert_eq!(app.cash_totals().cash.kurus(), 50_000);

    app.toggle_cash_account_active(account_id);
    let account = app
        .cash_state()
        .account_list
        .iter()
        .find(|account| account.id == account_id)
        .expect("pasif hesap listede kalmalı");
    assert!(!account.active);
    assert_eq!(
        app.cash_totals().cash.kurus(),
        0,
        "pasif hesap toplamdan çıkarılmalı"
    );

    app.toggle_cash_account_active(account_id);
    assert!(
        app.cash_state()
            .account_list
            .iter()
            .find(|account| account.id == account_id)
            .expect("hesap bulunmalı")
            .active
    );
    assert_eq!(app.cash_totals().cash.kurus(), 50_000);
    assert_eq!(app.cash_account_count(), 1, "kayıt silinmemeli");
}

#[test]
fn migration_v5_sema_surumu_ve_kasa_tablolari_olusur() {
    let (_directory, config) = temporary_config();

    let database = Database::open(config.database_path()).expect("veritabanı açılmalı");
    let status = database.status().expect("durum okunmalı");

    assert_eq!(latest_version(), 7);
    assert_eq!(status.schema_version, 7);
    assert_eq!(status.applied_migrations.len(), MIGRATIONS.len());
    assert!(database.cash_account_count().unwrap() == 0);
    assert!(database.payment_count().unwrap() == 0);

    // Migration sonrası açılış bakiyesi ve toplamlar çalışır.
    let totals = on_muhasebe::cash::CashTotals::default();
    assert_eq!(totals.total(), Money::ZERO);
}
