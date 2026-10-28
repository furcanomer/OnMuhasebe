// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Cari hesap modülünün uçtan uca (entegrasyon) testleri.
//!
//! Bu testler gerçek bir veritabanı dosyası ve gerçek uygulama durumu
//! (`OnMuhasebeApp`) üzerinden çalışır. Böylece yalnızca birimlerin değil,
//! katmanların birlikte doğru çalıştığı doğrulanır: doğrulama, kalıcılık,
//! yeniden açılış, arama/filtreleme ve hata mesajları.

use egui::Context;
use on_muhasebe::app::{CustomerScreen, OnMuhasebeApp, View};
use on_muhasebe::config::AppConfig;
use on_muhasebe::customer::{CustomerDraft, CustomerFilter, CustomerType, Field};
use on_muhasebe::db::{Database, MIGRATIONS, latest_version};
use on_muhasebe::error::AppError;
use tempfile::TempDir;

/// Geçici veri dizini ve bu dizine bağlı yapılandırma üretir.
fn temporary_config() -> (TempDir, AppConfig) {
    let directory = tempfile::tempdir().expect("geçici dizin oluşturulmalı");
    let config = AppConfig::with_data_dir(directory.path().join("veri"));
    (directory, config)
}

/// Yapılandırmaya bağlı, cari hesap bölümü açılmış uygulama durumu üretir.
fn open_app(config: &AppConfig) -> (Context, OnMuhasebeApp) {
    let context = Context::default();
    let mut app = OnMuhasebeApp::with_config(&context, config.clone());
    app.set_view(View::Customers);
    (context, app)
}

/// Form üzerinden örnek bir cari hesap ekler.
fn form_ile_ekle(app: &mut OnMuhasebeApp, code: &str, name: &str, kind: CustomerType) -> i64 {
    app.open_customer_new_form();
    {
        let state = app.customer_state_mut();
        state.draft.code = code.to_owned();
        state.draft.name = name.to_owned();
        state.draft.kind = kind;
    }
    app.submit_customer_form();

    assert!(
        app.customer_state().errors.is_empty(),
        "'{name}' kaydedilirken alan hatası oluşmamalı"
    );

    app.customer_state()
        .detail_id
        .unwrap_or_else(|| panic!("'{name}' kaydedilmeli ve ayrıntısı açılmalı"))
}

/// Doğrulamadan geçmiş örnek kayıt üretir.
fn validated(code: &str, name: &str) -> on_muhasebe::customer::ValidatedCustomer {
    CustomerDraft {
        code: code.to_owned(),
        name: name.to_owned(),
        ..CustomerDraft::default()
    }
    .validate()
    .expect("örnek veri geçerli olmalı")
}

#[test]
fn cari_hesap_eklenir_ve_uygulama_yeniden_acildiginda_korunur() {
    let (_directory, config) = temporary_config();
    let context = Context::default();

    {
        let mut app = OnMuhasebeApp::with_config(&context, config.clone());
        app.set_view(View::Customers);

        assert_eq!(
            app.customer_state().list.len(),
            0,
            "başlangıçta cari hesap bulunmamalı"
        );

        app.open_customer_new_form();
        assert_eq!(
            app.customer_state().draft.code,
            "CAR-0001",
            "yeni kayda cari kodu önerilmeli"
        );

        {
            let state = app.customer_state_mut();
            state.draft.name = "Öztürk İnşaat Ltd. Şti.".to_owned();
            state.draft.kind = CustomerType::Both;
            state.draft.phone = "0212 555 12 34".to_owned();
            state.draft.email = "info@ozturk.com".to_owned();
            state.draft.address = "Ankara".to_owned();
            state.draft.tax_office = "Çankaya".to_owned();
            state.draft.tax_number = "1234567890".to_owned();
            state.draft.contact_person = "Ayşe Öztürk".to_owned();
            state.draft.notes = "Aylık hesap".to_owned();
        }

        app.submit_customer_form();

        assert!(
            app.customer_state().errors.is_empty(),
            "kayıt hatasız olmalı"
        );
        assert!(app.customer_state().general_error.is_none());
        assert_eq!(
            app.customer_state().screen,
            CustomerScreen::Detail,
            "kayıttan sonra ayrıntı ekranı açılmalı"
        );
        assert!(
            app.customer_state().notice.is_some(),
            "kullanıcıya başarı mesajı gösterilmeli"
        );
        assert_eq!(app.customer_state().list.len(), 1);
    }

    // Uygulama kapatılıp aynı veritabanı dosyasıyla yeniden açılır.
    let (_context, app) = open_app(&config);

    let list = &app.customer_state().list;
    assert_eq!(list.len(), 1, "kayıt yeniden açılışta korunmalı");

    let customer = &list[0];
    assert_eq!(customer.code, "CAR-0001");
    assert_eq!(customer.name, "Öztürk İnşaat Ltd. Şti.");
    assert_eq!(customer.kind, CustomerType::Both);
    assert_eq!(
        customer.phone.as_deref(),
        Some("0212 555 12 34"),
        "telefon metin olarak ve baştaki sıfır korunarak saklanmalı"
    );
    assert_eq!(customer.email.as_deref(), Some("info@ozturk.com"));
    assert_eq!(customer.tax_office.as_deref(), Some("Çankaya"));
    assert_eq!(customer.tax_number.as_deref(), Some("1234567890"));
    assert_eq!(customer.contact_person.as_deref(), Some("Ayşe Öztürk"));
    assert_eq!(customer.notes.as_deref(), Some("Aylık hesap"));
    assert!(customer.active, "yeni kayıt varsayılan olarak aktif olmalı");
    assert_eq!(
        customer.balance().format_with_symbol(),
        "0,00 ₺",
        "başlangıç bakiyesi sıfır olmalı"
    );
    assert!(!customer.has_movements());
}

