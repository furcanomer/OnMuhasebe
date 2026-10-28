// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Alış/satış fatura modülünün uçtan uca (entegrasyon) testleri.
//!
//! Bu testler gerçek bir veritabanı dosyası ve gerçek uygulama durumu
//! (`OnMuhasebeApp`) üzerinden çalışır. Böylece yalnızca birimlerin değil,
//! katmanların birlikte doğru çalıştığı doğrulanır: doğrulama, kalıcılık,
//! stok hareketleri, cari bakiyesi, iptal (ters kayıt), işlem geri alma,
//! yeniden açılış ve arama/filtreleme.

use std::str::FromStr;

use egui::Context;
use on_muhasebe::app::{InvoiceScreen, OnMuhasebeApp, View};
use on_muhasebe::config::AppConfig;
use on_muhasebe::customer::CustomerType;
use on_muhasebe::db::{Database, MIGRATIONS, latest_version};
use on_muhasebe::invoice::{
    Field, InvoiceFilter, InvoiceStatus, InvoiceTotals, InvoiceType, ValidatedInvoice,
    ValidatedInvoiceLine, calculate_line,
};
use on_muhasebe::money::Money;
use on_muhasebe::product::{MovementType, Unit};
use on_muhasebe::quantity::Quantity;
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
fn musteri_ekle(app: &mut OnMuhasebeApp, code: &str, name: &str) -> i64 {
    app.set_view(View::Customers);
    app.open_customer_new_form();
    {
        let state = app.customer_state_mut();
        state.draft.code = code.to_owned();
        state.draft.name = name.to_owned();
        state.draft.kind = CustomerType::Both;
    }
    app.submit_customer_form();

    assert!(
        app.customer_state().errors.is_empty(),
        "'{name}' kaydedilirken alan hatası oluşmamalı"
    );
    app.customer_state().detail_id.expect("cari hesap oluşmalı")
}

/// Form üzerinden örnek bir ürün ekler (başlangıç stoğuyla).
#[allow(clippy::too_many_arguments)]
fn urun_ekle(
    app: &mut OnMuhasebeApp,
    code: &str,
    name: &str,
    purchase: &str,
    sale: &str,
    vat: &str,
    stock: &str,
) -> i64 {
    app.set_view(View::Inventory);
    app.open_product_new_form();
    {
        let state = app.product_state_mut();
        state.draft.code = code.to_owned();
        state.draft.name = name.to_owned();
        state.draft.purchase_price = purchase.to_owned();
        state.draft.sale_price = sale.to_owned();
        state.draft.vat_rate = vat.to_owned();
        state.draft.starting_stock = stock.to_owned();
    }
    app.submit_product_form();

    assert!(
        app.product_state().errors.is_empty(),
        "'{name}' kaydedilirken alan hatası oluşmamalı: {:?}",
        app.product_state().errors
    );
    app.product_state().detail_id.expect("ürün oluşmalı")
}

/// İlgili fatura bölümünü açar ve yeni fatura formunu gösterir.
fn fatura_formu_ac(app: &mut OnMuhasebeApp, invoice_type: InvoiceType) {
    app.set_view(match invoice_type {
        InvoiceType::Purchase => View::PurchaseInvoices,
        InvoiceType::Sales => View::SalesInvoices,
    });
    app.open_invoice_new_form();
}

/// Formdaki bir satırı doldurur (ürün seçimi varsayılanları tetikler).
#[allow(clippy::too_many_arguments)]
fn satir_doldur(
    app: &mut OnMuhasebeApp,
    index: usize,
    product_id: i64,
    quantity: &str,
    unit_price: &str,
    discount: &str,
    vat: &str,
) {
    while app.invoice_state().draft.lines.len() <= index {
        app.add_invoice_line();
    }

    app.apply_invoice_line_product(index, product_id);

    let state = app.invoice_state_mut();
    let line = &mut state.draft.lines[index];
    line.quantity = quantity.to_owned();
    line.unit_price = unit_price.to_owned();
    line.discount_rate = discount.to_owned();
    line.vat_rate = vat.to_owned();
    state.errors.clear_line_all(index);
}

