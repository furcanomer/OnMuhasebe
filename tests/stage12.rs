// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Aşama 12 hızlı kabul testleri: kurulum sihirbazı, kısayollar, global arama,
//! odak/başlık ve 900×600 arayüz çizimi.
//!
//! Gerçek veri dizinine dokunulmaz; her test kendi geçici dizinini kullanır.

use std::time::Duration;

use egui::{Key, Modifiers};
use on_muhasebe::app::{
    CustomerScreen, FocusTarget, OnMuhasebeApp, SHORTCUTS, ShortcutAction, View,
};
use on_muhasebe::cash::{AccountType, CashAccountDraft};
use on_muhasebe::config::AppConfig;
use on_muhasebe::customer::CustomerDraft;
use on_muhasebe::db::{Database, SearchKind};

use on_muhasebe::invoice::{InvoiceDraft, InvoiceLineDraft, InvoiceType};
use on_muhasebe::onboarding::OnboardingStep;
use on_muhasebe::product::{ProductDraft, Unit};

fn test_app() -> (tempfile::TempDir, OnMuhasebeApp) {
    let directory = tempfile::tempdir().expect("geçici dizin");
    let config = AppConfig::with_data_dir(directory.path().join("veri"));
    let context = egui::Context::default();
    let app = OnMuhasebeApp::with_config(&context, config);
    (directory, app)
}

fn create_admin(app: &mut OnMuhasebeApp) {
    {
        let state = app.auth_state_mut();
        state.username = "admin".to_owned();
        state.display_name = "Sistem Yöneticisi".to_owned();
        state.password = "GucluSifre1".to_owned();
        state.password_confirm = "GucluSifre1".to_owned();
    }
    app.create_initial_admin().expect("ilk yönetici");
}

fn skip_onboarding(app: &mut OnMuhasebeApp) {
    app.request_onboarding_skip();
    app.confirm_onboarding_skip();
    assert!(!app.onboarding_state().open);
}

fn add_customer(database: &Database, code: &str, name: &str) -> i64 {
    let draft = CustomerDraft {
        code: code.to_owned(),
        name: name.to_owned(),
        ..Default::default()
    };
    let validated = draft.validate().expect("cari doğrulanmalı");
    database
        .customer_insert(&validated)
        .expect("cari eklenmeli")
}

fn add_product(database: &Database, code: &str, name: &str) -> i64 {
    let draft = ProductDraft {
        code: code.to_owned(),
        name: name.to_owned(),
        purchase_price: "10,00".to_owned(),
        sale_price: "12,00".to_owned(),
        starting_stock: "0".to_owned(),
        ..Default::default()
    };
    let validated = draft.validate().expect("ürün doğrulanmalı");
    database.product_insert(&validated).expect("ürün eklenmeli")
}

fn add_cash(database: &Database, name: &str) -> i64 {
    let draft = CashAccountDraft {
        name: name.to_owned(),
        opening_balance: "0".to_owned(),
        ..Default::default()
    };
    let validated = draft.validate().expect("kasa doğrulanmalı");
    database
        .cash_account_insert(&validated)
        .expect("kasa eklenmeli")
}

fn add_invoice(
    database: &Database,
    number: &str,
    customer_id: i64,
    product_id: i64,
    product_code: &str,
    product_name: &str,
) -> i64 {
    let mut draft = InvoiceDraft::new(InvoiceType::Purchase);
    draft.number = number.to_owned();
    draft.customer_id = Some(customer_id);
    draft.lines = vec![InvoiceLineDraft {
        product_id: Some(product_id),
        product_code: product_code.to_owned(),
        product_name: product_name.to_owned(),
        unit: Unit::Adet,
        quantity: "1".to_owned(),
        unit_price: "10,00".to_owned(),
        discount_rate: String::new(),
        vat_rate: "20".to_owned(),
        ..Default::default()
    }];
    let validated = draft.validate().expect("fatura doğrulanmalı");
    database
        .invoice_insert(&validated)
        .expect("fatura eklenmeli")
}

fn render_900(app: &mut OnMuhasebeApp) {
    let context = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(900.0, 600.0),
        )),
        ..Default::default()
    };
    let mut output = context.run_ui(input, |ui| {
        on_muhasebe::ui::layout::draw(ui, app);
    });
    output.textures_delta.clear();
}

