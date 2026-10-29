// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! CSV içe/dışa aktarma, yedekleme ve ekranlar arası bağlantı testleri.

use std::path::{Path, PathBuf};

use tempfile::TempDir;

use crate::app::{OnMuhasebeApp, View};
use crate::cash::CashAccountDraft;
use crate::config::AppConfig;
use crate::customer::{CustomerDraft, CustomerFilter};
use crate::income_expense::EntryType;
use crate::invoice::InvoiceFilter;
use crate::payment::PaymentType;
use crate::product::{ProductDraft, ProductFilter};
use crate::report::ReportKind;

fn new_app() -> (TempDir, OnMuhasebeApp) {
    let directory = tempfile::tempdir().expect("geçici dizin");
    let config = AppConfig::with_data_dir(directory.path().join("veri"));
    let context = egui::Context::default();
    let app = OnMuhasebeApp::with_config(&context, config);
    (directory, app)
}

fn add_customer(app: &mut OnMuhasebeApp, code: &str, name: &str, active: bool) -> i64 {
    let draft = CustomerDraft {
        code: code.to_owned(),
        name: name.to_owned(),
        active,
        ..CustomerDraft::default()
    };
    let customer = draft.validate().expect("cari doğrulanmalı");
    let id = app
        .database
        .as_ref()
        .unwrap()
        .customer_insert(&customer)
        .unwrap();
    app.refresh_customer_list();
    id
}

fn add_product(app: &mut OnMuhasebeApp, code: &str, name: &str, barcode: &str) -> i64 {
    let draft = ProductDraft {
        code: code.to_owned(),
        barcode: barcode.to_owned(),
        name: name.to_owned(),
        unit: crate::product::Unit::Kilogram,
        purchase_price: "10,00".to_owned(),
        sale_price: "20,00".to_owned(),
        min_stock: "1".to_owned(),
        starting_stock: "10".to_owned(),
        ..ProductDraft::default()
    };
    let product = draft.validate().expect("ürün doğrulanmalı");
    let id = app
        .database
        .as_ref()
        .unwrap()
        .product_insert(&product)
        .unwrap();
    app.refresh_product_list();
    id
}

fn add_cash_account(app: &mut OnMuhasebeApp, name: &str) -> i64 {
    let draft = CashAccountDraft {
        name: name.to_owned(),
        opening_balance: "100,00".to_owned(),
        ..CashAccountDraft::default()
    };
    let validated = draft.validate().expect("kasa hesabı doğrulanmalı");
    let id = app
        .database
        .as_ref()
        .unwrap()
        .cash_account_insert(&validated)
        .unwrap();
    app.refresh_cash_account_list();
    id
}

fn add_sales_invoice(app: &mut OnMuhasebeApp, customer_id: i64, product_id: i64) -> (i64, String) {
    app.set_view(View::SalesInvoices);
    app.open_invoice_new_form();
    {
        let state = app.invoice_state_mut();
        state.draft.customer_id = Some(customer_id);
        state.draft.lines[0].product_id = Some(product_id);
        state.draft.lines[0].quantity = "2".to_owned();
        state.draft.lines[0].unit_price = "100,00".to_owned();
        state.draft.lines[0].vat_rate = "20".to_owned();
    }
    app.submit_invoice_form();
    assert!(app.invoice_state().pending.is_some(), "onay beklenmeli");
    app.confirm_invoice_save();
    let invoice = app
        .invoice_state()
        .selected
        .clone()
        .expect("fatura kaydedilmeli");
    (invoice.id, invoice.number)
}

fn add_payment(app: &mut OnMuhasebeApp, customer_id: i64) {
    app.open_payment_form_for_customer(customer_id, PaymentType::Tahsilat);
    app.cash_state_mut().payment_draft.amount = "50,00".to_owned();
    app.submit_payment_form();
    assert!(
        app.cash_state().payment_pending.is_some(),
        "tahsilat onayı beklenmeli"
    );
    app.confirm_payment_save();
}

fn add_category(app: &mut OnMuhasebeApp, name: &str, entry_type: EntryType) -> i64 {
    app.open_income_expense_category_new_form();
    {
        let state = app.income_expense_state_mut();
        state.category_draft.name = name.to_owned();
        state.category_draft.category_type = entry_type;
    }
    app.submit_income_expense_category_form();
    app.income_expense_state()
        .categories
        .iter()
        .find(|category| category.name == name && category.category_type == entry_type)
        .map(|category| category.id)
        .expect("kategori kaydedilmeli")
}