/// Formu doğrular, kaydetme onayını verir ve oluşan faturanın kimliğini döndürür.
fn fatura_kaydet(app: &mut OnMuhasebeApp) -> i64 {
    app.submit_invoice_form();

    assert!(
        app.invoice_state().errors.is_empty(),
        "doğrulama hatası olmamalı: {:?}",
        app.invoice_state().errors
    );
    assert!(
        app.invoice_state().pending.is_some(),
        "kaydetmeden önce onay istenmeli"
    );

    app.confirm_invoice_save();

    assert!(
        app.invoice_state().general_error.is_none(),
        "kayıtta genel hata olmamalı: {:?}",
        app.invoice_state().general_error
    );
    assert_eq!(
        app.invoice_state().screen,
        InvoiceScreen::Detail,
        "başarılı kayıttan sonra ayrıntı ekranı açılmalı"
    );
    app.invoice_state().detail_id.expect("fatura oluşmalı")
}

/// Veritabanı katmanı için doğrulanmış satır üretir.
fn db_satir(
    product_id: i64,
    code: &str,
    name: &str,
    quantity_milli: i64,
    unit_price_kurus: i64,
    discount: i64,
    vat: i64,
) -> ValidatedInvoiceLine {
    let quantity = Quantity::from_milli(quantity_milli);
    let unit_price = Money::from_kurus(unit_price_kurus);
    let amounts =
        calculate_line(quantity, unit_price, discount, vat).expect("hesap geçerli olmalı");
    ValidatedInvoiceLine {
        product_id,
        product_code: code.to_owned(),
        product_name: name.to_owned(),
        unit: Unit::Adet,
        description: None,
        quantity,
        unit_price,
        discount_rate: discount,
        vat_rate: vat,
        amounts,
    }
}

/// Doğrulanmış fatura üretir (veritabanı katmanı testleri için).
fn db_fatura(
    number: &str,
    invoice_type: InvoiceType,
    customer_id: i64,
    date: &str,
    lines: Vec<ValidatedInvoiceLine>,
) -> ValidatedInvoice {
    let totals = InvoiceTotals::from_lines(&lines).expect("toplamlar hesaplanabilmeli");
    ValidatedInvoice {
        number: number.to_owned(),
        invoice_type,
        customer_id,
        invoice_date: date.to_owned(),
        due_date: None,
        notes: None,
        lines,
        totals,
    }
}

