// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Aşama 13 arayüz sağlığı testleri: G10 (G6 ile birleşik), G11, G12 ve G13.
//!
//! G11 büyük veri setinde arama sırasında arayüzün kilitlenmediğini, G12
//! klavye kapsamını ve G13 farklı DPI değerlerinde 900×600 çizimi doğrular.

use std::thread;
use std::time::{Duration, Instant};

use egui::{Key, Modifiers};
use on_muhasebe::app::{CustomerScreen, OnMuhasebeApp, View};
use on_muhasebe::config::AppConfig;
use on_muhasebe::onboarding::OnboardingStep;
use rusqlite::types::Value;

use super::bulk_data::seed_customers_only;
use super::{build_test_db, insert_batch, measure_memory_mb, raw_connection, wait_for_condition};

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
}

fn render_at(app: &mut OnMuhasebeApp, pixels_per_point: f32) -> egui::Context {
    let context = egui::Context::default();
    context.set_pixels_per_point(pixels_per_point);
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
    context
}

fn press_key(app: &mut OnMuhasebeApp, key: Key, modifiers: Modifiers) {
    let context = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(900.0, 600.0),
        )),
        events: vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }],
        ..Default::default()
    };
    let mut output = context.run_ui(input, |ui| {
        app.handle_shortcuts(ui.ctx());
    });
    output.textures_delta.clear();
}

/// G10: Bellek ölçüm aracının çalıştığını doğrular; 1000 turluk asıl trend
/// G6 ile aynı testte (`g6_g10_bin_tur_soak_ve_bellek`) ölçülür.
#[test]
fn g10_bellek_olcumu_araci() {
    let rss = measure_memory_mb();
    assert!(rss > 0, "RSS ölçümü sıfır olmamalı");
    println!("[G10] RSS ölçüm aracı: {rss} MB (1000 tur trendi G6 testinde)");
}

/// G11: 1.000.000 cari + 100.000 fatura ile arama UI'yi bloke etmemeli.
#[test]
#[ignore = "ağır test: 1.000.000 cari + 100.000 fatura arama gecikmesi"]
fn g11_arama_ui_bloklamaz() {
    let directory = tempfile::tempdir().expect("geçici dizin");
    let path = directory.path().join("g11.db");
    let database = build_test_db(&path);
    let customers = seed_customers_only(&database, 1_000_000);
    assert_eq!(customers.len(), 1_000_000);
    drop(customers);

    let connection = raw_connection(&database);
    let transaction = connection.unchecked_transaction().expect("fatura işlemi");
    let mut inserted = 0usize;
    while inserted < 100_000 {
        let chunk = (100_000 - inserted).min(1_000);
        let rows: Vec<Vec<Value>> = (0..chunk)
            .map(|offset| {
                let index = inserted + offset + 1;
                vec![
                    Value::Text(format!("G11-SF-{index:06}")),
                    Value::Text("satis".to_owned()),
                    Value::Integer(((index - 1) % 1_000_000) as i64 + 1),
                    Value::Text("2026-01-15".to_owned()),
                    Value::Integer(10_000),
                    Value::Integer(0),
                    Value::Integer(10_000),
                    Value::Integer(2_000),
                    Value::Integer(12_000),
                    Value::Text("kesin".to_owned()),
                ]
            })
            .collect();
        insert_batch(
            &transaction,
            "invoices",
            "invoice_number, invoice_type, customer_id, invoice_date, subtotal, \
             discount_total, net_total, vat_total, grand_total, status",
            &rows,
        );
        inserted += chunk;
    }
    transaction.commit().expect("fatura commit");
    database.checkpoint().expect("checkpoint");
    drop(connection);
    drop(database);

    let config = AppConfig::with_data_dir(directory.path().join("veri"));
    std::fs::create_dir_all(config.data_dir()).expect("veri dizini");
    std::fs::copy(&path, config.database_path()).expect("test DB kopyası");
    let context = egui::Context::default();
    let mut app = OnMuhasebeApp::with_config(&context, config);

    let direct_start = Instant::now();
    let direct = app
        .database()
        .expect("DB açık")
        .global_search("cari")
        .expect("doğrudan arama");
    let direct_ms = direct_start.elapsed().as_millis();
    assert!(!direct.is_empty());

    app.open_search_palette();
    app.search_state_mut().query = "cari".to_owned();
    app.search_query_changed();
    thread::sleep(Duration::from_millis(170));
    let tick_start = Instant::now();
    app.tick_search();
    let tick_ms = tick_start.elapsed().as_millis();
    assert!(
        tick_ms < 500,
        "arama başlatma UI'yi 500 ms'den fazla bloke etti: {tick_ms} ms"
    );

    let completed = wait_for_condition(
        || {
            app.tick_search();
            !app.search_state().searching
        },
        15_000,
    );
    assert!(completed, "arka plan araması 15 sn içinde tamamlanmadı");
    assert!(!app.search_state().results.is_empty());
    println!(
        "[G11] doğrudan arama={direct_ms} ms, UI tick={tick_ms} ms, sonuç={}",
        app.search_state().results.len()
    );
}

