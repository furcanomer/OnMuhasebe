// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Ürün ve stok modülünün uçtan uca (entegrasyon) testleri.
//!
//! Bu testler gerçek bir veritabanı dosyası ve gerçek uygulama durumu
//! (`OnMuhasebeApp`) üzerinden çalışır. Böylece yalnızca birimlerin değil,
//! katmanların birlikte doğru çalıştığı doğrulanır: doğrulama, kalıcılık,
//! yeniden açılış, arama/filtreleme, stok hareketleri ve hata mesajları.

use egui::Context;
use on_muhasebe::app::{OnMuhasebeApp, ProductScreen, View};
use on_muhasebe::config::AppConfig;
use on_muhasebe::db::{Database, MIGRATIONS, latest_version};
use on_muhasebe::error::AppError;
use on_muhasebe::product::{Field, MovementType, ProductDraft, ProductFilter, Unit};
use on_muhasebe::quantity::Quantity;
use tempfile::TempDir;

/// Geçici veri dizini ve bu dizine bağlı yapılandırma üretir.
fn temporary_config() -> (TempDir, AppConfig) {
    let directory = tempfile::tempdir().expect("geçici dizin oluşturulmalı");
    let config = AppConfig::with_data_dir(directory.path().join("veri"));
    (directory, config)
}

/// Yapılandırmaya bağlı, ürün ve stok bölümü açılmış uygulama durumu üretir.
fn open_app(config: &AppConfig) -> (Context, OnMuhasebeApp) {
    let context = Context::default();
    let mut app = OnMuhasebeApp::with_config(&context, config.clone());
    app.set_view(View::Inventory);
    (context, app)
}

/// Form üzerinden örnek bir ürün ekler ve oluşan kaydın kimliğini döndürür.
fn form_ile_ekle(app: &mut OnMuhasebeApp, code: &str, name: &str) -> i64 {
    app.open_product_new_form();
    {
        let state = app.product_state_mut();
        state.draft.code = code.to_owned();
        state.draft.name = name.to_owned();
    }
    app.submit_product_form();

    assert!(
        app.product_state().errors.is_empty(),
        "'{name}' kaydedilirken alan hatası oluşmamalı: {:?}",
        app.product_state().errors
    );
    assert!(
        app.product_state().general_error.is_none(),
        "'{name}' için genel hata oluşmamalı"
    );

    app.product_state()
        .detail_id
        .unwrap_or_else(|| panic!("'{name}' kaydedilmeli ve ayrıntısı açılmalı"))
}

/// Ürün ayrıntısı açıkken yeni stok hareketi kaydeder.
fn hareket_ekle(
    app: &mut OnMuhasebeApp,
    movement_type: MovementType,
    quantity: &str,
    adjustment_negative: bool,
) {
    let id = app
        .product_state()
        .detail_id
        .expect("ayrıntı ekranında ürün açık olmalı");
    app.open_movement_form(id);
    {
        let state = app.product_state_mut();
        state.movement_draft.movement_type = movement_type;
        state.movement_draft.quantity = quantity.to_owned();
        state.movement_draft.adjustment_negative = adjustment_negative;
        state.movement_draft.date = "2026-10-18".to_owned();
        state.movement_draft.description = "Test hareketi".to_owned();
    }
    app.submit_movement_form();

    assert!(
        app.product_state().movement_errors.is_empty(),
        "hareket kaydedilirken alan hatası oluşmamalı: {:?}",
        app.product_state().movement_errors
    );
    assert!(
        app.product_state().movement_confirmation.is_none(),
        "bu test hareketleri stoğu eksiye düşürmemeli"
    );
    assert_eq!(
        app.product_state().screen,
        ProductScreen::Detail,
        "başarılı kayıttan sonra ayrıntı ekranına dönülmeli"
    );
}

/// Geçerli bir ürün taslağı üretir.
fn ornek_draft(code: &str, name: &str) -> ProductDraft {
    ProductDraft {
        code: code.to_owned(),
        name: name.to_owned(),
        ..ProductDraft::default()
    }
}