#[test]
fn sihirbaz_dort_adimda_tamamlanir_ve_tek_transaction_yazar() {
    let (_directory, mut app) = test_app();
    create_admin(&mut app);

    assert!(app.onboarding_state().open);
    assert_eq!(app.onboarding_state().step, OnboardingStep::Company);

    {
        let state = app.onboarding_state_mut();
        state.company.name = "Deneme Ticaret A.Ş.".to_owned();
        state.company.contact_person = "Ayşe Yılmaz".to_owned();
        state.company.phone = "0212 000 00 00".to_owned();
        state.cash_draft.name = "Merkez Kasa".to_owned();
        state.cash_draft.account_type = AccountType::Kasa;
        state.cash_draft.opening_balance = "1.000,00".to_owned();
    }

    app.onboarding_next();
    assert_eq!(app.onboarding_state().step, OnboardingStep::Cash);
    app.onboarding_next();
    assert_eq!(app.onboarding_state().step, OnboardingStep::Categories);
    app.onboarding_next();
    assert_eq!(app.onboarding_state().step, OnboardingStep::Summary);

    app.onboarding_finish();
    assert!(!app.onboarding_state().open);

    let database = app.database().expect("veritabanı açık");
    assert!(database.onboarding_completed().expect("durum okunmalı"));
    assert_eq!(
        database.onboarding_company_name().expect("şirket adı"),
        Some("Deneme Ticaret A.Ş.".to_owned())
    );
    assert_eq!(database.cash_account_count().expect("kasa sayısı"), 1);
    assert_eq!(
        database.expense_category_count().expect("kategori sayısı"),
        14
    );
}

#[test]
fn sihirbaz_kasa_cakismasinda_tum_yazimlari_geri_alir() {
    let (_directory, mut app) = test_app();
    create_admin(&mut app);

    {
        let database = app.database().expect("veritabanı açık");
        add_cash(database, "Merkez Kasa");
    }
    {
        let state = app.onboarding_state_mut();
        state.company.name = "Geri Alınacak Şirket".to_owned();
        state.cash_draft.name = "Merkez Kasa".to_owned();
    }

    app.onboarding_finish();

    let database = app.database().expect("veritabanı açık");
    assert!(!database.onboarding_completed().expect("durum okunmalı"));
    assert_eq!(
        database.onboarding_company_name().expect("şirket adı"),
        None,
        "transaction geri alınınca şirket anahtarı yazılmamalı"
    );
    assert_eq!(database.cash_account_count().expect("kasa sayısı"), 1);
    assert_eq!(
        database.expense_category_count().expect("kategori sayısı"),
        0
    );
    assert!(app.onboarding_state().open);
    assert_eq!(app.onboarding_state().step, OnboardingStep::Cash);
    assert!(
        app.onboarding_state()
            .cash_errors
            .get(on_muhasebe::cash::Field::Name)
            .is_some_and(|message| message.contains("zaten var"))
    );
}

#[test]
fn sihirbaz_atlanir_ve_ayarlardan_yeniden_acilir() {
    let (_directory, mut app) = test_app();
    create_admin(&mut app);

    app.request_onboarding_skip();
    assert!(app.onboarding_state().skip_confirm);
    app.cancel_onboarding_skip();
    assert!(app.onboarding_state().open);
    assert!(!app.onboarding_state().skip_confirm);

    app.request_onboarding_skip();
    app.confirm_onboarding_skip();
    assert!(!app.onboarding_state().open);
    let database = app.database().expect("veritabanı açık");
    assert!(database.onboarding_completed().expect("durum okunmalı"));
    assert_eq!(
        database.onboarding_company_name().expect("şirket adı"),
        None
    );
    assert_eq!(database.cash_account_count().expect("kasa sayısı"), 0);
    assert_eq!(
        database.expense_category_count().expect("kategori sayısı"),
        0
    );

    app.reopen_onboarding();
    assert!(app.onboarding_state().open);
    assert_eq!(app.onboarding_state().step, OnboardingStep::Company);
}