#[test]
fn satis_faturasi_kaydedilir_stok_ve_bakiye_guncellenir_ve_kalici_olur() {
    let (_directory, config) = temporary_config();
    let context = Context::default();

    let (invoice_id, customer_id, product_id) = {
        let mut app = OnMuhasebeApp::with_config(&context, config.clone());

        let customer_id = musteri_ekle(&mut app, "CAR-0001", "Öztürk İnşaat");
        let product_id = urun_ekle(&mut app, "URN-0001", "Çelik Vida", "5", "10", "20", "100");

        fatura_formu_ac(&mut app, InvoiceType::Sales);
        assert_eq!(
            app.invoice_state().draft.number,
            "SF-2026-0001",
            "satış faturası için seri numarası önerilmeli"
        );

        {
            let state = app.invoice_state_mut();
            state.draft.customer_id = Some(customer_id);
            state.draft.invoice_date = "2026-10-18".to_owned();
        }

        // 1. satır: 2 adet × 10,00 ₺, %20 KDV -> 24,00 ₺
        satir_doldur(&mut app, 0, product_id, "2", "10", "0", "20");
        // 2. satır: 1 adet × 100,00 ₺, %10 iskonto, %20 KDV -> 108,00 ₺
        satir_doldur(&mut app, 1, product_id, "1", "100", "10", "20");

        // Canlı toplamlar kaydetmeden önce doğru olmalı.
        let totals = app.invoice_state().draft.preview_totals();
        assert_eq!(
            totals.subtotal.kurus(),
            12_000,
            "brüt toplam 120,00 ₺ olmalı"
        );
        assert_eq!(
            totals.discount_total.kurus(),
            1_000,
            "iskonto 10,00 ₺ olmalı"
        );
        assert_eq!(totals.net_total.kurus(), 11_000, "matrah 110,00 ₺ olmalı");
        assert_eq!(totals.vat_total.kurus(), 2_200, "KDV 22,00 ₺ olmalı");
        assert_eq!(
            totals.grand_total.kurus(),
            13_200,
            "genel toplam 132,00 ₺ olmalı"
        );

        let id = fatura_kaydet(&mut app);

        // Ayrıntıda satırlar görünmeli.
        assert_eq!(app.invoice_state().selected_lines.len(), 2);

        // Cari bakiyesi genel toplam kadar artmalı.
        let customer = app
            .customer_state()
            .list
            .iter()
            .find(|customer| customer.id == customer_id)
            .expect("cari listede bulunmalı");
        assert_eq!(customer.balance().kurus(), 13_200);
        assert!(customer.has_movements());

        // Satış 2 + 1 = 3 adet stok azaltmalı: 100 - 3 = 97.
        let product = app
            .product_state()
            .list
            .iter()
            .find(|product| product.id == product_id)
            .expect("ürün listede bulunmalı");
        assert_eq!(product.current_stock.milli(), 97_000);

        // Aynı ürün birden fazla satırda kullanılabilmeli.
        assert_eq!(app.invoice_state().list.len(), 1);

        (id, customer_id, product_id)
    };

    // Veritabanı düzeyinde kalıcılık ve stok hareketleri doğrulanır.
    let database = Database::open(config.database_path()).expect("veritabanı açılmalı");
    assert_eq!(database.invoice_count().unwrap(), 1);
    let lines = database.invoice_lines(invoice_id).unwrap();
    assert_eq!(lines.len(), 2, "iki satır yazılmalı");
    assert_eq!(lines[0].line_no, 1);
    assert_eq!(lines[1].line_no, 2);
    assert_eq!(lines[0].amounts.total.kurus(), 2400);
    assert_eq!(lines[1].amounts.total.kurus(), 10_800);

    let movements = database.movement_list_for_product(product_id).unwrap();
    let invoice_movements: Vec<_> = movements
        .iter()
        .filter(|movement| movement.movement_type == MovementType::SalesInvoice)
        .collect();
    assert_eq!(invoice_movements.len(), 2, "her satır için bir hareket");
    assert!(
        invoice_movements
            .iter()
            .all(|movement| movement.quantity_delta.milli() < 0),
        "satış faturası hareketleri eksi yönlü olmalı"
    );

    // Güncel stok, hareketlerin toplamına eşit olmalı.
    let sum: i64 = movements
        .iter()
        .map(|movement| movement.quantity_delta.milli())
        .sum();
    let product = database.product_get(product_id).unwrap().unwrap();
    assert_eq!(product.current_stock.milli(), sum);
    assert_eq!(product.current_stock.milli(), 97_000);

    let customer = database.customer_get(customer_id).unwrap().unwrap();
    assert_eq!(customer.balance().kurus(), 13_200);
    drop(database);

    // Yeniden açılışta fatura, satırlar ve stok korunur.
    let (_context, mut app) = open_app(&config);
    app.set_view(View::SalesInvoices);
    assert_eq!(app.invoice_state().list.len(), 1);
    assert_eq!(app.invoice_state().list[0].id, invoice_id);
    app.open_invoice_detail(invoice_id);
    assert_eq!(app.invoice_state().selected_lines.len(), 2);
    assert_eq!(
        app.invoice_state()
            .selected
            .as_ref()
            .unwrap()
            .totals
            .grand_total
            .kurus(),
        13_200
    );
}

#[test]
fn alis_faturasi_stogu_artirir_ve_bakiyeyi_negatife_ceker() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    let customer_id = musteri_ekle(&mut app, "CAR-0001", "Demir Toptan");
    let product_id = urun_ekle(&mut app, "URN-0001", "Demir Çubuk", "5", "9", "20", "10");

    fatura_formu_ac(&mut app, InvoiceType::Purchase);
    assert_eq!(
        app.invoice_state().draft.number,
        "AF-2026-0001",
        "alış faturası için AF- serisi önerilmeli"
    );
    {
        let state = app.invoice_state_mut();
        state.draft.customer_id = Some(customer_id);
        state.draft.invoice_date = "2026-10-18".to_owned();
    }
    satir_doldur(&mut app, 0, product_id, "4", "5", "0", "20");

    // Ürün kartındaki alış fiyatı varsayılan olarak gelmeli.
    assert_eq!(
        Money::from_str(&app.invoice_state().draft.lines[0].unit_price)
            .expect("varsayılan fiyat okunabilmeli")
            .kurus(),
        500,
        "ürün kartındaki alış fiyatı varsayılan olmalı"
    );

    fatura_kaydet(&mut app);

    // Alış stoğu artırmalı: 10 + 4 = 14.
    let product = app
        .product_state()
        .list
        .iter()
        .find(|product| product.id == product_id)
        .unwrap();
    assert_eq!(product.current_stock.milli(), 14_000);

    // Alış faturası bakiyeyi negatife çeker (biz cariye borçluyuz).
    let customer = app
        .customer_state()
        .list
        .iter()
        .find(|customer| customer.id == customer_id)
        .unwrap();
    assert_eq!(customer.balance().kurus(), -2400);
    assert!(customer.balance().is_negative());

    let database = Database::open(config.database_path()).unwrap();
    let movements = database.movement_list_for_product(product_id).unwrap();
    assert!(movements.iter().any(|movement| {
        movement.movement_type == MovementType::PurchaseInvoice
            && movement.quantity_delta.milli() == 4000
    }));
}