#[test]
fn urun_eklenir_baslangic_stogu_hareket_uretir_ve_yeniden_acilista_korunur() {
    let (_directory, config) = temporary_config();
    let context = Context::default();

    let first_id = {
        let mut app = OnMuhasebeApp::with_config(&context, config.clone());
        app.set_view(View::Inventory);

        assert_eq!(
            app.product_state().list.len(),
            0,
            "başlangıçta ürün bulunmamalı"
        );

        app.open_product_new_form();
        assert_eq!(
            app.product_state().draft.code,
            "URN-0001",
            "yeni kayda ürün kodu önerilmeli"
        );

        {
            let state = app.product_state_mut();
            state.draft.name = "Çelik Vida 4x40".to_owned();
            state.draft.barcode = "8691234567890".to_owned();
            state.draft.unit = Unit::Kilogram;
            state.draft.category = "Bağlantı Elemanları".to_owned();
            state.draft.purchase_price = "1.234,56".to_owned();
            state.draft.sale_price = "1.999,00".to_owned();
            state.draft.vat_rate = "20".to_owned();
            state.draft.min_stock = "10".to_owned();
            state.draft.notes = "Raf A1".to_owned();
            state.draft.starting_stock = "12,5".to_owned();
        }

        app.submit_product_form();

        assert!(
            app.product_state().errors.is_empty(),
            "kayıt hatasız olmalı: {:?}",
            app.product_state().errors
        );
        assert_eq!(
            app.product_state().screen,
            ProductScreen::Detail,
            "kayıttan sonra ayrıntı ekranı açılmalı"
        );
        assert!(
            app.product_state().notice.is_some(),
            "kullanıcıya başarı mesajı gösterilmeli"
        );

        let product = app
            .product_state()
            .selected
            .as_ref()
            .expect("seçili ürün bulunmalı");
        assert_eq!(product.code, "URN-0001");
        assert_eq!(product.unit, Unit::Kilogram);
        assert_eq!(product.current_stock.milli(), 12_500);
        assert!(product.active);

        // Başlangıç stoğu için tek bir `İlk Stok` hareketi oluşmalı.
        assert_eq!(app.product_state().movements.len(), 1);
        assert_eq!(
            app.product_state().movements[0].movement_type,
            MovementType::Initial
        );

        app.product_state().detail_id.expect("kimlik bulunmalı")
    };

    // Uygulama kapatılıp aynı veritabanı dosyasıyla yeniden açılır.
    let (_context, app) = open_app(&config);

    assert_eq!(app.product_state().list.len(), 1, "kayıt korunmalı");
    assert_eq!(app.product_state().list[0].id, first_id);
    assert_eq!(app.product_state().list[0].current_stock.milli(), 12_500);
}

#[test]
fn urun_duzenlenir_stok_degismez_ve_kalici_olur() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    let id = {
        app.open_product_new_form();
        {
            let state = app.product_state_mut();
            state.draft.code = "URN-0001".to_owned();
            state.draft.name = "Eski Ad".to_owned();
            state.draft.starting_stock = "20".to_owned();
        }
        app.submit_product_form();
        app.product_state().detail_id.expect("kayıt oluşmalı")
    };

    app.open_product_edit_form(id);
    assert_eq!(
        app.product_state().screen,
        ProductScreen::Form,
        "düzenleme formu açılmalı"
    );
    assert_eq!(
        app.product_state().draft.name,
        "Eski Ad",
        "form mevcut değerlerle doldurulmalı"
    );

    {
        let state = app.product_state_mut();
        state.draft.name = "Yeni Ad".to_owned();
        state.draft.unit = Unit::Litre;
        state.draft.sale_price = "25".to_owned();
    }
    app.submit_product_form();

    assert!(app.product_state().errors.is_empty());
    let product = app
        .product_state()
        .selected
        .as_ref()
        .expect("seçili ürün bulunmalı");
    assert_eq!(product.name, "Yeni Ad");
    assert_eq!(product.unit, Unit::Litre);
    assert_eq!(product.sale_price.kurus(), 2500);
    assert_eq!(
        product.current_stock.milli(),
        20_000,
        "düzenleme stoğu değiştirmemeli"
    );

    let (_context, app) = open_app(&config);
    assert_eq!(app.product_state().list[0].name, "Yeni Ad");
}