#[test]
fn cari_hesap_duzenlenir_ve_degisiklik_kalicidir() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    let id = form_ile_ekle(&mut app, "CAR-0001", "Eski Unvan", CustomerType::Customer);

    app.open_customer_edit_form(id);
    let draft = &app.customer_state().draft;
    assert_eq!(draft.code, "CAR-0001", "düzenlemede mevcut veri gelmeli");
    assert_eq!(draft.name, "Eski Unvan");

    {
        let state = app.customer_state_mut();
        state.draft.name = "Yeni Unvan A.Ş.".to_owned();
        state.draft.kind = CustomerType::Supplier;
        state.draft.email = "yeni@unvan.com".to_owned();
    }
    app.submit_customer_form();

    assert!(app.customer_state().errors.is_empty());
    let customer = app
        .customer_state()
        .selected
        .clone()
        .expect("ayrıntı ekranında kayıt olmalı");
    assert_eq!(customer.id, id, "güncelleme yeni kayıt oluşturmamalı");
    assert_eq!(customer.name, "Yeni Unvan A.Ş.");
    assert_eq!(customer.kind, CustomerType::Supplier);
    assert_eq!(customer.email.as_deref(), Some("yeni@unvan.com"));
    assert_eq!(app.customer_state().list.len(), 1, "kayıt sayısı artmamalı");

    // Yeniden açılışta da güncel değer okunur.
    let (_context, app) = open_app(&config);
    assert_eq!(app.customer_state().list[0].name, "Yeni Unvan A.Ş.");
}

#[test]
fn ayni_cari_kodu_ile_ikinci_kayit_engellenir_ve_girdi_korunur() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    form_ile_ekle(&mut app, "CAR-0001", "İlk Firma", CustomerType::Customer);

    app.open_customer_new_form();
    {
        let state = app.customer_state_mut();
        state.draft.code = "CAR-0001".to_owned();
        state.draft.name = "İkinci Firma".to_owned();
        state.draft.phone = "0212 000 00 00".to_owned();
    }
    app.submit_customer_form();

    let state = app.customer_state();
    assert_eq!(
        state.screen,
        CustomerScreen::Form,
        "hatalı kayıt formu kapatmamalı"
    );
    let message = state
        .errors
        .get(Field::Code)
        .expect("cari kodu alanında hata gösterilmeli");
    assert!(
        message.contains("CAR-0001"),
        "hata mesajı kodu içermeli: {message}"
    );
    assert_eq!(
        state.draft.name, "İkinci Firma",
        "kullanıcı girdisi kaybolmamalı"
    );
    assert_eq!(state.draft.phone, "0212 000 00 00");
    assert_eq!(state.list.len(), 1, "ikinci kayıt oluşmamalı");

    // Büyük/küçük harf farkı da çakışma sayılır.
    app.customer_state_mut().draft.code = "car-0001".to_owned();
    app.submit_customer_form();
    assert!(
        app.customer_state().errors.get(Field::Code).is_some(),
        "büyük/küçük harf farklı kod da reddedilmeli"
    );
    assert_eq!(app.customer_state().list.len(), 1);
}