#[test]
fn stok_yetersizse_onay_istenir_vazgecince_kaydedilmez() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    let customer_id = musteri_ekle(&mut app, "CAR-0001", "Öztürk İnşaat");
    let product_id = urun_ekle(&mut app, "URN-0001", "Vida", "5", "10", "20", "2");

    fatura_formu_ac(&mut app, InvoiceType::Sales);
    {
        let state = app.invoice_state_mut();
        state.draft.customer_id = Some(customer_id);
    }
    satir_doldur(&mut app, 0, product_id, "10", "10", "0", "20");

    app.submit_invoice_form();
    assert!(app.invoice_state().errors.is_empty());
    assert_eq!(
        app.invoice_state().screen,
        InvoiceScreen::Form,
        "onay beklenirken formda kalınmalı"
    );

    // Stoğu eksiye düşürecek ürün bildirilmeli.
    let pending = app
        .invoice_state()
        .pending
        .clone()
        .expect("onay beklenmeli");
    let shortages = app.invoice_stock_shortages(&pending);
    assert_eq!(shortages.len(), 1, "bir ürün eksiye düşmeli");
    assert_eq!(shortages[0].1.milli(), 2_000);
    assert_eq!(shortages[0].2.milli(), -8_000);

    // Vazgeç: hiçbir kayıt oluşmamalı.
    app.discard_invoice_confirmation();
    assert!(app.invoice_state().pending.is_none());
    let database = Database::open(config.database_path()).unwrap();
    assert_eq!(database.invoice_count().unwrap(), 0);
    drop(database);

    // Yeniden dene ve onayla: fatura yine kaydedilir, stok negatif görünür.
    app.submit_invoice_form();
    assert!(app.invoice_state().pending.is_some());
    app.confirm_invoice_save();
    assert_eq!(app.invoice_state().screen, InvoiceScreen::Detail);
    assert_eq!(
        app.product_state()
            .list
            .iter()
            .find(|product| product.id == product_id)
            .unwrap()
            .current_stock
            .milli(),
        -8_000
    );
}

#[test]
fn fatura_iptali_stok_ve_bakiyeyi_geri_alir() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    let customer_id = musteri_ekle(&mut app, "CAR-0001", "Öztürk İnşaat");
    let product_id = urun_ekle(&mut app, "URN-0001", "Vida", "5", "10", "20", "50");

    fatura_formu_ac(&mut app, InvoiceType::Sales);
    {
        let state = app.invoice_state_mut();
        state.draft.customer_id = Some(customer_id);
        state.draft.invoice_date = "2026-10-18".to_owned();
    }
    satir_doldur(&mut app, 0, product_id, "5", "10", "0", "20");
    let invoice_id = fatura_kaydet(&mut app);

    assert_eq!(
        app.product_state()
            .list
            .iter()
            .find(|product| product.id == product_id)
            .unwrap()
            .current_stock
            .milli(),
        45_000
    );

    // İptal nedenini gir ve iptali onayla.
    app.start_invoice_cancel();
    assert!(app.invoice_state().cancelling);

    // Boş neden reddedilmeli.
    app.submit_invoice_cancel();
    assert!(
        app.invoice_state().cancel_error.is_some(),
        "iptal nedeni zorunlu olmalı"
    );
    assert_eq!(
        app.invoice_state().selected.as_ref().unwrap().id,
        invoice_id
    );

    {
        let state = app.invoice_state_mut();
        state.cancel_reason = "Müşteri vazgeçti.".to_owned();
    }
    app.submit_invoice_cancel();

    assert!(!app.invoice_state().cancelling);
    assert!(app.invoice_state().cancel_error.is_none());
    let invoice = app.invoice_state().selected.as_ref().unwrap();
    assert_eq!(invoice.status, InvoiceStatus::Cancelled);
    assert!(invoice.cancel_reason.is_some());
    assert!(invoice.is_cancelled());

    // Stok geri alınmalı: 45 + 5 = 50.
    assert_eq!(
        app.product_state()
            .list
            .iter()
            .find(|product| product.id == product_id)
            .unwrap()
            .current_stock
            .milli(),
        50_000
    );

    // Bakiye sıfırlanmalı (iptal edilen fatura hesaba katılmaz).
    let customer = app
        .customer_state()
        .list
        .iter()
        .find(|customer| customer.id == customer_id)
        .unwrap();
    assert_eq!(customer.balance().kurus(), 0);
    assert!(
        customer.has_movements(),
        "iptal edilmiş fatura da hareket sayılır"
    );

    // İptal edilen fatura listede kalmalı ama `İptal` durumunda görünmeli.
    app.open_invoice_list();
    assert_eq!(app.invoice_state().list.len(), 1);
    assert!(app.invoice_state().list[0].is_cancelled());

    // Veritabanı: ters yönlü düzeltme hareketi ve `iptal` durumu.
    let database = Database::open(config.database_path()).unwrap();
    let movements = database.movement_list_for_product(product_id).unwrap();
    let reversals: Vec<_> = movements
        .iter()
        .filter(|movement| movement.movement_type == MovementType::Adjust)
        .collect();
    assert_eq!(reversals.len(), 1, "iptal ters yönlü düzeltme üretmeli");
    assert_eq!(reversals[0].quantity_delta.milli(), 5_000);
    assert!(
        reversals[0]
            .description
            .as_deref()
            .unwrap_or_default()
            .contains("Fatura iptali"),
        "iptal açıklaması fatura numarasını içermeli: {:?}",
        reversals[0].description
    );

    let sum: i64 = movements
        .iter()
        .map(|movement| movement.quantity_delta.milli())
        .sum();
    assert_eq!(sum, 50_000, "hareket toplamı başlangıç stoğuna dönmeli");
    let invoice = database.invoice_get(invoice_id).unwrap().unwrap();
    assert_eq!(invoice.status, InvoiceStatus::Cancelled);

    // Aynı fatura ikinci kez iptal edilememeli.
    let error = database
        .invoice_cancel(invoice_id, "tekrar")
        .expect_err("ikinci iptal reddedilmeli");
    assert!(matches!(
        error,
        on_muhasebe::error::AppError::InvoiceAlreadyCancelled(_)
    ));
}