#[test]
fn ayni_kod_buyuk_kucuk_harf_duyarsiz_reddedilir_ve_girdi_korunur() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    form_ile_ekle(&mut app, "URN-0001", "Birinci Ürün");

    app.open_product_new_form();
    {
        let state = app.product_state_mut();
        state.draft.code = "urn-0001".to_owned();
        state.draft.name = "İkinci Ürün".to_owned();
    }
    app.submit_product_form();

    assert_eq!(
        app.product_state().screen,
        ProductScreen::Form,
        "hatalı kayıtta form açık kalmalı"
    );
    assert!(
        app.product_state().errors.get(Field::Code).is_some(),
        "kod çakışması alan hatası olarak gösterilmeli"
    );
    assert_eq!(
        app.product_state().draft.name,
        "İkinci Ürün",
        "girdi korunmalı"
    );
    assert_eq!(app.product_state().list.len(), 1, "yeni kayıt oluşmamalı");
}

#[test]
fn ayni_barkod_reddedilir_bos_barkod_serbesttir() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    app.open_product_new_form();
    {
        let state = app.product_state_mut();
        state.draft.code = "URN-0001".to_owned();
        state.draft.name = "Birinci".to_owned();
        state.draft.barcode = "8691234567890".to_owned();
    }
    app.submit_product_form();
    assert!(app.product_state().errors.is_empty());

    app.open_product_new_form();
    {
        let state = app.product_state_mut();
        state.draft.code = "URN-0002".to_owned();
        state.draft.name = "İkinci".to_owned();
        state.draft.barcode = "8691234567890".to_owned();
    }
    app.submit_product_form();

    assert!(
        app.product_state().errors.get(Field::Barcode).is_some(),
        "barkod çakışması alan hatası olmalı"
    );
    assert_eq!(app.product_state().screen, ProductScreen::Form);

    // Barkodsuz iki ürün sorunsuz eklenebilmeli.
    form_ile_ekle(&mut app, "URN-0003", "Üçüncü");
    form_ile_ekle(&mut app, "URN-0004", "Dördüncü");
    assert_eq!(app.product_state().list.len(), 3);
}

#[test]
fn zorunlu_alan_ve_tutar_miktar_dogrulamasi_girdiyi_korur() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    app.open_product_new_form();
    {
        let state = app.product_state_mut();
        // Kod ve ad boş; fiyat geçersiz; miktar negatif.
        state.draft.code = String::new();
        state.draft.purchase_price = "abc".to_owned();
        state.draft.sale_price = "-5".to_owned();
        state.draft.min_stock = "-1".to_owned();
        state.draft.vat_rate = "250".to_owned();
        state.draft.notes = "Notlar korunuyor".to_owned();
    }
    app.submit_product_form();

    let errors = &app.product_state().errors;
    assert_eq!(app.product_state().screen, ProductScreen::Form);
    for field in [
        Field::Code,
        Field::Name,
        Field::PurchasePrice,
        Field::SalePrice,
        Field::MinStock,
        Field::VatRate,
    ] {
        assert!(errors.get(field).is_some(), "{field:?} hatası bekleniyordu");
    }
    assert_eq!(
        app.product_state().draft.notes,
        "Notlar korunuyor",
        "girdi korunmalı"
    );
    assert_eq!(app.product_state().list.len(), 0, "kayıt oluşmamalı");

    // Alan düzeltilince ilgili hata temizlenir ve kayıt başarılı olur.
    {
        let state = app.product_state_mut();
        state.errors.clear(Field::PurchasePrice);
        state.draft.code = "URN-0001".to_owned();
        state.draft.name = "Düzeltilmiş".to_owned();
        state.draft.purchase_price = "12,5".to_owned();
        state.draft.sale_price = "20".to_owned();
        state.draft.min_stock = "5".to_owned();
        state.draft.vat_rate = "20".to_owned();
    }
    app.submit_product_form();

    assert!(app.product_state().errors.is_empty());
    assert_eq!(app.product_state().list.len(), 1);
    let product = app.product_state().selected.as_ref().unwrap();
    assert_eq!(product.purchase_price.kurus(), 1250);
    assert_eq!(product.min_stock.milli(), 5000);
}

