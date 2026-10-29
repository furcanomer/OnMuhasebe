// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Hızlı kabul testleri: ilk yönetici, giriş, kilit, RBAC,
//! yedekleme ve ayarlar.
//!
//! Gerçek veri dizinine dokunulmaz; her test kendi geçici dizinini kullanır.

use on_muhasebe::app::{AuthScreen, OnMuhasebeApp};
use on_muhasebe::auth::{Role, hash_password};
use on_muhasebe::backup::{self, BackupKind, RotationPolicy};
use on_muhasebe::config::AppConfig;
use on_muhasebe::customer::CustomerDraft;
use on_muhasebe::db::{Database, UserDraft};
use on_muhasebe::error::AppError;
use on_muhasebe::product::ProductDraft;

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

#[test]
fn ilk_acilis_yonetici_olusturma_ekrani_ister() {
    let (_directory, app) = test_app();
    assert_eq!(app.auth_screen(), AuthScreen::FirstAdmin);
    assert!(!app.is_authenticated());
}

#[test]
fn ilk_yonetici_olusturulur_ve_otomatik_giris_yapilir() {
    let (_directory, mut app) = test_app();
    create_admin(&mut app);
    assert!(app.is_authenticated());
    assert!(app.is_manager());
    assert_eq!(
        app.database()
            .expect("veritabanı")
            .user_count()
            .expect("kullanıcı sayısı"),
        1
    );
    let user = app
        .database()
        .unwrap()
        .user_by_username("ADMIN")
        .unwrap()
        .expect("kullanıcı harf duyarsız bulunmalı");
    assert_eq!(user.role, Role::Yonetici);
    assert!(on_muhasebe::auth::is_argon2id_hash(&user.password_hash));
}

#[test]
fn hatali_girisler_kilitlenir_ve_dogru_sifre_sonra_calisir() {
    let (_directory, mut app) = test_app();
    create_admin(&mut app);
    app.logout();
    assert_eq!(app.auth_screen(), AuthScreen::Login);

    let start = std::time::Instant::now();
    app.auth_state_mut().username = "admin".to_owned();
    for _ in 0..5 {
        app.auth_state_mut().password = "yanlis-sifre".to_owned();
        let _ = app.attempt_login_at(start);
    }
    assert!(!app.is_authenticated());
    assert!(matches!(
        app.attempt_login_at(start),
        Err(AppError::LoginThrottled(_))
    ));

    app.auth_state_mut().password = "GucluSifre1".to_owned();
    assert!(
        app.attempt_login_at(start + std::time::Duration::from_secs(31))
            .is_ok(),
        "30 saniye sonra doğru şifre kabul edilmeli"
    );
    assert!(app.is_authenticated());
}

#[test]
fn sifre_degistirme_mevcut_sifreyi_ister() {
    let (_directory, mut app) = test_app();
    create_admin(&mut app);

    assert!(matches!(
        app.change_own_password("yanlis", "YeniSifre1", "YeniSifre1"),
        Err(AppError::InvalidCredentials)
    ));
    app.change_own_password("GucluSifre1", "YeniSifre1", "YeniSifre1")
        .expect("şifre değişmeli");
    app.logout();
    app.auth_state_mut().username = "admin".to_owned();
    app.auth_state_mut().password = "YeniSifre1".to_owned();
    app.attempt_login().expect("yeni şifreyle giriş");
}

#[test]
fn operatore_ayar_ve_sifre_sifirlama_yetkisi_verilmez() {
    let (_directory, mut app) = test_app();
    create_admin(&mut app);
    {
        app.open_new_user_form();
        let form = &mut app.settings_state_mut().user_form;
        form.username = "operator".to_owned();
        form.display_name = "Saha".to_owned();
        form.role = Role::Operator;
        form.password = "Operator1".to_owned();
        form.password_confirm = "Operator1".to_owned();
    }
    app.submit_user_form().expect("operatör eklenmeli");
    let operator_id = app
        .settings_state()
        .users
        .iter()
        .find(|user| user.username == "operator")
        .unwrap()
        .id;

    app.logout();
    {
        let state = app.auth_state_mut();
        state.username = "operator".to_owned();
        state.password = "Operator1".to_owned();
    }
    app.attempt_login().expect("operatör girişi");
    assert!(!app.is_manager());
    app.set_view(on_muhasebe::app::View::Settings);
    assert_eq!(
        app.settings_state().tab,
        on_muhasebe::app::SettingsTab::Security,
        "operatör yalnızca kendi şifre sekmesini görmeli"
    );
    assert!(matches!(
        app.save_general_settings(),
        Err(AppError::PermissionDenied { .. })
    ));
    assert!(matches!(
        app.reset_user_password(operator_id, "YeniSifre1", "YeniSifre1"),
        Err(AppError::PermissionDenied { .. })
    ));
}