#[test]
fn fatura_numarasi_buyuk_kucuk_harf_duyarsiz_benzersizdir_ve_girdi_korunur() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    let customer_id = musteri_ekle(&mut app, "CAR-0001", "Öztürk İnşaat");
    let product_id = urun_ekle(&mut app, "URN-0001", "Vida", "5", "10", "20", "100");

    fatura_formu_ac(&mut app, InvoiceType::Sales);
    {
        let state = app.invoice_state_mut();
        state.draft.customer_id = Some(customer_id);
    }
    satir_doldur(&mut app, 0, product_id, "1", "10", "0", "20");
    fatura_kaydet(&mut app);

    // Yeni formda bir sonraki numara önerilir.
    app.open_invoice_new_form();
    assert_eq!(
        app.invoice_state().draft.number,
        "SF-2026-0002",
        "kullanılmamış bir numara önerilmeli"
    );

    {
        let state = app.invoice_state_mut();
        state.draft.customer_id = Some(customer_id);
        state.draft.number = "sf-2026-0001".to_owned();
    }
    satir_doldur(&mut app, 0, product_id, "1", "10", "0", "20");
    app.submit_invoice_form();
    assert!(app.invoice_state().pending.is_some());
    app.confirm_invoice_save();

    assert_eq!(
        app.invoice_state().screen,
        InvoiceScreen::Form,
        "hatalı kayıtta form açık kalmalı"
    );
    assert!(
        app.invoice_state().errors.get(Field::Number).is_some(),
        "numara çakışması alan hatası olarak gösterilmeli"
    );
    assert_eq!(
        app.invoice_state().draft.number,
        "sf-2026-0001",
        "girdi korunmalı"
    );

    let database = Database::open(config.database_path()).unwrap();
    assert_eq!(
        database.invoice_count().unwrap(),
        1,
        "yeni kayıt oluşmamalı"
    );
}