#[test]
fn arama_ve_filtreler_turkce_karakterler_ile_calisir() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    // Düşük stoklu ürün.
    app.open_product_new_form();
    {
        let state = app.product_state_mut();
        state.draft.code = "URN-0001".to_owned();
        state.draft.name = "Çelik Vida".to_owned();
        state.draft.barcode = "8691234567890".to_owned();
        state.draft.category = "Bağlantı Elemanları".to_owned();
        state.draft.min_stock = "10".to_owned();
        state.draft.starting_stock = "4".to_owned();
    }
    app.submit_product_form();
    assert!(app.product_state().errors.is_empty());

    // Yeterli stoklu ve pasif ürün.
    app.open_product_new_form();
    {
        let state = app.product_state_mut();
        state.draft.code = "URN-0002".to_owned();
        state.draft.name = "Bakır Kablo".to_owned();
        state.draft.min_stock = "10".to_owned();
        state.draft.starting_stock = "50".to_owned();
        state.draft.active = false;
    }
    app.submit_product_form();
    assert!(app.product_state().errors.is_empty());

    app.open_product_list();
    assert_eq!(app.product_state().list.len(), 2, "tüm kayıtlar görünmeli");

    // Türkçe karakterden etkilenmeyen arama.
    for needle in [
        "celik",
        "ÇELİK",
        "çelik",
        "8691234567890",
        "baglanti",
        "BAĞLANTI",
    ] {
        app.product_state_mut().filter = ProductFilter {
            search: needle.to_owned(),
            ..ProductFilter::default()
        };
        app.refresh_product_list();
        assert_eq!(
            app.product_state().list.len(),
            1,
            "'{needle}' tek kayıt bulmalı"
        );
        assert_eq!(app.product_state().list[0].code, "URN-0001");
    }

    // Durum filtresi.
    app.product_state_mut().filter = ProductFilter {
        active: Some(true),
        ..ProductFilter::default()
    };
    app.refresh_product_list();
    assert_eq!(app.product_state().list.len(), 1);
    assert_eq!(app.product_state().list[0].code, "URN-0001");

    // Düşük stok filtresi (Genel Bakış'taki bağlantının kullandığı yol).
    app.reset_product_filter();
    app.show_low_stock_products();
    assert_eq!(app.view(), View::Inventory);
    assert_eq!(app.product_state().list.len(), 1);
    assert_eq!(app.product_state().list[0].code, "URN-0001");

    // Bulunmayan arama boş liste döndürür.
    app.product_state_mut().filter = ProductFilter {
        search: "bulunmayan".to_owned(),
        ..ProductFilter::default()
    };
    app.refresh_product_list();
    assert!(app.product_state().list.is_empty());
}

#[test]
fn aktif_pasif_durumu_kaydi_silmeden_degisir() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    let id = form_ile_ekle(&mut app, "URN-0001", "Deneme Ürünü");

    app.toggle_product_active(id);
    assert!(
        !app.product_state().selected.as_ref().unwrap().active,
        "kayıt pasife alınmalı"
    );
    assert_eq!(app.product_state().list.len(), 1, "kayıt silinmemeli");

    app.toggle_product_active(id);
    assert!(
        app.product_state().selected.as_ref().unwrap().active,
        "kayıt yeniden aktif olmalı"
    );
}