fn add_income(app: &mut OnMuhasebeApp, category_id: i64) {
    app.open_income_expense_entry_form(EntryType::Gelir);
    app.set_income_expense_category(Some(category_id));
    {
        let state = app.income_expense_state_mut();
        state.entry_draft.amount = "25,00".to_owned();
        state.entry_draft.description = "Test geliri".to_owned();
    }
    app.submit_income_expense_entry_form();
    assert!(
        app.income_expense_state().entry_pending.is_some(),
        "gelir onayı beklenmeli"
    );
    app.confirm_income_expense_entry_save();
}

fn csv_text(path: &Path) -> String {
    let bytes = std::fs::read(path).expect("CSV okunmalı");
    assert!(
        bytes.starts_with(&[0xEF, 0xBB, 0xBF]),
        "CSV UTF-8 BOM ile başlamalı"
    );
    String::from_utf8(bytes).expect("CSV UTF-8 olmalı")
}

fn last_line_contains(text: &str, needle: &str) -> bool {
    text.lines().any(|line| line.contains(needle))
}

fn seed_business(app: &mut OnMuhasebeApp) -> (i64, i64, i64, String) {
    let customer_id = add_customer(app, "CAR-1", "Örnek Müşteri", true);
    let product_id = add_product(app, "URN-1", "Örnek Ürün", "869000000001");
    add_cash_account(app, "Merkez Kasa");
    let (invoice_id, invoice_number) = add_sales_invoice(app, customer_id, product_id);
    add_payment(app, customer_id);
    let category_id = add_category(app, "Danışmanlık", EntryType::Gelir);
    add_income(app, category_id);
    (customer_id, product_id, invoice_id, invoice_number)
}

#[test]
fn csv_sablonlari_tam_basliklariyla_yazilir() {
    let (_directory, mut app) = new_app();
    let temp = tempfile::tempdir().unwrap();

    let customer_path = temp.path().join("cari.csv");
    app.write_customer_template(&customer_path).unwrap();
    let customer = csv_text(&customer_path);
    assert_eq!(
        customer.strip_prefix('\u{feff}').unwrap().trim_end(),
        "code;name;kind;phone;email;address;tax_office;tax_number;national_id;\
         contact_person;notes;active"
            .replace(' ', "")
    );

    let product_path = temp.path().join("urun.csv");
    app.write_product_template(&product_path).unwrap();
    let product = csv_text(&product_path);
    assert_eq!(
        product.strip_prefix('\u{feff}').unwrap().trim_end(),
        "code;barcode;name;unit;category;purchase_price;sale_price;vat_rate;\
         min_stock;notes;active"
            .replace(' ', "")
    );
}

#[test]
fn cari_listesi_urun_listesi_fatura_ve_hareket_csv_icerikleri_dogrudur() {
    let (_directory, mut app) = new_app();
    let (_customer_id, _product_id, _invoice_id, invoice_number) = seed_business(&mut app);
    let temp = tempfile::tempdir().unwrap();

    let path = temp.path().join("cari.csv");
    app.export_customer_list_csv(&path).unwrap();
    let text = csv_text(&path);
    assert!(last_line_contains(&text, "Cari Kodu;Cari Adı"));
    assert!(last_line_contains(&text, "CAR-1;Örnek Müşteri"));

    let path = temp.path().join("urun.csv");
    app.export_product_list_csv(&path).unwrap();
    let text = csv_text(&path);
    assert!(last_line_contains(&text, "Ürün Kodu;Barkod;Ürün Adı"));
    assert!(last_line_contains(&text, "URN-1;869000000001;Örnek Ürün"));
    assert!(text.contains("Kilogram (kg)"));
    assert!(last_line_contains(&text, ";8;;Aktif"));

    app.set_view(View::SalesInvoices);
    app.refresh_invoice_list();
    let path = temp.path().join("fatura.csv");
    app.export_invoice_list_csv(&path).unwrap();
    let text = csv_text(&path);
    assert!(last_line_contains(&text, "Fatura No;Tür;Tarih"));
    assert!(text.contains(&invoice_number));

    let path = temp.path().join("tahsilat.csv");
    app.export_payment_list_csv(&path).unwrap();
    let text = csv_text(&path);
    assert!(last_line_contains(&text, "Tür;Tarih;Cari Kodu"));
    assert!(text.contains("50,00"));

    let path = temp.path().join("gelir_gider.csv");
    app.export_income_expense_list_csv(&path).unwrap();
    let text = csv_text(&path);
    assert!(last_line_contains(&text, "Tür;Tarih;Kategori"));
    assert!(text.contains("Danışmanlık"));

    app.refresh_cash_account_list();
    let path = temp.path().join("kasa.csv");
    app.export_cash_account_list_csv(&path).unwrap();
    let text = csv_text(&path);
    assert!(last_line_contains(&text, "Hesap Adı;Tür;Banka"));
    assert!(text.contains("Merkez Kasa"));
}