#[test]
fn liste_arama_ve_filtreleri_turkce_karakterlerle_calisir() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    let ozturk = musteri_ekle(&mut app, "CAR-0001", "Öztürk İnşaat");
    let yilmaz = musteri_ekle(&mut app, "CAR-0002", "Yılmaz Ticaret");
    let product_id = urun_ekle(&mut app, "URN-0001", "Vida", "5", "10", "20", "500");

    // İki satış faturası; ikincisi iptal edilir.
    for (customer_id, date) in [(ozturk, "2026-10-01"), (yilmaz, "2026-10-05")] {
        fatura_formu_ac(&mut app, InvoiceType::Sales);
        {
            let state = app.invoice_state_mut();
            state.draft.customer_id = Some(customer_id);
            state.draft.invoice_date = date.to_owned();
        }
        satir_doldur(&mut app, 0, product_id, "1", "10", "0", "20");
        fatura_kaydet(&mut app);
    }

    // İkinci faturayı iptal et.
    let yilmaz_invoice = app.invoice_state().detail_id.expect("ayrıntı açık olmalı");
    app.start_invoice_cancel();
    {
        let state = app.invoice_state_mut();
        state.cancel_reason = "Test iptali".to_owned();
    }
    app.submit_invoice_cancel();

    app.set_view(View::SalesInvoices);
    assert_eq!(app.invoice_state().list.len(), 2, "tüm faturalar görünmeli");

    // Türkçe karakterden etkilenmeyen arama.
    for needle in ["ozturk", "ÖZTÜRK", "İnşaat", "inşaat"] {
        app.invoice_state_mut().filter = InvoiceFilter {
            search: needle.to_owned(),
            ..InvoiceFilter::default()
        };
        app.refresh_invoice_list();
        assert_eq!(
            app.invoice_state().list.len(),
            1,
            "'{needle}' tek kayıt bulmalı"
        );
        assert_eq!(app.invoice_state().list[0].customer_name, "Öztürk İnşaat");
    }

    // Numara ile arama.
    app.invoice_state_mut().filter = InvoiceFilter {
        search: "SF-2026-0002".to_owned(),
        ..InvoiceFilter::default()
    };
    app.refresh_invoice_list();
    assert_eq!(app.invoice_state().list.len(), 1);
    assert!(app.invoice_state().list[0].is_cancelled());

    // Durum filtresi.
    app.invoice_state_mut().filter = InvoiceFilter {
        status: Some(InvoiceStatus::Confirmed),
        ..InvoiceFilter::default()
    };
    app.refresh_invoice_list();
    assert_eq!(app.invoice_state().list.len(), 1);
    assert_eq!(app.invoice_state().list[0].customer_name, "Öztürk İnşaat");

    app.invoice_state_mut().filter = InvoiceFilter {
        status: Some(InvoiceStatus::Cancelled),
        ..InvoiceFilter::default()
    };
    app.refresh_invoice_list();
    assert_eq!(app.invoice_state().list.len(), 1);
    assert_eq!(app.invoice_state().list[0].id, yilmaz_invoice);

    // Cari filtresi.
    app.invoice_state_mut().filter = InvoiceFilter {
        customer_id: Some(yilmaz),
        ..InvoiceFilter::default()
    };
    app.refresh_invoice_list();
    assert_eq!(app.invoice_state().list.len(), 1);
    assert_eq!(app.invoice_state().list[0].customer_name, "Yılmaz Ticaret");

    // Tarih aralığı.
    app.invoice_state_mut().filter = InvoiceFilter {
        date_from: "2026-10-04".to_owned(),
        ..InvoiceFilter::default()
    };
    app.refresh_invoice_list();
    assert_eq!(app.invoice_state().list.len(), 1);
    assert_eq!(app.invoice_state().list[0].id, yilmaz_invoice);

    app.invoice_state_mut().filter = InvoiceFilter {
        date_to: "2026-10-02".to_owned(),
        ..InvoiceFilter::default()
    };
    app.refresh_invoice_list();
    assert_eq!(app.invoice_state().list.len(), 1);
    assert_eq!(app.invoice_state().list[0].customer_name, "Öztürk İnşaat");

    // Eşleşmeyen arama.
    app.invoice_state_mut().filter = InvoiceFilter {
        search: "bulunmayan".to_owned(),
        ..InvoiceFilter::default()
    };
    app.refresh_invoice_list();
    assert!(app.invoice_state().list.is_empty());

    // Filtreler temizlenince tüm kayıtlar geri gelir.
    app.reset_invoice_filter();
    assert_eq!(app.invoice_state().list.len(), 2);
}