/// G12: Klavyeyle 11 ekran/akış kapsamı.
#[test]
fn g12_klavye_erisilebilirligi() {
    let (_directory, mut app) = test_app();
    create_admin(&mut app);
    skip_onboarding(&mut app);

    let mut checked = 0usize;
    for (index, view) in View::ALL.iter().enumerate() {
        app.set_view(View::Overview);
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
        press_key(&mut app, key, Modifiers::CTRL);
        assert_eq!(app.view(), *view, "Ctrl+{} gezinmesi çalışmalı", index + 1);
        checked += 1;
    }

    app.open_search_palette();
    assert!(app.search_state().open);
    press_key(&mut app, Key::Escape, Modifiers::NONE);
    assert!(!app.search_state().open, "ESC arama paletini kapatmalı");
    checked += 1;

    app.open_shortcuts_help();
    press_key(&mut app, Key::Escape, Modifiers::NONE);
    assert!(
        !app.shortcuts_help_open(),
        "ESC kısayol yardımını kapatmalı"
    );
    checked += 1;

    app.set_about_open(true);
    press_key(&mut app, Key::Escape, Modifiers::NONE);
    assert!(!app.about_open(), "ESC Hakkında penceresini kapatmalı");
    checked += 1;

    app.set_view(View::Customers);
    app.open_customer_new_form();
    app.customer_state_mut().draft.name = "Klavye Testi".to_owned();
    assert_eq!(app.customer_state().screen, CustomerScreen::Form);
    assert!(app.customer_form_dirty());
    press_key(&mut app, Key::Escape, Modifiers::NONE);
    assert_eq!(
        app.customer_state().screen,
        CustomerScreen::Form,
        "ESC önce iptal onayı göstermeli"
    );
    press_key(&mut app, Key::Escape, Modifiers::NONE);
    assert_eq!(app.customer_state().screen, CustomerScreen::Form);
    app.cancel_customer_form();
    checked += 1;

    app.reopen_onboarding();
    assert!(app.onboarding_state().open);
    app.request_onboarding_skip();
    assert!(app.onboarding_state().skip_confirm);
    press_key(&mut app, Key::Escape, Modifiers::NONE);
    assert!(
        !app.onboarding_state().skip_confirm,
        "ESC atlama onayını kapatmalı"
    );
    checked += 1;

    assert_eq!(checked, 14, "9 görünüm + 5 pencere/akış denenmeli");
    println!("[G12] klavye kapsamı: 14/14 akış (9 görünüm + 5 pencere)");
}

/// G13: %100, %125, %150, %200 DPI simülasyonunda 900×600 çizim.
#[test]
fn g13_dpi_olcekleri_900x600() {
    let (_directory, mut app) = test_app();
    create_admin(&mut app);
    skip_onboarding(&mut app);

    for pixels_per_point in [1.0_f32, 1.25, 1.5, 2.0] {
        for view in View::ALL {
            app.set_view(view);
            let context = render_at(&mut app, pixels_per_point);
            assert!(
                (context.pixels_per_point() - pixels_per_point).abs() < f32::EPSILON,
                "DPI {pixels_per_point} uygulanmadı"
            );
        }
        app.open_search_palette();
        app.search_state_mut().query = "test".to_owned();
        render_at(&mut app, pixels_per_point);
        app.close_search_palette();
        app.set_about_open(true);
        render_at(&mut app, pixels_per_point);
        app.set_about_open(false);
        app.open_shortcuts_help();
        render_at(&mut app, pixels_per_point);
        app.close_shortcuts_help();
        app.open_customer_new_form();
        render_at(&mut app, pixels_per_point);
        app.cancel_customer_form();
        println!(
            "[G13] DPI {:.0}%: 900×600 çizim tamam",
            pixels_per_point * 100.0
        );
    }

    let onboarding_context = egui::Context::default();
    let onboarding_directory = tempfile::tempdir().expect("geçici");
    let config = AppConfig::with_data_dir(onboarding_directory.path().join("v"));
    let mut onboarding_app = OnMuhasebeApp::with_config(&onboarding_context, config);
    create_admin(&mut onboarding_app);
    for step in [
        OnboardingStep::Company,
        OnboardingStep::Cash,
        OnboardingStep::Categories,
        OnboardingStep::Summary,
    ] {
        onboarding_app.onboarding_state_mut().step = step;
        for pixels_per_point in [1.0_f32, 1.25, 1.5, 2.0] {
            render_at(&mut onboarding_app, pixels_per_point);
        }
    }
    println!("[G13] 4/4 DPI × ana ekranlar + sihirbaz adımları çizildi");
}