#[test]
fn ardisik_hareketler_stok_toplamiyla_uyusur_ve_kalici_olur() {
    let (_directory, config) = temporary_config();
    let context = Context::default();

    let id = {
        let mut app = OnMuhasebeApp::with_config(&context, config.clone());
        app.set_view(View::Inventory);

        let id = form_ile_ekle(&mut app, "URN-0001", "Vida");

        hareket_ekle(&mut app, MovementType::In, "10", false);
        hareket_ekle(&mut app, MovementType::Out, "4", false);
        // Düzeltme: eksi yönlü (sayım farkı).
        hareket_ekle(&mut app, MovementType::Adjust, "2,5", true);

        let product = app.product_state().selected.as_ref().unwrap();
        assert_eq!(product.current_stock.milli(), 10_000 - 4_000 - 2_500);
        assert_eq!(app.product_state().movements.len(), 3);
        id
    };

    // Veritabanı düzeyinde current_stock == SUM(quantity_delta) doğrulanır.
    let database = Database::open(config.database_path()).expect("veritabanı açılmalı");
    let movements = database
        .movement_list_for_product(id)
        .expect("hareketler okunmalı");
    let sum: i64 = movements
        .iter()
        .map(|movement| movement.quantity_delta.milli())
        .sum();

    let product = database.product_get(id).unwrap().unwrap();
    assert_eq!(
        product.current_stock.milli(),
        sum,
        "güncel stok hareketlerin toplamına eşit olmalı"
    );

    drop(database);

    // Yeniden açılışta stok ve hareketler korunur.
    let (_context, mut app) = open_app(&config);
    assert_eq!(app.product_state().list[0].current_stock.milli(), 3_500);
    app.open_product_detail(id);
    assert_eq!(app.product_state().movements.len(), 3);
}

#[test]
fn duzeltme_hareketi_aciklama_gerektirir() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    let id = form_ile_ekle(&mut app, "URN-0001", "Vida");
    app.open_movement_form(id);
    {
        let state = app.product_state_mut();
        state.movement_draft.movement_type = MovementType::Adjust;
        state.movement_draft.quantity = "5".to_owned();
        state.movement_draft.description = String::new();
    }
    app.submit_movement_form();

    assert!(
        app.product_state()
            .movement_errors
            .get(Field::MovementDescription)
            .is_some(),
        "düzeltmede açıklama zorunlu olmalı"
    );
    assert_eq!(
        app.product_state().movements.len(),
        0,
        "hareket yazılmamalı"
    );
}

#[test]
fn negatif_stok_onay_ister_vazgecince_kaydedilmez() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    let id = form_ile_ekle(&mut app, "URN-0001", "Vida");
    hareket_ekle(&mut app, MovementType::In, "2", false);

    // 10 çıkış stoğu eksiye düşürür; onay istenmeli.
    app.open_movement_form(id);
    {
        let state = app.product_state_mut();
        state.movement_draft.movement_type = MovementType::Out;
        state.movement_draft.quantity = "10".to_owned();
        state.movement_draft.date = "2026-10-18".to_owned();
    }
    app.submit_movement_form();

    assert_eq!(
        app.product_state().screen,
        ProductScreen::MovementForm,
        "onay beklenirken formda kalınmalı"
    );
    assert_eq!(
        app.product_state().movement_confirmation,
        Some(Quantity::from_milli(-8000)),
        "yeni stok onay kartında gösterilmeli"
    );
    assert_eq!(
        app.product_state().movements.len(),
        1,
        "onay verilmeden hareket yazılmamalı"
    );

    // Vazgeç: hareket kaydedilmez.
    app.cancel_pending_movement();
    assert!(app.product_state().movement_confirmation.is_none());
    assert_eq!(app.product_state().movements.len(), 1);

    // Yeniden dene ve onayla: hareket kaydedilir, stok eksiye düşer.
    app.submit_movement_form();
    assert!(app.product_state().movement_confirmation.is_some());
    app.confirm_pending_movement();

    assert_eq!(app.product_state().screen, ProductScreen::Detail);
    assert_eq!(app.product_state().movements.len(), 2);
    assert_eq!(
        app.product_state()
            .selected
            .as_ref()
            .unwrap()
            .current_stock
            .milli(),
        -8000,
        "onay sonrası stok eksiye düşmeli"
    );
}