#[test]
fn tek_islmde_geri_alma_hata_durumunda_hicbir_kayit_birakmaz() {
    let (_directory, config) = temporary_config();
    let database = Database::open(config.database_path()).expect("veritabanı açılmalı");

    // Gerçek cari ve ürün eklenir (yabancı anahtar ilişkileri için).
    let customer = on_muhasebe::customer::CustomerDraft {
        code: "CAR-0001".to_owned(),
        name: "Öztürk İnşaat".to_owned(),
        ..Default::default()
    }
    .validate()
    .unwrap();
    let customer_id = database.customer_insert(&customer).unwrap();

    let product = on_muhasebe::product::ProductDraft {
        code: "URN-0001".to_owned(),
        name: "Vida".to_owned(),
        starting_stock: "10".to_owned(),
        ..Default::default()
    }
    .validate()
    .unwrap();
    let product_id = database.product_insert(&product).unwrap();
    let stock_before = database
        .product_get(product_id)
        .unwrap()
        .unwrap()
        .current_stock;

    // İkinci satır var olmayan bir ürüne bağlıdır: satır yazımı başarısız
    // olacağı için tüm işlem (başlık, ilk satır, hareket ve stok) geri alınmalı.
    let lines = vec![
        db_satir(product_id, "URN-0001", "Vida", 1000, 1000, 0, 20),
        db_satir(999_999, "URN-HAYALI", "Hayali Ürün", 1000, 1000, 0, 20),
    ];
    let invalid = db_fatura(
        "SF-2026-0001",
        InvoiceType::Sales,
        customer_id,
        "2026-10-18",
        lines,
    );

    let error = database
        .invoice_insert(&invalid)
        .expect_err("geçersiz ürün nedeniyle kayıt başarısız olmalı");
    assert!(matches!(error, on_muhasebe::error::AppError::Database(_)));

    // Hiçbir kayıt kalmamalı.
    assert_eq!(database.invoice_count().unwrap(), 0, "fatura yazılmamalı");
    let movements = database.movement_list_for_product(product_id).unwrap();
    assert!(
        movements.iter().all(|movement| {
            movement.movement_type != MovementType::SalesInvoice
                && movement.movement_type != MovementType::PurchaseInvoice
        }),
        "hiçbir fatura stok hareketi yazılmamalı"
    );
    assert_eq!(
        database
            .product_get(product_id)
            .unwrap()
            .unwrap()
            .current_stock,
        stock_before,
        "stok değişmemeli"
    );
}

#[test]
fn cari_bakiyesi_satis_ekler_alis_cikarir_iptaller_haric_tutar() {
    let (_directory, config) = temporary_config();
    let database = Database::open(config.database_path()).expect("veritabanı açılmalı");

    let customer = on_muhasebe::customer::CustomerDraft {
        code: "CAR-0001".to_owned(),
        name: "Öztürk İnşaat".to_owned(),
        ..Default::default()
    }
    .validate()
    .unwrap();
    let customer_id = database.customer_insert(&customer).unwrap();

    let product = on_muhasebe::product::ProductDraft {
        code: "URN-0001".to_owned(),
        name: "Vida".to_owned(),
        ..Default::default()
    }
    .validate()
    .unwrap();
    let product_id = database.product_insert(&product).unwrap();

    // Satış 120,00 ₺ (KDV %20 dâhil), alış 60,00 ₺.
    let sales = db_fatura(
        "SF-2026-0001",
        InvoiceType::Sales,
        customer_id,
        "2026-10-18",
        vec![db_satir(
            product_id, "URN-0001", "Vida", 10_000, 1000, 0, 20,
        )],
    );
    let sales_id = database.invoice_insert(&sales).unwrap();

    let purchase = db_fatura(
        "AF-2026-0001",
        InvoiceType::Purchase,
        customer_id,
        "2026-10-18",
        vec![db_satir(product_id, "URN-0001", "Vida", 10_000, 500, 0, 20)],
    );
    database.invoice_insert(&purchase).unwrap();

    let customer = database.customer_get(customer_id).unwrap().unwrap();
    assert_eq!(customer.sales_total.kurus(), 12_000);
    assert_eq!(customer.purchase_total.kurus(), 6_000);
    assert_eq!(customer.balance().kurus(), 6_000, "satış eksi alış");
    assert_eq!(customer.invoice_count, 2);
    assert!(customer.has_movements());

    // Genel bakış toplamları.
    assert_eq!(database.confirmed_sales_total().unwrap().kurus(), 12_000);
    assert_eq!(database.confirmed_purchases_total().unwrap().kurus(), 6_000);
    assert_eq!(database.recent_invoices(5).unwrap().len(), 2);

    // Satış faturası iptal edilince bakiyeden düşmeli.
    database.invoice_cancel(sales_id, "Test").unwrap();
    let customer = database.customer_get(customer_id).unwrap().unwrap();
    assert_eq!(customer.sales_total.kurus(), 0, "iptaller hariç tutulmalı");
    assert_eq!(customer.balance().kurus(), -6_000);
    assert_eq!(database.confirmed_sales_total().unwrap().kurus(), 0);
    assert_eq!(
        database.recent_invoices(5).unwrap().len(),
        2,
        "iptal edilen fatura da listede kalır"
    );

    // Cari için fatura listesi okunabilmeli.
    let invoices = database.invoice_list_for_customer(customer_id).unwrap();
    assert_eq!(invoices.len(), 2);
    assert!(invoices.iter().any(|invoice| invoice.is_cancelled()));
}