#[test]
fn klavye_kisayol_tablosu_gerekli_komutlari_icerir() {
    assert!(SHORTCUTS.len() >= 17);
    assert!(SHORTCUTS.iter().any(|shortcut| {
        shortcut.ctrl && shortcut.key == Key::N && shortcut.action == ShortcutAction::New
    }));
    assert!(SHORTCUTS.iter().any(|shortcut| {
        shortcut.ctrl && shortcut.key == Key::S && shortcut.action == ShortcutAction::Save
    }));
    assert!(SHORTCUTS.iter().any(|shortcut| {
        shortcut.ctrl
            && shortcut.key == Key::K
            && shortcut.action == ShortcutAction::Search
            && shortcut.global
    }));
    assert!(SHORTCUTS.iter().any(|shortcut| {
        shortcut.ctrl && shortcut.key == Key::F && shortcut.action == ShortcutAction::Search
    }));
    assert!(SHORTCUTS.iter().any(|shortcut| {
        !shortcut.ctrl && shortcut.key == Key::F5 && shortcut.action == ShortcutAction::Refresh
    }));
    assert!(SHORTCUTS.iter().any(|shortcut| {
        !shortcut.ctrl && shortcut.key == Key::Escape && shortcut.action == ShortcutAction::Cancel
    }));
    assert!(SHORTCUTS.iter().any(|shortcut| {
        shortcut.ctrl && shortcut.key == Key::Comma && shortcut.action == ShortcutAction::Settings
    }));
    assert!(SHORTCUTS.iter().any(|shortcut| {
        shortcut.ctrl && shortcut.key == Key::Q && shortcut.action == ShortcutAction::Logout
    }));
    for (index, view) in View::ALL.iter().enumerate() {
        let key = match index {
            0 => Key::Num1,
            1 => Key::Num2,
            2 => Key::Num3,
            3 => Key::Num4,
            4 => Key::Num5,
            5 => Key::Num6,
            6 => Key::Num7,
            7 => Key::Num8,
            _ => Key::Num9,
        };
        assert!(
            SHORTCUTS.iter().any(|shortcut| {
                shortcut.ctrl
                    && shortcut.key == key
                    && shortcut.action == ShortcutAction::Navigate(*view)
            }),
            "{view:?} için Ctrl+{key:?} tanımlı olmalı"
        );
    }
}

#[test]
fn global_arama_gruplari_ve_turkce_eslesme_calisir() {
    let (_directory, mut app) = test_app();
    create_admin(&mut app);
    skip_onboarding(&mut app);

    let database = app.database().expect("veritabanı açık");
    let mut first_customer = 0;
    for index in 1..=6 {
        let id = add_customer(
            database,
            &format!("TST-C{index:02}"),
            &format!("Test Cari {index:02}"),
        );
        if index == 1 {
            first_customer = id;
        }
    }
    add_customer(database, "OZT-0001", "Öztürk İnşaat");

    let mut first_product = 0;
    for index in 1..=6 {
        let id = add_product(
            database,
            &format!("TST-U{index:02}"),
            &format!("Test Ürün {index:02}"),
        );
        if index == 1 {
            first_product = id;
        }
    }
    add_product(database, "URN-OZEL", "Ürün Özel");

    for index in 1..=6 {
        add_cash(database, &format!("Test Kasa {index:02}"));
    }
    add_cash(database, "Kasa Özel");

    for index in 1..=6 {
        add_invoice(
            database,
            &format!("SF-{index:04}"),
            first_customer,
            first_product,
            "TST-U01",
            "Test Ürün 01",
        );
    }

    let hits = database.global_search("test").expect("arama çalışmalı");
    assert_eq!(hits.len(), 20, "toplam sonuç 20 ile sınırlı olmalı");
    let customers = hits
        .iter()
        .filter(|hit| hit.kind == SearchKind::Customer)
        .count();
    let products = hits
        .iter()
        .filter(|hit| hit.kind == SearchKind::Product)
        .count();
    let invoices = hits
        .iter()
        .filter(|hit| matches!(hit.kind, SearchKind::Invoice(_)))
        .count();
    let cash = hits
        .iter()
        .filter(|hit| hit.kind == SearchKind::CashAccount)
        .count();
    assert_eq!(customers, 5);
    assert_eq!(products, 5);
    assert_eq!(invoices, 5);
    assert_eq!(cash, 5);

    let turkish_product = database.global_search("urun").expect("ürün araması");
    assert!(
        turkish_product.iter().any(|hit| hit.title == "Ürün Özel"),
        "'urun' araması 'Ürün Özel' kaydını bulmalı"
    );
    let turkish_customer = database.global_search("ozturk").expect("cari araması");
    assert!(
        turkish_customer
            .iter()
            .any(|hit| hit.title == "Öztürk İnşaat"),
        "'ozturk' araması 'Öztürk İnşaat' kaydını bulmalı"
    );
    let invoices_by_number = database.global_search("SF-").expect("fatura araması");
    assert_eq!(
        invoices_by_number
            .iter()
            .filter(|hit| matches!(hit.kind, SearchKind::Invoice(_)))
            .count(),
        5
    );
    let cash_hits = database.global_search("kasa").expect("kasa araması");
    assert!(
        cash_hits.iter().any(|hit| hit.title == "Kasa Özel"),
        "'kasa' araması 'Kasa Özel' kaydını bulmalı"
    );
    assert!(
        database
            .global_search("zzzz")
            .expect("boş arama")
            .is_empty()
    );
}