#[test]
fn pasif_kullanici_giris_yapamaz() {
    let (_directory, mut app) = test_app();
    create_admin(&mut app);
    app.open_new_user_form();
    {
        let form = &mut app.settings_state_mut().user_form;
        form.username = "pasif".to_owned();
        form.role = Role::Operator;
        form.password = "PasifSifre1".to_owned();
        form.password_confirm = "PasifSifre1".to_owned();
        form.active = false;
    }
    app.submit_user_form().unwrap();
    app.logout();
    app.auth_state_mut().username = "pasif".to_owned();
    app.auth_state_mut().password = "PasifSifre1".to_owned();
    assert!(matches!(
        app.attempt_login(),
        Err(AppError::InvalidCredentials)
    ));
    assert!(!app.is_authenticated());
}

#[test]
fn operator_db_katmaninda_iptal_ve_pasife_almayi_yapamaz() {
    let database = Database::open_in_memory().unwrap();
    let admin = database
        .create_initial_admin("admin", None, &hash_password("GucluSifre1").unwrap())
        .unwrap();
    database.set_actor(Some(admin.actor()));
    let operator = database
        .user_insert(&UserDraft {
            username: "operator".to_owned(),
            display_name: None,
            password_hash: hash_password("Operator1").unwrap(),
            role: Role::Operator,
            active: true,
        })
        .unwrap();
    database.set_actor(Some(operator.actor()));

    assert!(matches!(
        database.customer_set_active(1, false),
        Err(AppError::PermissionDenied { .. })
    ));
    assert!(matches!(
        database.product_set_active(1, false),
        Err(AppError::PermissionDenied { .. })
    ));
    assert!(matches!(
        database.invoice_cancel(1, "deneme"),
        Err(AppError::PermissionDenied { .. })
    ));
    assert!(matches!(
        database.payment_cancel(1, "deneme"),
        Err(AppError::PermissionDenied { .. })
    ));
    assert!(matches!(
        database.cash_account_set_active(1, false),
        Err(AppError::PermissionDenied { .. })
    ));
    assert!(matches!(
        database.expense_category_set_active(1, false),
        Err(AppError::PermissionDenied { .. })
    ));
    assert!(matches!(
        database.income_expense_cancel(1, "deneme"),
        Err(AppError::PermissionDenied { .. })
    ));
    assert!(matches!(
        database.user_list(),
        Err(AppError::PermissionDenied { .. })
    ));
}

#[test]
fn operator_kayit_guncellemesiyle_pasife_alamaz() {
    let database = Database::open_in_memory().unwrap();
    let _admin = database
        .create_initial_admin("admin", None, &hash_password("GucluSifre1").unwrap())
        .unwrap();
    let operator = database
        .user_insert(&UserDraft {
            username: "operator".to_owned(),
            display_name: None,
            password_hash: hash_password("Operator1").unwrap(),
            role: Role::Operator,
            active: true,
        })
        .unwrap();
    database.set_actor(Some(operator.actor()));

    let customer = CustomerDraft {
        code: "CAR-0001".to_owned(),
        name: "Test Cari".to_owned(),
        ..CustomerDraft::default()
    }
    .validate()
    .expect("cari doğrulama");
    let customer_id = database.customer_insert(&customer).unwrap();
    let mut passive_customer = customer.clone();
    passive_customer.active = false;
    assert!(matches!(
        database.customer_update(customer_id, &passive_customer),
        Err(AppError::PermissionDenied { .. })
    ));
    assert!(
        database
            .customer_get(customer_id)
            .unwrap()
            .expect("cari bulunmalı")
            .active
    );

    let product = ProductDraft {
        code: "URN-0001".to_owned(),
        name: "Test Ürün".to_owned(),
        purchase_price: "10".to_owned(),
        sale_price: "12".to_owned(),
        ..ProductDraft::default()
    }
    .validate()
    .expect("ürün doğrulama");
    let product_id = database.product_insert(&product).unwrap();
    let mut passive_product = product.clone();
    passive_product.active = false;
    assert!(matches!(
        database.product_update(product_id, &passive_product),
        Err(AppError::PermissionDenied { .. })
    ));
    assert!(
        database
            .product_get(product_id)
            .unwrap()
            .expect("ürün bulunmalı")
            .active
    );
}