#[test]
fn migration_fatura_tablolarini_olusturur_ve_sema_surumu_6_olur() {
    let (_directory, config) = temporary_config();

    let database = Database::open(config.database_path()).expect("veritabanı açılmalı");
    let status = database.status().expect("durum okunmalı");

    assert_eq!(
        latest_version(),
        7,
        "kullanıcılar ve ayarlar migration'ı eklenmiş olmalı"
    );
    assert_eq!(status.schema_version, 7);
    assert_eq!(status.applied_migrations.len(), MIGRATIONS.len());
    assert_eq!(database.invoice_count().unwrap(), 0);
    assert_eq!(database.confirmed_sales_total().unwrap().kurus(), 0);
    assert_eq!(database.confirmed_purchases_total().unwrap().kurus(), 0);
    drop(database);

    // Yeniden açılışta migration tekrar uygulanmaz ve şema korunur.
    let database = Database::open(config.database_path()).expect("yeniden açılmalı");
    assert_eq!(
        database.status().unwrap().applied_migrations.len(),
        MIGRATIONS.len()
    );
    assert_eq!(
        database
            .invoice_next_number(InvoiceType::Sales, 2026)
            .expect("numara önerilebilmeli"),
        "SF-2026-0001"
    );
}

#[test]
fn form_dogrulama_hatalari_girdiyi_korur_ve_formu_acik_birakir() {
    let (_directory, config) = temporary_config();
    let (_context, mut app) = open_app(&config);

    let customer_id = musteri_ekle(&mut app, "CAR-0001", "Öztürk İnşaat");
    let product_id = urun_ekle(&mut app, "URN-0001", "Vida", "5", "10", "20", "10");

    fatura_formu_ac(&mut app, InvoiceType::Sales);
    {
        let state = app.invoice_state_mut();
        state.draft.number = String::new();
        state.draft.invoice_date = "18.10.2026".to_owned();
        state.draft.customer_id = None;
        state.draft.notes = "Notlar korunuyor".to_owned();
        state.draft.lines[0].quantity = "0".to_owned();
        state.draft.lines[0].unit_price = "abc".to_owned();
        state.draft.lines[0].vat_rate = "150".to_owned();
    }
    app.submit_invoice_form();

    let errors = &app.invoice_state().errors;
    assert_eq!(app.invoice_state().screen, InvoiceScreen::Form);
    assert!(errors.get(Field::Number).is_some());
    assert!(errors.get(Field::Date).is_some());
    assert!(errors.get(Field::Customer).is_some());
    assert!(errors.get_line(0, Field::LineProduct).is_some());
    assert!(errors.get_line(0, Field::LineQuantity).is_some());
    assert!(errors.get_line(0, Field::LineUnitPrice).is_some());
    assert!(errors.get_line(0, Field::LineVat).is_some());
    assert!(app.invoice_state().pending.is_none(), "onay istenmemeli");
    assert_eq!(
        app.invoice_state().draft.notes,
        "Notlar korunuyor",
        "girdi korunmalı"
    );

    // Alanlar düzeltildiğinde; boş iskonto ve boş vade tarihi 0/kabul edilmeli.
    {
        let state = app.invoice_state_mut();
        state.draft.number = "SF-2026-0100".to_owned();
        state.draft.invoice_date = "2026-10-18".to_owned();
        state.draft.due_date = String::new();
        state.draft.customer_id = Some(customer_id);
        state.draft.lines[0].product_id = Some(product_id);
        state.draft.lines[0].quantity = "1".to_owned();
        state.draft.lines[0].unit_price = "10".to_owned();
        state.draft.lines[0].discount_rate = String::new();
        state.draft.lines[0].vat_rate = "20".to_owned();
    }
    app.submit_invoice_form();
    assert!(
        app.invoice_state().errors.is_empty(),
        "boş iskonto/vade kabul edilmeli: {:?}",
        app.invoice_state().errors
    );
    assert!(app.invoice_state().pending.is_some());
}