#[test]
fn global_arama_gecikmesi_ve_klavye_gezinmesi_calisir() {
    let (_directory, mut app) = test_app();
    create_admin(&mut app);
    skip_onboarding(&mut app);

    {
        let database = app.database().expect("veritabanı açık");
        add_customer(database, "TST-C01", "Test Cari 01");
        add_product(database, "TST-U01", "Test Ürün 01");
    }

    app.open_search_palette();
    assert!(app.search_state().open);
    assert_eq!(app.pending_focus(), Some(FocusTarget::SearchQuery));

    app.search_state_mut().query = "test".to_owned();
    app.search_query_changed();
    assert!(app.search_state().results.is_empty());
    assert!(app.search_state().changed_at.is_some());

    std::thread::sleep(Duration::from_millis(170));
    app.tick_search();
    for _ in 0..200 {
        if !app.search_state().searching {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
        app.tick_search();
    }
    assert_eq!(app.search_state().last_query, "test");
    assert!(
        !app.search_state().searching,
        "arka plan araması tamamlanmalı"
    );
    assert!(!app.search_state().results.is_empty());

    app.search_move(1);
    assert_eq!(app.search_state().selected, 1);
    app.search_move(-1);
    assert_eq!(app.search_state().selected, 0);

    assert!(app.search_activate_selected());
    assert!(!app.search_state().open);
    assert_eq!(app.view(), View::Customers);
    assert_eq!(
        app.customer_state().screen,
        CustomerScreen::Detail,
        "cari sonucu cari ayrıntısına gitmeli"
    );
}

#[test]
fn ilk_alan_ve_ilk_hata_odagi_calisir() {
    let (_directory, mut app) = test_app();
    create_admin(&mut app);
    skip_onboarding(&mut app);

    app.open_customer_new_form();
    assert_eq!(app.pending_focus(), Some(FocusTarget::CustomerCode));
    assert!(app.take_focus(FocusTarget::CustomerCode));
    {
        let state = app.customer_state_mut();
        state.draft.code.clear();
        state.draft.name.clear();
    }
    app.submit_customer_form();
    assert_eq!(app.pending_focus(), Some(FocusTarget::CustomerCode));

    app.open_product_new_form();
    assert_eq!(app.pending_focus(), Some(FocusTarget::ProductCode));
    {
        let state = app.product_state_mut();
        state.draft.code.clear();
        state.draft.name.clear();
    }
    app.submit_product_form();
    assert_eq!(app.pending_focus(), Some(FocusTarget::ProductCode));

    app.open_cash_account_new_form();
    assert_eq!(app.pending_focus(), Some(FocusTarget::CashName));
    app.cash_state_mut().account_draft.name.clear();
    app.submit_cash_account_form();
    assert_eq!(app.pending_focus(), Some(FocusTarget::CashName));

    app.open_income_expense_category_new_form();
    assert_eq!(app.pending_focus(), Some(FocusTarget::CategoryName));
    app.income_expense_state_mut().category_draft.name.clear();
    app.submit_income_expense_category_form();
    assert_eq!(app.pending_focus(), Some(FocusTarget::CategoryName));

    let customer_id = {
        let database = app.database().expect("veritabanı açık");
        add_customer(database, "TST-C01", "Test Cari 01")
    };
    app.open_invoice_new_form();
    assert_eq!(app.pending_focus(), Some(FocusTarget::InvoiceNumber));
    {
        let state = app.invoice_state_mut();
        state.draft.customer_id = Some(customer_id);
        state.draft.number.clear();
    }
    app.submit_invoice_form();
    assert_eq!(app.pending_focus(), Some(FocusTarget::InvoiceNumber));
}

#[test]
fn degisiklik_yok_mesaji_ve_kirli_takip_calisir() {
    let (_directory, mut app) = test_app();
    create_admin(&mut app);
    skip_onboarding(&mut app);

    app.open_customer_new_form();
    {
        let state = app.customer_state_mut();
        state.draft.code = "TST-C01".to_owned();
        state.draft.name = "Test Cari 01".to_owned();
    }
    app.submit_customer_form();
    let id = app
        .customer_state()
        .detail_id
        .expect("kayıt sonrası ayrıntı açılmalı");

    app.open_customer_edit_form(id);
    assert!(!app.customer_form_dirty());
    app.customer_state_mut().draft.name.push('X');
    assert!(app.customer_form_dirty());
    app.customer_state_mut().draft.name.pop();
    assert!(!app.customer_form_dirty());

    app.submit_customer_form();
    assert_eq!(app.customer_state().screen, CustomerScreen::Form);
    assert_eq!(
        app.customer_state().notice.as_deref(),
        Some("Kaydedilecek değişiklik yok.")
    );
}

#[test]
fn pencere_basligi_ekranla_degisir() {
    let (_directory, mut app) = test_app();
    assert!(app.window_title().contains("İlk kurulum"));
    create_admin(&mut app);

    let overview = app.window_title();
    assert!(overview.contains("Ön Muhasebe"));
    assert!(overview.contains("Sistem Yöneticisi"));
    assert!(overview.contains("Genel Bakış"));
    assert!(overview.contains("v1.0.0"));

    app.set_view(View::Customers);
    let customers = app.window_title();
    assert!(customers.contains("Cari Hesaplar"));
    assert_ne!(overview, customers);
}

#[test]
fn kisayol_yardimi_ve_hakkinda_acilip_kapanir() {
    let (_directory, mut app) = test_app();
    create_admin(&mut app);
    skip_onboarding(&mut app);

    assert!(!app.shortcuts_help_open());
    app.open_shortcuts_help();
    assert!(app.shortcuts_help_open());
    app.close_shortcuts_help();
    assert!(!app.shortcuts_help_open());

    assert!(!app.about_open());
    app.set_about_open(true);
    assert!(app.about_open());
    app.set_about_open(false);
    assert!(!app.about_open());
}

#[test]
fn sihirbaz_aramalar_ve_hakkinda_900x600de_cizilir() {
    let (_directory, mut app) = test_app();
    create_admin(&mut app);

    for step in [
        OnboardingStep::Company,
        OnboardingStep::Cash,
        OnboardingStep::Categories,
        OnboardingStep::Summary,
    ] {
        app.onboarding_state_mut().step = step;
        render_900(&mut app);
    }

    app.request_onboarding_skip();
    render_900(&mut app);
    app.cancel_onboarding_skip();

    skip_onboarding(&mut app);
    app.open_search_palette();
    app.search_state_mut().query = "test".to_owned();
    app.search_query_changed();
    render_900(&mut app);
    app.close_search_palette();

    app.open_shortcuts_help();
    render_900(&mut app);
    app.close_shortcuts_help();

    app.set_about_open(true);
    render_900(&mut app);
    app.set_about_open(false);
}

#[test]
fn metin_odagi_varken_global_olmayan_kisayol_calismaz() {
    let (_directory, mut app) = test_app();
    create_admin(&mut app);
    skip_onboarding(&mut app);

    let context = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(900.0, 600.0),
        )),
        ..Default::default()
    };
    let mut output = context.run_ui(input, |ui| {
        let response = ui.text_edit_singleline(&mut app.search_state_mut().query);
        response.request_focus();
    });
    output.textures_delta.clear();
    assert!(context.egui_wants_keyboard_input());

    app.set_view(View::Overview);
    let before = app.view();
    let context = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(900.0, 600.0),
        )),
        events: vec![egui::Event::Key {
            key: Key::Num2,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::CTRL,
        }],
        ..Default::default()
    };
    let mut output = context.run_ui(input, |ui| {
        let response = ui.text_edit_singleline(&mut app.search_state_mut().query);
        response.request_focus();
        app.handle_shortcuts(ui.ctx());
    });
    output.textures_delta.clear();
    assert_eq!(app.view(), before);
}