#[test]
fn zorunlu_alan_hatasi_kaydi_engeller_ve_girdiyi_korur() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    app.open_customer_new_form();
    {
        let state = app.customer_state_mut();
        state.draft.name = "   ".to_owned();
        state.draft.phone = "0212 555 12 34".to_owned();
        state.draft.email = "bozuk-eposta".to_owned();
    }
    app.submit_customer_form();

    let state = app.customer_state();
    assert!(state.errors.get(Field::Name).is_some(), "ad zorunludur");
    assert!(
        state.errors.get(Field::Email).is_some(),
        "geçersiz e-posta reddedilmeli"
    );
    assert_eq!(state.screen, CustomerScreen::Form);
    assert_eq!(state.draft.phone, "0212 555 12 34");
    assert_eq!(state.draft.email, "bozuk-eposta");
    assert_eq!(state.list.len(), 0, "geçersiz kayıt yazılmamalı");

    // Alan düzeltilince hata kendiliğinden temizlenir (arayüz bu metodu çağırır).
    app.customer_state_mut().draft.name = "Düzeltilmiş Firma".to_owned();
    app.customer_state_mut().errors.clear(Field::Name);
    app.customer_state_mut().draft.email = "info@firma.com".to_owned();
    app.customer_state_mut().errors.clear(Field::Email);
    app.submit_customer_form();

    assert!(app.customer_state().errors.is_empty());
    assert_eq!(
        app.customer_state().list.len(),
        1,
        "düzeltilen kayıt yazılmalı"
    );
}

#[test]
fn arama_ve_filtreleme_uygulama_uzerinden_calisir() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    form_ile_ekle(
        &mut app,
        "CAR-0001",
        "Öztürk İnşaat",
        CustomerType::Customer,
    );
    form_ile_ekle(&mut app, "CAR-0002", "Yılmaz Gıda", CustomerType::Supplier);
    form_ile_ekle(&mut app, "CAR-0003", "Pasif Firma", CustomerType::Customer);

    // Üçüncü kaydı pasife al.
    let id = app.customer_state().list[2].id;
    app.toggle_customer_active(id);

    app.open_customer_list();
    assert_eq!(app.customer_state().list.len(), 3, "önce tüm kayıtlar");

    // Ada göre arama (Türkçe karakter ve büyük/küçük harf duyarsız).
    for needle in ["öztürk", "OZTURK", "insaat"] {
        app.customer_state_mut().filter = CustomerFilter {
            search: needle.to_owned(),
            ..CustomerFilter::default()
        };
        app.refresh_customer_list();

        let list = &app.customer_state().list;
        assert_eq!(list.len(), 1, "'{needle}' araması tek kayıt bulmalı");
        assert_eq!(list[0].code, "CAR-0001");
    }

    // Telefona göre arama sonuç bulur, bulunmayan metin boş liste verir.
    app.customer_state_mut().filter = CustomerFilter {
        search: "CAR-0002".to_owned(),
        ..CustomerFilter::default()
    };
    app.refresh_customer_list();
    assert_eq!(app.customer_state().list.len(), 1);

    app.customer_state_mut().filter = CustomerFilter {
        search: "bulunmayan-kayit".to_owned(),
        ..CustomerFilter::default()
    };
    app.refresh_customer_list();
    assert!(
        app.customer_state().list.is_empty(),
        "sonuç bulunamadığında liste boş olmalı"
    );
    assert!(
        app.customer_state().list_error.is_none(),
        "boş sonuç hata değildir"
    );

    // Türe göre filtreleme.
    app.customer_state_mut().filter = CustomerFilter {
        kind: Some(CustomerType::Supplier),
        ..CustomerFilter::default()
    };
    app.refresh_customer_list();
    assert_eq!(app.customer_state().list.len(), 1);
    assert_eq!(app.customer_state().list[0].code, "CAR-0002");

    // Duruma göre filtreleme.
    app.customer_state_mut().filter = CustomerFilter {
        active: Some(false),
        ..CustomerFilter::default()
    };
    app.refresh_customer_list();
    assert_eq!(app.customer_state().list.len(), 1);
    assert_eq!(app.customer_state().list[0].code, "CAR-0003");

    // Filtreleri temizleme ölçütleri sıfırlar.
    app.reset_customer_filter();
    assert!(app.customer_state().filter.is_empty());
    assert_eq!(app.customer_state().list.len(), 3);
}

#[test]
fn pasife_alma_kaydi_silmez_ve_yeniden_acilista_korunur() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    let id = form_ile_ekle(
        &mut app,
        "CAR-0001",
        "Pasif Olacak Firma",
        CustomerType::Customer,
    );

    app.toggle_customer_active(id);
    let selected = app.customer_state().selected.clone().expect("kayıt");
    assert!(!selected.active, "kayıt pasif olmalı");
    assert_eq!(selected.status_label(), "Pasif");
    assert_eq!(app.customer_state().list.len(), 1, "kayıt silinmemeli");

    // Yeniden açılışta pasif kayıt hâlâ oradadır.
    let (_context, mut app) = open_app(&config);
    let list = &app.customer_state().list;
    assert_eq!(list.len(), 1, "pasif kayıt silinmemeli");
    assert!(!list[0].active);

    // Tekrar aktif yapılabilir.
    app.toggle_customer_active(id);
    let customer = app
        .customer_state()
        .list
        .iter()
        .find(|customer| customer.id == id)
        .cloned()
        .expect("kayıt listede bulunmalı");
    assert!(customer.active, "kayıt yeniden aktif olmalı");
}