#[test]
fn genel_bakis_ozeti_gercek_kayitlari_okur() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    let id = form_ile_ekle(&mut app, "URN-0001", "Eksik Stoklu Ürün");
    app.open_product_edit_form(id);
    {
        let state = app.product_state_mut();
        state.draft.min_stock = "10".to_owned();
        state.draft.starting_stock.clear();
    }
    app.submit_product_form();
    hareket_ekle(&mut app, MovementType::In, "3", false);

    form_ile_ekle(&mut app, "URN-0002", "Yeterli Stoklu Ürün");

    let overview = app.overview_metrics();
    assert_eq!(overview.active_products, 2);
    assert_eq!(overview.low_stock_products, 1);
    assert_eq!(overview.customers, 0, "cari kayıt yokken 0 olmalı");
    assert!(
        !overview.recent_movements.is_empty(),
        "son stok hareketleri listelenmeli"
    );
}

#[test]
fn migration_urun_tablolarini_tek_seferde_olusturur() {
    let (_directory, config) = temporary_config();

    let database = Database::open(config.database_path()).expect("veritabanı açılmalı");
    let status = database.status().expect("durum okunmalı");

    assert_eq!(
        latest_version(),
        7,
        "kullanıcılar ve ayarlar migration'ı eklenmiş olmalı"
    );
    assert_eq!(status.schema_version, 7);
    assert_eq!(
        status.applied_migrations.len(),
        MIGRATIONS.len(),
        "her migration bir kez uygulanmalı"
    );
    assert_eq!(database.product_count().unwrap(), 0);
    assert_eq!(database.low_stock_count().unwrap(), 0);

    let validated = ornek_draft("URN-0001", "Kalıcı Ürün")
        .validate()
        .expect("örnek veri geçerli olmalı");
    let id = database
        .product_insert(&validated)
        .expect("kayıt eklenebilmeli");
    drop(database);

    let database = Database::open(config.database_path()).expect("yeniden açılmalı");
    let status = database.status().expect("durum okunmalı");
    assert_eq!(
        status.applied_migrations.len(),
        MIGRATIONS.len(),
        "yeniden açılışta migration tekrar uygulanmamalı"
    );
    assert_eq!(status.schema_version, 7);
    assert_eq!(
        database.product_get(id).unwrap().unwrap().name,
        "Kalıcı Ürün",
        "veri korunmalı"
    );
}

#[test]
fn veritabani_katmani_kod_ve_barkod_cakismasini_bildirir() {
    let (_directory, config) = temporary_config();
    let database = Database::open(config.database_path()).expect("veritabanı açılmalı");

    let mut first = ornek_draft("URN-0001", "Birinci");
    first.barcode = "8691234567890".to_owned();
    let first = first.validate().unwrap();
    database.product_insert(&first).expect("ilk kayıt");

    let second = ornek_draft("urn-0001", "İkinci").validate().unwrap();
    let error = database
        .product_insert(&second)
        .expect_err("kod çakışması bekleniyordu");
    assert!(matches!(error, AppError::ProductCodeTaken(_)));

    let mut third = ornek_draft("URN-0002", "Üçüncü");
    third.barcode = "8691234567890".to_owned();
    let third = third.validate().unwrap();
    let error = database
        .product_insert(&third)
        .expect_err("barkod çakışması bekleniyordu");
    assert!(matches!(error, AppError::ProductBarcodeTaken(_)));
}