#[test]
fn tum_rapor_csv_noktalari_baslik_ve_veri_uretir() {
    let (_directory, mut app) = new_app();
    let (customer_id, _product_id, _invoice_id, _invoice_number) = seed_business(&mut app);
    let temp = tempfile::tempdir().unwrap();

    app.enter_reports_section();
    app.set_report_customer(Some(customer_id));

    let path = temp.path().join("ekstre.csv");
    assert_eq!(
        app.report_csv_default_name(),
        format!(
            "cari_ekstre_CAR-1_{}.csv",
            chrono::Local::now().format("%Y%m%d")
        )
    );
    app.export_report_csv(&path).unwrap();
    let text = csv_text(&path);
    assert!(last_line_contains(&text, "Cari Kodu;Cari Adı;Tarih"));
    assert!(text.contains("Örnek Müşteri"));

    app.set_report_tab(ReportKind::CustomerBalances);
    let path = temp.path().join("bakiye.csv");
    app.export_report_csv(&path).unwrap();
    let text = csv_text(&path);
    assert!(last_line_contains(&text, "Cari Kodu;Cari Adı;Tür;Satış"));
    assert!(text.contains("CAR-1"));

    app.set_report_tab(ReportKind::StockStatus);
    let path = temp.path().join("stok.csv");
    app.export_report_csv(&path).unwrap();
    let text = csv_text(&path);
    assert!(last_line_contains(&text, "Ürün Kodu;Ürün Adı;Birim"));
    assert!(text.contains("URN-1"));

    app.set_report_tab(ReportKind::SalesPurchase);
    let path = temp.path().join("satis_cari.csv");
    app.export_report_csv(&path).unwrap();
    let text = csv_text(&path);
    assert!(last_line_contains(&text, "Cari Kodu;Cari Adı;Satış Adedi"));
    assert!(text.contains("CAR-1"));

    app.set_sales_grouping(crate::report::SalesPurchaseGrouping::Product);
    let path = temp.path().join("satis_urun.csv");
    app.export_report_csv(&path).unwrap();
    let text = csv_text(&path);
    assert!(last_line_contains(&text, "Ürün Kodu;Ürün Adı;Birim"));
    assert!(text.contains("URN-1"));

    app.set_report_tab(ReportKind::CashSummary);
    let path = temp.path().join("kasa_rapor.csv");
    app.export_report_csv(&path).unwrap();
    let text = csv_text(&path);
    assert!(last_line_contains(&text, "Kasa/Banka Hesap Dökümü"));
    assert!(last_line_contains(&text, "Tarih;Tür;Hesap"));
    assert!(text.contains("Merkez Kasa"));

    app.set_report_tab(ReportKind::IncomeExpense);
    let path = temp.path().join("gelir_rapor.csv");
    app.export_report_csv(&path).unwrap();
    let text = csv_text(&path);
    assert!(last_line_contains(&text, "Kategoriler"));
    assert!(last_line_contains(&text, "Aylık Trend"));
    assert!(text.contains("Danışmanlık"));
}

fn write_customer_import(path: &Path, contents: &str) {
    std::fs::write(path, format!("\u{feff}{contents}")).unwrap();
}