#[test]
fn migration_yalnizca_bir_kez_uygulanir_ve_cari_tablosu_olusur() {
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
    assert_eq!(database.customer_count().unwrap(), 0);
    assert_eq!(
        database.product_count().unwrap(),
        0,
        "ürün tablosu da oluşturulmalı"
    );

    let first_id = database
        .customer_insert(&validated("CAR-0001", "Kalıcı Firma"))
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
    assert_eq!(database.customer_count().unwrap(), 1, "veri korunmalı");
    assert_eq!(
        database
            .customer_get(first_id)
            .unwrap()
            .expect("kayıt bulunmalı")
            .name,
        "Kalıcı Firma"
    );
}

#[test]
fn ekran_degisiminde_icerik_bastan_gosterilir() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    // Bölüm değişimi kaydırma sıfırlama isteği üretir ve istek bir kez tüketilir.
    app.set_view(View::Overview);
    assert!(app.take_scroll_reset(), "bölüm değişince istek üretilmeli");
    assert!(
        !app.take_scroll_reset(),
        "istek yalnızca bir kez tüketilmeli"
    );

    // Aynı bölüme yeniden geçmek gereksiz istek üretmez.
    app.set_view(View::Overview);
    assert!(!app.take_scroll_reset());

    app.set_view(View::Customers);
    assert!(
        app.take_scroll_reset(),
        "cari bölümüne geçiş istek üretmeli"
    );
    app.take_scroll_reset();

    // Alt ekran geçişleri de içeriği baştan gösterir.
    app.open_customer_new_form();
    assert!(app.take_scroll_reset(), "form açılınca baştan gösterilmeli");

    let id = form_ile_ekle(
        &mut app,
        "CAR-0001",
        "Kaydırma Firması",
        CustomerType::Customer,
    );
    assert!(
        app.take_scroll_reset(),
        "kaydettikten sonra ayrıntı baştan gösterilmeli"
    );

    app.open_customer_list();
    assert!(
        app.take_scroll_reset(),
        "listeye dönünce baştan gösterilmeli"
    );
    app.take_scroll_reset();

    app.open_customer_detail(id);
    assert!(
        app.take_scroll_reset(),
        "ayrıntı açılınca baştan gösterilmeli"
    );
    app.take_scroll_reset();

    app.open_customer_edit_form(id);
    assert!(
        app.take_scroll_reset(),
        "düzenleme formu baştan gösterilmeli"
    );
    app.take_scroll_reset();

    app.cancel_customer_form();
    assert!(
        app.take_scroll_reset(),
        "formdan çıkınca hedef ekran baştan gösterilmeli"
    );
}

#[test]
fn veritabani_hatalari_anlasilir_mesaja_donusur() {
    let (_directory, config) = temporary_config();
    let database = Database::open(config.database_path()).expect("veritabanı açılmalı");

    // Olmayan kaydı güncelleme.
    let error = database
        .customer_update(999, &validated("CAR-0001", "Olmayan"))
        .expect_err("olmayan kayıt güncellenememeli");
    assert!(matches!(error, AppError::CustomerNotFound(999)));
    assert!(error.to_string().contains("999"));

    // Aynı kod iki kez eklenemez.
    database
        .customer_insert(&validated("CAR-0001", "Birinci"))
        .expect("ilk kayıt eklenmeli");
    let error = database
        .customer_insert(&validated("CAR-0001", "İkinci"))
        .expect_err("aynı kod reddedilmeli");
    assert!(matches!(&error, AppError::CustomerCodeTaken(code) if code == "CAR-0001"));
    assert!(
        error.to_string().contains("zaten kullanılıyor"),
        "mesaj kullanıcıya anlaşılır olmalı: {error}"
    );
}

#[test]
fn form_gecersiz_kaydetme_girisimi_veritabanina_yazmaz() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    // Uygulama üzerinden, doğrulamadan geçmeyen veri.
    app.open_customer_new_form();
    {
        let state = app.customer_state_mut();
        state.draft.name = String::new();
        state.draft.code = "   ".to_owned();
    }
    app.submit_customer_form();
    assert_eq!(app.customer_state().list.len(), 0);
    assert!(app.customer_state().errors.len() >= 2);

    // Doğrudan veritabanı katmanı da doğrulanmamış veri kabul etmez:
    // `customer_insert` yalnızca `ValidatedCustomer` alır.
    assert!(
        app.customer_state().draft.validate().is_err(),
        "geçersiz taslak doğrulanamamalı"
    );

    let database = Database::open(config.database_path()).expect("veritabanı açılmalı");
    assert_eq!(
        database.customer_count().unwrap(),
        0,
        "geçersiz kayıt veritabanına yazılmamalı"
    );
}