#[test]
fn yonetici_kendi_hesabini_pasife_alamaz() {
    let database = Database::open_in_memory().unwrap();
    let admin = database
        .create_initial_admin("admin", None, &hash_password("GucluSifre1").unwrap())
        .unwrap();
    database.set_actor(Some(admin.actor()));
    assert!(matches!(
        database.user_update(admin.id, None, Role::Yonetici, false),
        Err(AppError::SelfOperationProtected)
    ));
}

#[test]
fn son_aktif_yonetici_korunur() {
    let database = Database::open_in_memory().unwrap();
    let admin = database
        .create_initial_admin("admin", None, &hash_password("GucluSifre1").unwrap())
        .unwrap();
    assert!(matches!(
        database.user_delete(admin.id),
        Err(AppError::LastAdminProtected)
    ));
    database.set_actor(Some(admin.actor()));
    assert!(matches!(
        database.user_update(admin.id, None, Role::Operator, true),
        Err(AppError::SelfOperationProtected)
    ));
    assert!(matches!(
        database.user_delete(admin.id),
        Err(AppError::SelfOperationProtected)
    ));
}

#[test]
fn genel_ayarlar_kaydedilir_ve_okunur() {
    let (_directory, mut app) = test_app();
    create_admin(&mut app);
    {
        let settings = app.settings_mut();
        settings.auto_lock_enabled = false;
        settings.auto_lock_minutes = 30;
        settings.log_info = true;
    }
    app.save_general_settings().expect("genel ayarlar");
    assert!(!app.settings().auto_lock_enabled);
    assert_eq!(app.settings().auto_lock_minutes, 30);
    assert!(app.settings().log_info);

    let database = app.database().expect("veritabanı");
    assert_eq!(
        database
            .setting_get("auto_lock_enabled")
            .unwrap()
            .as_deref(),
        Some("0")
    );
    assert_eq!(
        database
            .setting_get("auto_lock_minutes")
            .unwrap()
            .as_deref(),
        Some("30")
    );
    assert_eq!(
        database.setting_get("log_level").unwrap().as_deref(),
        Some("info")
    );
}

#[test]
fn yonetici_manuel_yedek_alir_ve_dogrular() {
    let (directory, mut app) = test_app();
    create_admin(&mut app);
    let path = app.create_manual_backup().expect("manuel yedek");
    assert!(path.exists());
    assert!(
        path.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("onmuhasebe-manual-")
    );
    let info = Database::validate_backup(&path).expect("yedek doğrulanmalı");
    assert_eq!(info.schema_version, on_muhasebe::db::latest_version());
    assert!(
        directory.path().join("veri").join("backups").exists(),
        "yedekler veri dizini yanında tutulmalı"
    );
}

#[test]
fn yonetici_geri_yukleme_onayi_hazirlar_ve_iptal_eder() {
    let (_directory, mut app) = test_app();
    create_admin(&mut app);
    let path = app.create_manual_backup().expect("manuel yedek");

    app.prepare_restore(&path).expect("geri yükleme onayı");
    assert!(app.transfer_state().pending_restore.is_some());
    app.cancel_restore();
    assert!(app.transfer_state().pending_restore.is_none());
}