#[test]
fn cari_ice_aktarma_onizleme_ve_tek_islem_akisi() {
    let (_directory, mut app) = new_app();
    add_customer(&mut app, "CAR-1", "Mevcut Cari", true);
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("cari_ice.csv");
    write_customer_import(
        &path,
        "code;name;kind;phone;email;address;tax_office;tax_number;national_id;\
contact_person;notes;active\r\n\
CAR-2;Yeni Müşteri;Musteri;0212 111 22 33;yeni@ornek.com;Adres;Beşiktaş;1234567890;;;;Evet\r\n\
CAR-3;Pasif Tedarikçi;Tedarikci;;;;;;;;;Hayır\r\n\
CAR-2;Tekrar;Musteri;;;;;;;;;Evet\r\n\
CAR-1;Mevcut Tekrar;Musteri;;;;;;;;;Evet\r\n\
;Adsız;Musteri;;;;;;;;;Evet\r\n",
    );

    app.preview_customer_import(&path).unwrap();
    let preview = app.transfer_state().import_preview.as_ref().unwrap();
    assert_eq!(preview.ready_count(), 2);
    assert_eq!(preview.error_count(), 3);
    assert!(preview.rows.iter().any(|row| {
        row.status
            .reason()
            .is_some_and(|reason| reason == "kod zaten mevcut")
    }));
    assert!(preview.rows.iter().any(|row| {
        row.status
            .reason()
            .is_some_and(|reason| reason == "kod dosyada tekrar ediyor")
    }));

    app.confirm_import().unwrap();
    let result = app.transfer_state().import_result.as_ref().unwrap();
    assert_eq!(result.inserted, 2);
    assert_eq!(result.skipped.len(), 3);

    let customers = app
        .database
        .as_ref()
        .unwrap()
        .customer_list(&CustomerFilter::default())
        .unwrap();
    assert_eq!(customers.len(), 3);
    let imported = customers
        .iter()
        .find(|customer| customer.code == "CAR-3")
        .unwrap();
    assert!(!imported.active, "Hayır pasif olarak aktarılmalı");
}