#[test]
fn operator_geri_yukleme_hazirlayamaz() {
    let (_directory, mut app) = test_app();
    create_admin(&mut app);
    let path = app.create_manual_backup().expect("manuel yedek");
    app.logout();
    app.auth_state_mut().username = "operator".to_owned();
    app.auth_state_mut().password = "Operator1".to_owned();
    let database = app.database().expect("veritabanı");
    database
        .user_insert(&UserDraft {
            username: "operator".to_owned(),
            display_name: None,
            password_hash: hash_password("Operator1").unwrap(),
            role: Role::Operator,
            active: true,
        })
        .unwrap();
    app.attempt_login().expect("operatör girişi");
    assert!(matches!(
        app.prepare_restore(&path),
        Err(AppError::PermissionDenied { .. })
    ));
}

#[test]
fn acilista_otomatik_yedek_alir_ve_zaman_damgasini_yazar() {
    let (directory, mut app) = test_app();
    create_admin(&mut app);
    assert!(app.settings().last_auto_backup_at.is_none());

    app.run_startup_auto_backup().expect("otomatik yedek");

    assert!(app.settings().last_auto_backup_at.is_some());
    let backups = directory.path().join("veri").join("backups");
    let names: Vec<String> = std::fs::read_dir(&backups)
        .expect("yedek klasörü")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        names
            .iter()
            .any(|name| name.starts_with("onmuhasebe-auto-") && name.ends_with(".onm-backup")),
        "otomatik yedek dosyası bulunmalı: {names:?}"
    );

    assert!(
        !app.settings()
            .should_auto_backup(chrono::Local::now().naive_local()),
        "24 saat dolmadan yeni otomatik yedek alınmamalı"
    );
}

#[test]
fn otomatik_yedek_zaman_damgasi_ve_rotasyon_ayarlari() {
    let (directory, mut app) = test_app();
    create_admin(&mut app);
    let path = {
        let db = app.database().unwrap();
        let target = directory
            .path()
            .join("veri")
            .join("backups")
            .join(backup::backup_file_name(
                BackupKind::Auto,
                chrono::NaiveDate::from_ymd_opt(2026, 10, 23)
                    .unwrap()
                    .and_hms_opt(10, 0, 0)
                    .unwrap(),
            ));
        backup::create_backup(db, &target).expect("otomatik yedek");
        target
    };
    assert!(path.exists());
    app.settings_mut().rotation = RotationPolicy {
        daily_keep: 10,
        monthly_keep: 6,
        yearly_keep: 3,
    };
    app.save_backup_settings().expect("rotasyon ayarları");
    assert_eq!(app.settings().rotation.daily_keep, 10);
}

fn draw_900(
    context: &egui::Context,
    app: &mut OnMuhasebeApp,
    draw: impl FnOnce(&mut egui::Ui, &mut OnMuhasebeApp),
) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(900.0, 600.0),
        )),
        ..Default::default()
    };
    let mut draw = Some(draw);
    context
        .run_ui(input, |ui| {
            if let Some(draw) = draw.take() {
                draw(ui, app);
            }
        })
        .drop_without_applying_deltas();
}

#[test]
fn ayarlar_ekrani_900x600de_dort_sekmede_cizilir() {
    use on_muhasebe::app::SettingsTab;

    let directory = tempfile::tempdir().unwrap();
    let config = AppConfig::with_data_dir(directory.path().join("veri"));
    let context = egui::Context::default();
    let mut app = OnMuhasebeApp::with_config(&context, config);
    create_admin(&mut app);
    app.set_view(on_muhasebe::app::View::Settings);
    for tab in [
        SettingsTab::General,
        SettingsTab::Security,
        SettingsTab::Backup,
        SettingsTab::Users,
    ] {
        app.settings_state_mut().tab = tab;
        draw_900(&context, &mut app, |ui, app| {
            on_muhasebe::ui::layout::draw(ui, app);
        });
    }
}

#[test]
fn giris_ve_ilk_yonetici_ekranlari_900x600de_cizilir() {
    let directory = tempfile::tempdir().unwrap();
    let config = AppConfig::with_data_dir(directory.path().join("veri"));
    let context = egui::Context::default();
    let mut app = OnMuhasebeApp::with_config(&context, config);
    assert_eq!(app.auth_screen(), AuthScreen::FirstAdmin);
    draw_900(&context, &mut app, on_muhasebe::ui::auth::show);
    create_admin(&mut app);
    app.logout();
    assert_eq!(app.auth_screen(), AuthScreen::Login);
    draw_900(&context, &mut app, on_muhasebe::ui::auth::show);
}