#[test]
fn urun_ice_aktarma_ondalik_ve_sifir_stok_kurallari() {
    let (_directory, mut app) = new_app();
    add_cash_account(&mut app, "Kasa");
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("urun_ice.csv");
    write_customer_import(
        &path,
        "code;barcode;name;unit;category;purchase_price;sale_price;vat_rate;\
min_stock;notes;active\r\n\
URN-10;1111;Çelik Vida;kg;Hırdavat;1.234,56;1234.56;20;2,5;;Evet\r\n\
URN-11;1111;Tekrar Barkod;adet;Hırdavat;10;20;20;0;;Evet\r\n\
URN-12;2222;Geçersiz Birim;uçak;Hırdavat;10;20;20;0;;Evet\r\n\
;3333;Kodsuz;adet;Hırdavat;10;20;20;0;;Evet\r\n",
    );

    app.preview_product_import(&path).unwrap();
    let preview = app.transfer_state().import_preview.as_ref().unwrap();
    assert_eq!(preview.ready_count(), 1);
    assert_eq!(preview.error_count(), 3);
    assert!(preview.rows.iter().any(|row| {
        row.status
            .reason()
            .is_some_and(|reason| reason == "barkod dosyada tekrar ediyor")
    }));

    app.confirm_import().unwrap();
    assert_eq!(
        app.transfer_state()
            .import_result
            .as_ref()
            .unwrap()
            .inserted,
        1
    );

    let products = app
        .database
        .as_ref()
        .unwrap()
        .product_list(&ProductFilter::default())
        .unwrap();
    let imported = products
        .iter()
        .find(|product| product.code == "URN-10")
        .unwrap();
    assert_eq!(imported.purchase_price.kurus(), 123_456);
    assert_eq!(imported.sale_price.kurus(), 123_456);
    assert_eq!(imported.current_stock, crate::quantity::Quantity::ZERO);
    assert!(imported.active);
    assert!(
        app.database
            .as_ref()
            .unwrap()
            .movement_list_for_product(imported.id)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn yedek_export_bayt_bayt_kopya_ve_geri_yukleme_calisir() {
    let (directory, mut app) = new_app();
    let customer_a = add_customer(&mut app, "CAR-A", "Yedekteki Cari", true);
    let backup = directory.path().join("yedek.onm-backup");
    app.export_backup_now(&backup).unwrap();

    let source = app.config().database_path().to_path_buf();
    assert_eq!(
        std::fs::read(&source).unwrap(),
        std::fs::read(&backup).unwrap(),
        "yedek SQLite dosyasının bayt bayt kopyası olmalı"
    );
    assert!(
        app.transfer_state()
            .notice
            .as_ref()
            .unwrap()
            .message
            .starts_with("Yedek kaydedildi:")
    );

    let customer_b = add_customer(&mut app, "CAR-B", "Sonradan Eklenen", true);
    app.prepare_restore(&backup).unwrap();
    app.confirm_restore().unwrap();

    let customers = app
        .database
        .as_ref()
        .unwrap()
        .customer_list(&CustomerFilter::default())
        .unwrap();
    assert!(customers.iter().any(|customer| customer.id == customer_a));
    assert!(!customers.iter().any(|customer| customer.id == customer_b));
    assert!(app.transfer_state().restore_success);
    assert_eq!(
        app.transfer_state().notice.as_ref().unwrap().message,
        "Yedek geri yüklendi. Uygulama yeniden başlatılacak."
    );
    let safety_exists = std::fs::read_dir(source.parent().unwrap())
        .unwrap()
        .filter_map(Result::ok)
        .any(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .contains("on_muhasebe.db.pre-restore-")
        });
    assert!(safety_exists, "güvenlik yedeği bırakılmalı");
}

#[test]
fn gecersiz_yedek_mevcut_veriye_dokunmaz() {
    let (directory, mut app) = new_app();
    add_customer(&mut app, "CAR-1", "Korunan Cari", true);
    let path = directory.path().join("bos.onm-backup");
    std::fs::write(&path, b"").unwrap();

    let result = app.prepare_restore(&path);
    assert!(result.is_err());
    assert!(app.transfer_state().pending_restore.is_none());
    assert!(
        app.transfer_state()
            .notice
            .as_ref()
            .is_some_and(|notice| notice.kind == crate::app::NoticeKind::Error)
    );

    let customers = app
        .database
        .as_ref()
        .unwrap()
        .customer_list(&CustomerFilter::default())
        .unwrap();
    assert_eq!(customers.len(), 1);
    assert_eq!(customers[0].code, "CAR-1");
}

#[test]
fn yanlis_uzantili_dosya_geri_yuklenmez() {
    let (directory, mut app) = new_app();
    add_customer(&mut app, "CAR-1", "Korunan Cari", true);
    let path = directory.path().join("yedek.txt");
    std::fs::write(&path, b"sqlite degil").unwrap();

    let result = app.prepare_restore(&path);
    assert!(result.is_err());
    assert!(app.transfer_state().pending_restore.is_none());
    assert_eq!(
        app.transfer_state().notice.as_ref().unwrap().message,
        "Yalnızca .onm-backup uzantılı dosyalar geri yüklenebilir."
    );
}

#[test]
fn bozuk_yedek_geri_yuklemede_mevcut_veri_korunur() {
    let (directory, mut app) = new_app();
    add_customer(&mut app, "CAR-1", "Korunan Cari", true);

    let path = directory.path().join("bozuk.onm-backup");
    {
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE app_metadata (
                     key TEXT PRIMARY KEY NOT NULL,
                     value TEXT NOT NULL,
                     updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                 );
                 INSERT INTO app_metadata (key, value) VALUES ('app_name', 'OnMuhasebe');
                 CREATE TABLE schema_migrations (
                     version INTEGER PRIMARY KEY NOT NULL,
                     name TEXT NOT NULL,
                     applied_at TEXT NOT NULL DEFAULT (datetime('now'))
                 );
                 CREATE TABLE customers (id INTEGER PRIMARY KEY);",
            )
            .unwrap();
    }

    let result = app.prepare_restore(&path);
    assert!(
        result.is_err(),
        "boş şema geçmişi olan yedek doğrulamada reddedilmeli"
    );
    assert!(
        app.database.is_some(),
        "mevcut veritabanı yeniden açık kalmalı"
    );
    let customers = app
        .database
        .as_ref()
        .unwrap()
        .customer_list(&CustomerFilter::default())
        .unwrap();
    assert_eq!(customers.len(), 1);
    assert_eq!(customers[0].code, "CAR-1");
    assert!(
        app.transfer_state()
            .notice
            .as_ref()
            .is_some_and(|notice| notice.kind == crate::app::NoticeKind::Error)
    );
}

#[test]
fn ekranlar_arasi_baglantilar_dogru_hedefi_acar() {
    let (_directory, mut app) = new_app();
    let customer_id = add_customer(&mut app, "CAR-1", "Bağlantı Müşterisi", true);
    let product_id = add_product(&mut app, "URN-1", "Bağlantı Ürünü", "869000000009");
    let cash_id = add_cash_account(&mut app, "Bağlantı Kasası");
    let (invoice_id, _number) = add_sales_invoice(&mut app, customer_id, product_id);

    app.show_customer_detail(customer_id);
    assert_eq!(app.view(), View::Customers);
    assert_eq!(app.customer_state().detail_id, Some(customer_id));

    app.show_product_detail(product_id);
    assert_eq!(app.view(), View::Inventory);
    assert_eq!(app.product_state().detail_id, Some(product_id));

    app.show_customer_sales_invoices(customer_id);
    assert_eq!(app.view(), View::SalesInvoices);
    assert_eq!(
        app.invoice_state().filter.customer_id,
        Some(customer_id),
        "fatura listesi cariye filtrelenmeli"
    );
    assert!(
        app.invoice_state()
            .list
            .iter()
            .all(|invoice| invoice.customer_id == customer_id)
    );

    app.show_customer_statement(customer_id);
    assert_eq!(app.view(), View::Reports);
    assert_eq!(app.reports_state().tab, ReportKind::CustomerStatement);
    assert_eq!(app.reports_state().customer_id, Some(customer_id));
    assert!(app.reports_state().customer_statement.is_some());

    app.show_invoice_detail(invoice_id);
    assert_eq!(app.view(), View::SalesInvoices);
    assert_eq!(app.invoice_state().detail_id, Some(invoice_id));
    assert_eq!(
        app.invoice_state().selected.as_ref().unwrap().customer_id,
        customer_id
    );

    app.show_cash_account_highlight(cash_id);
    assert_eq!(app.view(), View::CashAndBank);
    assert_eq!(app.cash_state().tab, crate::app::CashTab::Accounts);
    assert_eq!(app.cash_state().highlight_account_id, Some(cash_id));
}

#[test]
fn varsayilan_dosya_adlari_turkce_ve_tarihlidir() {
    let (_directory, mut app) = new_app();
    let customer_id = add_customer(&mut app, "CAR-9", "Ad", true);
    app.enter_reports_section();
    app.set_report_customer(Some(customer_id));
    let name = app.report_csv_default_name();
    assert!(name.starts_with("cari_ekstre_CAR-9_"));
    assert!(name.ends_with(".csv"));
    assert!(OnMuhasebeApp::default_backup_name().ends_with(".onm-backup"));
    assert_eq!(
        OnMuhasebeApp::backup_path(PathBuf::from("yedek")),
        PathBuf::from("yedek.onm-backup")
    );
}

#[test]
fn cari_filtresi_ile_ice_aktarma_mevcut_kodu_hata_gosterir() {
    let (_directory, mut app) = new_app();
    add_customer(&mut app, "CAR-1", "Mevcut", true);
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("tekrar.csv");
    write_customer_import(
        &path,
        "code;name;kind;phone;email;address;tax_office;tax_number;national_id;\
contact_person;notes;active\r\nCAR-1;Tekrar;Musteri;;;;;;;;;;Evet\r\n",
    );
    app.preview_customer_import(&path).unwrap();
    let preview = app.transfer_state().import_preview.as_ref().unwrap();
    assert_eq!(preview.ready_count(), 0);
    assert_eq!(preview.rows[0].status.reason(), Some("kod zaten mevcut"));
    assert!(app.confirm_import().is_err());
    assert!(
        app.database
            .as_ref()
            .unwrap()
            .customer_list(&CustomerFilter::default())
            .unwrap()
            .iter()
            .all(|customer| customer.code != "TEKRAR")
    );
}

#[test]
fn fatura_filtresi_cari_baglantisinda_temizlenir() {
    let (_directory, mut app) = new_app();
    let customer_id = add_customer(&mut app, "CAR-1", "Ad", true);
    app.set_view(View::SalesInvoices);
    app.invoice_state_mut().filter = InvoiceFilter {
        search: "eski".to_owned(),
        status: Some(crate::invoice::InvoiceStatus::Confirmed),
        ..InvoiceFilter::default()
    };
    app.show_customer_sales_invoices(customer_id);
    assert_eq!(
        app.invoice_state().filter,
        InvoiceFilter {
            customer_id: Some(customer_id),
            ..InvoiceFilter::default()
        }
    );
}
