// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Ürün ve stok modülü ekranları: liste, form, ayrıntı ve stok hareketi formu.
//!
//! Bu modül **yalnızca çizim yapar**. Veriyi [`OnMuhasebeApp::product_state`]
//! üzerinden okur ve kullanıcı eylemlerini [`Action`] değerlerine dönüştürüp
//! uygulama katmanının komutlarına devreder. Hiçbir SQL bu dosyada bulunmaz.
//!
//! ## Ekran akışı
//!
//! ```text
//! Liste --(Yeni ürün)--> Form --(Kaydet)--> Ayrıntı --(Yeni Hareket)--> Hareket formu
//!   ^                       |                  |  ^                          |
//!   |                (İptal)|                  |  +-------(Kaydet)----------+
//!   +-----------------------+                  |(Düzenle)
//!   +-------(Listeye dön)----------------------+
//! ```
//!
//! Satır içi hizalamalarda [`egui::Sides`] kullanılır ve sağ taraf korunur
//! (`shrink_left()`); böylece uzun adlar dar pencerelerde satırı taşırmaz.

use egui::{Button, ComboBox, Grid, Label, RichText, ScrollArea, Sides, TextEdit, Ui, vec2};

use crate::app::{FocusTarget, OnMuhasebeApp, ProductScreen};
use crate::product::{Field, FieldErrors, MovementType, Product, ProductFilter, Unit, VAT_RATES};
use crate::ui::format::format_date_short;
use crate::ui::theme::{Palette, metrics};
use crate::ui::{transfer, widgets};

/// Arayüzde kullanıcının başlattığı eylem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    /// Yeni kayıt formunu açar.
    NewForm,
    /// Kaydın ayrıntısını açar.
    OpenDetail(i64),
    /// Kaydın düzenleme formunu açar.
    EditForm(i64),
    /// Kaydı aktif/pasif yapar.
    ToggleActive(i64),
    /// Liste ekranına döner.
    BackToList,
    /// Arama ve filtreleri temizler.
    ResetFilter,
    /// Bilgilendirme mesajını kapatır.
    DismissNotice,
    /// Yeni stok hareketi formunu açar.
    NewMovement(i64),
    /// Hareket formunu kapatıp ayrıntıya döner.
    CancelMovement,
    /// Negatif stok onayını verir.
    ConfirmMovement,
    /// Negatif stok onayını iptal eder.
    CancelMovementConfirmation,
    /// Ürün listesini CSV olarak dışa aktarır.
    ExportCsv,
    /// CSV dosyası seçip içe aktarma önizlemesi açar.
    ImportCsv,
    /// Ürün içe aktarma şablonunu indirir.
    DownloadTemplate,
}

impl Action {
    /// Eylemi uygulama katmanında uygular.
    fn apply(self, app: &mut OnMuhasebeApp) {
        match self {
            Self::NewForm => app.open_product_new_form(),
            Self::OpenDetail(id) => app.open_product_detail(id),
            Self::EditForm(id) => app.open_product_edit_form(id),
            Self::ToggleActive(id) => app.toggle_product_active(id),
            Self::BackToList => app.open_product_list(),
            Self::ResetFilter => app.reset_product_filter(),
            Self::DismissNotice => app.dismiss_product_notice(),
            Self::NewMovement(id) => app.open_movement_form(id),
            Self::CancelMovement => app.cancel_movement_form(),
            Self::ConfirmMovement => app.confirm_pending_movement(),
            Self::CancelMovementConfirmation => app.cancel_pending_movement(),
            Self::ExportCsv => {
                if let Some(path) =
                    transfer::save_csv_dialog(&OnMuhasebeApp::product_list_csv_name())
                {
                    let path = OnMuhasebeApp::csv_path(path);
                    let _ = app.export_product_list_csv(path);
                }
            }
            Self::ImportCsv => {
                if let Some(path) = transfer::open_csv_dialog() {
                    let _ = app.preview_product_import(path);
                }
            }
            Self::DownloadTemplate => {
                if let Some(path) = transfer::save_csv_dialog("urun_sablon.csv") {
                    let path = OnMuhasebeApp::csv_path(path);
                    let _ = app.write_product_template(path);
                }
            }
        }
    }
}

/// Aktif alt ekranı çizer.
pub fn show(ui: &mut Ui, app: &mut OnMuhasebeApp) {
    match app.product_state().screen {
        ProductScreen::List => list(ui, app),
        ProductScreen::Form => form(ui, app),
        ProductScreen::Detail => detail(ui, app),
        ProductScreen::MovementForm => movement_form(ui, app),
    }
}

// ---------------------------------------------------------------------------
// Ortak parçalar
// ---------------------------------------------------------------------------

/// Bilgilendirme, genel hata ve liste hatası mesajlarını çizer.
fn messages(ui: &mut Ui, app: &OnMuhasebeApp, action: &mut Option<Action>) -> bool {
    let state = app.product_state();
    let mut drawn = false;

    if let Some(notice) = &state.notice {
        if widgets::success_card(ui, "İşlem başarılı", notice) {
            *action = Some(Action::DismissNotice);
        }
        drawn = true;
    }

    for (title, message) in [
        ("İşlem tamamlanamadı", state.general_error.as_deref()),
        ("Ürünler yüklenemedi", state.list_error.as_deref()),
    ] {
        if let Some(message) = message {
            if drawn {
                ui.add_space(metrics::SECTION_GAP);
            }
            widgets::error_card(ui, title, message);
            drawn = true;
        }
    }

    drawn
}

/// Alan etiketi (zorunlu alanlar `*` ile işaretlenir, hatalıysa kırmızı olur).
fn field_label(text: &str, required: bool, has_error: bool) -> RichText {
    let text = if required {
        format!("{text} *")
    } else {
        text.to_owned()
    };
    let color = if has_error {
        Palette::DANGER
    } else {
        Palette::MUTED
    };

    RichText::new(text).size(13.0).color(color)
}

/// Alanın hata mesajını (varsa) çizer.
fn field_error(ui: &mut Ui, errors: &FieldErrors, field: Field) {
    if let Some(message) = errors.get(field) {
        ui.label(RichText::new(message).size(11.5).color(Palette::DANGER));
    }
}

/// Form alanının tanımı (etiket, zorunluluk, hata anahtarı, ipucu, genişlik).
struct FieldSpec {
    label: &'static str,
    required: bool,
    field: Field,
    hint: &'static str,
    width: f32,
}

impl FieldSpec {
    const fn new(
        label: &'static str,
        required: bool,
        field: Field,
        hint: &'static str,
        width: f32,
    ) -> Self {
        Self {
            label,
            required,
            field,
            hint,
            width,
        }
    }
}

/// Tek satırlık metin alanı (etiket + giriş + hata).
fn text_field_row(ui: &mut Ui, spec: &FieldSpec, value: &mut String, errors: &mut FieldErrors) {
    text_field_row_focused(ui, spec, value, errors, false);
}

/// Tek satırlık metin alanı; `focus` ise imleci bu alana taşır.
fn text_field_row_focused(
    ui: &mut Ui,
    spec: &FieldSpec,
    value: &mut String,
    errors: &mut FieldErrors,
    focus: bool,
) {
    ui.label(field_label(
        spec.label,
        spec.required,
        errors.get(spec.field).is_some(),
    ));
    ui.vertical(|ui| {
        let response = ui.add(
            TextEdit::singleline(value)
                .hint_text(spec.hint)
                .desired_width(spec.width),
        );
        if focus {
            response.request_focus();
        }
        if response.changed() {
            errors.clear(spec.field);
        }
        field_error(ui, errors, spec.field);
    });
    ui.end_row();
}

/// Çok satırlı metin alanı (notlar, hareket açıklaması).
fn multiline_field_row(
    ui: &mut Ui,
    spec: &FieldSpec,
    value: &mut String,
    errors: &mut FieldErrors,
    rows: usize,
) {
    ui.label(field_label(
        spec.label,
        spec.required,
        errors.get(spec.field).is_some(),
    ));
    ui.vertical(|ui| {
        if ui
            .add(
                TextEdit::multiline(value)
                    .desired_width(spec.width)
                    .desired_rows(rows),
            )
            .changed()
        {
            errors.clear(spec.field);
        }
        field_error(ui, errors, spec.field);
    });
    ui.end_row();
}

/// Aktif/pasif durumunu gösteren küçük renkli etiket.
fn status_badge(ui: &mut Ui, active: bool) {
    let (color, text) = if active {
        (Palette::SUCCESS, "Aktif")
    } else {
        (Palette::MUTED, "Pasif")
    };

    ui.label(RichText::new(format!("● {text}")).size(12.5).color(color));
}

/// Düşük stok uyarısı etiketi.
fn low_stock_badge(ui: &mut Ui) {
    ui.label(
        RichText::new("⚠ Düşük stok")
            .size(12.5)
            .color(Palette::WARNING),
    );
}

/// Boş bırakılmış alanlar için "—" gösteren yardımcı.
fn show_optional(value: Option<&str>) -> &str {
    match value {
        Some(value) if !value.trim().is_empty() => value,
        _ => "—",
    }
}

/// `YYYY-MM-DD` tarihini kısa Türkçe biçimde (`18.10.2026`) gösterir.
fn format_movement_date(iso: &str) -> String {
    chrono::NaiveDate::parse_from_str(iso, "%Y-%m-%d")
        .map(format_date_short)
        .unwrap_or_else(|_| iso.to_owned())
}

// ---------------------------------------------------------------------------
// Liste ekranı
// ---------------------------------------------------------------------------

/// Kayıt listesi, arama ve filtreleme.
fn list(ui: &mut Ui, app: &mut OnMuhasebeApp) {
    let mut action: Option<Action> = None;

    Sides::new().height(50.0).shrink_left().show(
        ui,
        |ui| {
            ui.vertical(|ui| {
                ui.add(
                    Label::new(
                        RichText::new("Ürün ve Stok")
                            .size(22.0)
                            .strong()
                            .color(Palette::TEXT),
                    )
                    .truncate(),
                );
                ui.add(
                    Label::new(
                        RichText::new(
                            "Ürün kartları, fiyatlar ve stok hareketleri. Güncel stok \
                             yalnızca hareketlerle değişir.",
                        )
                        .size(13.0)
                        .color(Palette::MUTED),
                    )
                    .truncate(),
                );
            });
        },
        |ui| {
            if ui
                .add(Button::new("Yeni ürün").min_size(vec2(130.0, 30.0)))
                .clicked()
            {
                action = Some(Action::NewForm);
            }
        },
    );

    ui.add_space(metrics::SECTION_GAP);
    if messages(ui, app, &mut action) {
        ui.add_space(metrics::SECTION_GAP);
    }

    ui.horizontal_wrapped(|ui| {
        if ui.button("CSV dışa aktar").clicked() {
            action = Some(Action::ExportCsv);
        }
        if app.is_manager() && ui.button("CSV içe aktar").clicked() {
            action = Some(Action::ImportCsv);
        }
        if ui.button("Şablon indir").clicked() {
            action = Some(Action::DownloadTemplate);
        }
    });
    ui.add_space(8.0);

    filter_bar(ui, app, &mut action);
    ui.add_space(10.0);

    let count = app.product_state().list.len();
    ui.label(
        RichText::new(format!("{count} kayıt görüntüleniyor"))
            .size(12.5)
            .color(Palette::MUTED),
    );
    ui.add_space(2.0);
    ui.add(
        Label::new(
            RichText::new(
                "Sütunlar: ürün kodu · barkod · ad · birim · alış fiyatı · satış fiyatı · \
                 KDV % · mevcut stok · min. stok · durum",
            )
            .size(11.0)
            .color(Palette::MUTED),
        )
        .wrap(),
    );
    ui.add_space(6.0);

    {
        let state = app.product_state();

        if state.list_error.is_none() {
            if state.list.is_empty() {
                empty_state(ui, &state.filter, &mut action);
            } else {
                ScrollArea::vertical()
                    .id_salt("urun_listesi")
                    .auto_shrink([false, false])
                    .max_height(430.0)
                    .show_rows(ui, 92.0, state.list.len(), |ui, range| {
                        for index in range {
                            product_row(ui, &state.list[index], app.is_manager(), &mut action);
                            ui.add_space(6.0);
                        }
                    });
            }
        }
    }

    if let Some(action) = action {
        action.apply(app);
    }
}

/// Arama ve filtreleme çubuğu.
fn filter_bar(ui: &mut Ui, app: &mut OnMuhasebeApp, action: &mut Option<Action>) {
    let before = app.product_state().filter.clone();

    {
        let state = app.product_state_mut();

        widgets::card(ui, "Ara ve filtrele", |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Ara:").color(Palette::MUTED));
                ui.add(
                    TextEdit::singleline(&mut state.filter.search)
                        .hint_text("Ürün kodu, barkod, ad veya kategori")
                        .desired_width(230.0),
                );

                ui.add_space(12.0);
                ui.label(RichText::new("Durum:").color(Palette::MUTED));
                ComboBox::from_id_salt("urun_durum_filtresi")
                    .selected_text(status_filter_text(state.filter.active))
                    .width(120.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut state.filter.active, None, "Tümü");
                        ui.selectable_value(&mut state.filter.active, Some(true), "Aktif");
                        ui.selectable_value(&mut state.filter.active, Some(false), "Pasif");
                    });

                ui.add_space(12.0);
                ui.checkbox(
                    &mut state.filter.low_stock_only,
                    "Yalnızca düşük stoklu ürünler",
                );

                ui.add_space(12.0);
                if ui.button("Filtreleri temizle").clicked() {
                    *action = Some(Action::ResetFilter);
                }
            });
        });
    }

    if app.product_state().filter != before {
        app.refresh_product_list();
    }
}

fn status_filter_text(active: Option<bool>) -> &'static str {
    match active {
        None => "Tümü",
        Some(true) => "Aktif",
        Some(false) => "Pasif",
    }
}

/// Tek bir ürünün liste satırı (düşük stokluysa vurgulu).
fn product_row(ui: &mut Ui, product: &Product, can_manage: bool, action: &mut Option<Action>) {
    if product.is_low_stock() {
        widgets::surface_tinted(
            ui,
            Palette::WARNING.gamma_multiply(0.10),
            Palette::WARNING,
            |ui| product_row_contents(ui, product, can_manage, action),
        );
    } else {
        widgets::surface(ui, |ui| {
            product_row_contents(ui, product, can_manage, action)
        });
    }
}

/// Ürün satırının içeriği (iki satır: başlık ve ayrıntılar).
fn product_row_contents(
    ui: &mut Ui,
    product: &Product,
    can_manage: bool,
    action: &mut Option<Action>,
) {
    // 1. satır: kod + ad (sol) | stok + durum (sağ)
    Sides::new().height(24.0).shrink_left().show(
        ui,
        |ui| {
            ui.label(
                RichText::new(&product.code)
                    .monospace()
                    .size(12.5)
                    .color(Palette::ACCENT),
            );
            ui.add(
                Label::new(
                    RichText::new(&product.name)
                        .size(14.5)
                        .strong()
                        .color(Palette::TEXT),
                )
                .truncate(),
            );
            if product.is_low_stock() {
                low_stock_badge(ui);
            }
        },
        |ui| {
            let stock_color = if product.is_low_stock() {
                Palette::WARNING
            } else {
                Palette::TEXT
            };
            ui.label(
                RichText::new(format!("Stok: {}", product.formatted_stock()))
                    .size(12.5)
                    .strong()
                    .color(stock_color),
            );
            ui.add_space(10.0);
            status_badge(ui, product.active);
        },
    );

    // 2. satır: ayrıntılar (sol) | işlem düğmeleri (sağ)
    Sides::new().height(26.0).shrink_left().show(
        ui,
        |ui| {
            let details = format!(
                "Barkod: {}  ·  Birim: {}  ·  Alış: {}  ·  Satış: {}  ·  KDV: {}  ·  Min. stok: {}",
                show_optional(product.barcode.as_deref()),
                product.unit.short_label(),
                product.purchase_price.format_with_symbol(),
                product.sale_price.format_with_symbol(),
                product.vat_label(),
                product.formatted_min_stock(),
            );
            ui.add(Label::new(RichText::new(details).size(12.5).color(Palette::MUTED)).truncate());
        },
        |ui| {
            // `right_to_left`: görsel sıra [Ayrıntı][Düzenle][Durum] olacak
            // şekilde önce durum düğmesi eklenir.
            let toggle_text = if product.active {
                "Pasife al"
            } else {
                "Aktife al"
            };
            if can_manage
                && ui
                    .add(Button::new(toggle_text).small())
                    .on_hover_text("Kayıt silinmez, yalnızca durumu değişir.")
                    .clicked()
            {
                *action = Some(Action::ToggleActive(product.id));
            }
            if ui.add(Button::new("Düzenle").small()).clicked() {
                *action = Some(Action::EditForm(product.id));
            }
            if ui.add(Button::new("Ayrıntı").small()).clicked() {
                *action = Some(Action::OpenDetail(product.id));
            }
        },
    );
}

/// Sonuç bulunamadığında gösterilen bilgilendirme.
fn empty_state(ui: &mut Ui, filter: &ProductFilter, action: &mut Option<Action>) {
    widgets::card(ui, "Kayıt bulunamadı", |ui| {
        if filter.is_empty() {
            widgets::subheading(ui, "Henüz ürün kaydı bulunmuyor");
            ui.add_space(4.0);
            ui.add(
                Label::new("Sağ üstteki \"Yeni ürün\" düğmesiyle ilk ürünü oluşturabilirsiniz.")
                    .wrap(),
            );
        } else {
            widgets::subheading(ui, "Arama ve filtre ölçütlerine uyan ürün bulunamadı");
            ui.add_space(4.0);
            ui.add(Label::new("Arama metnini değiştirin veya filtreleri temizleyin.").wrap());
            ui.add_space(8.0);
            if ui.button("Filtreleri temizle").clicked() {
                *action = Some(Action::ResetFilter);
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Form ekranı
// ---------------------------------------------------------------------------

/// Yeni kayıt / düzenleme formu.
fn form(ui: &mut Ui, app: &mut OnMuhasebeApp) {
    let mut action: Option<Action> = None;
    let mut save = false;
    let mut cancel = false;
    let mut suggest_code = false;

    let is_new = app.product_state().form_id.is_none();
    let error_count = app.product_state().errors.len();

    ui.horizontal(|ui| {
        if ui.button("← Listeye dön").clicked() {
            cancel = true;
        }
        ui.add_space(6.0);
        ui.add(
            Label::new(
                RichText::new(if is_new {
                    "Yeni ürün"
                } else {
                    "Ürünü düzenle"
                })
                .size(20.0)
                .strong()
                .color(Palette::TEXT),
            )
            .truncate(),
        );
    });
    ui.add_space(metrics::SECTION_GAP);

    if messages(ui, app, &mut action) {
        ui.add_space(metrics::SECTION_GAP);
    }

    if error_count > 0 {
        ui.add_space(metrics::SECTION_GAP);
        widgets::error_card(
            ui,
            "Form kaydedilemedi",
            &format!(
                "{error_count} alan düzeltilmeli. Girdiğiniz bilgiler korundu; \
                 işaretli alanları düzenleyip yeniden kaydedin."
            ),
        );
    }

    ui.add_space(metrics::SECTION_GAP);

    let focus_code = app.take_focus(FocusTarget::ProductCode);
    let focus_name = app.take_focus(FocusTarget::ProductName);

    {
        let can_manage = app.is_manager();
        let state = app.product_state_mut();
        let draft = &mut state.draft;
        let errors = &mut state.errors;

        widgets::card(ui, "Kimlik bilgileri", |ui| {
            Grid::new("urun_form_kimlik")
                .num_columns(2)
                .min_col_width(150.0)
                .spacing([16.0, 8.0])
                .show(ui, |ui| {
                    // Ürün kodu: önerilir ama kullanıcı değiştirebilir.
                    ui.label(field_label(
                        "Ürün kodu",
                        true,
                        errors.get(Field::Code).is_some(),
                    ));
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            let response = ui.add(
                                TextEdit::singleline(&mut draft.code)
                                    .hint_text("URN-0001")
                                    .desired_width(200.0),
                            );
                            if focus_code {
                                response.request_focus();
                            }
                            if response.changed() {
                                errors.clear(Field::Code);
                            }
                            if ui
                                .button("Yeni kod öner")
                                .on_hover_text("Kullanılmayan bir kod üretir.")
                                .clicked()
                            {
                                suggest_code = true;
                            }
                        });
                        field_error(ui, errors, Field::Code);
                    });
                    ui.end_row();

                    text_field_row(
                        ui,
                        &FieldSpec::new(
                            "Barkod",
                            false,
                            Field::Barcode,
                            "Örnek: 8691234567890",
                            240.0,
                        ),
                        &mut draft.barcode,
                        errors,
                    );
                    text_field_row_focused(
                        ui,
                        &FieldSpec::new(
                            "Ürün adı",
                            true,
                            Field::Name,
                            "Örnek: Çelik vida 4x40",
                            340.0,
                        ),
                        &mut draft.name,
                        errors,
                        focus_name,
                    );

                    ui.label(field_label("Ölçü birimi", true, false));
                    ComboBox::from_id_salt("urun_form_birim")
                        .selected_text(draft.unit.label())
                        .width(220.0)
                        .show_ui(ui, |ui| {
                            for unit in Unit::ALL {
                                ui.selectable_value(&mut draft.unit, unit, unit.label());
                            }
                        });
                    ui.end_row();

                    text_field_row(
                        ui,
                        &FieldSpec::new(
                            "Kategori",
                            false,
                            Field::Category,
                            "Örnek: Bağlantı elemanları",
                            300.0,
                        ),
                        &mut draft.category,
                        errors,
                    );
                });
        });

        ui.add_space(metrics::SECTION_GAP);

        widgets::card(ui, "Fiyat ve vergi", |ui| {
            Grid::new("urun_form_fiyat")
                .num_columns(2)
                .min_col_width(150.0)
                .spacing([16.0, 8.0])
                .show(ui, |ui| {
                    text_field_row(
                        ui,
                        &FieldSpec::new(
                            "Alış fiyatı (₺)",
                            false,
                            Field::PurchasePrice,
                            "Örnek: 1.234,56",
                            200.0,
                        ),
                        &mut draft.purchase_price,
                        errors,
                    );
                    text_field_row(
                        ui,
                        &FieldSpec::new(
                            "Satış fiyatı (₺)",
                            false,
                            Field::SalePrice,
                            "Örnek: 1.999,00",
                            200.0,
                        ),
                        &mut draft.sale_price,
                        errors,
                    );

                    ui.label(field_label(
                        "KDV oranı (%)",
                        true,
                        errors.get(Field::VatRate).is_some(),
                    ));
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            if ui
                                .add(
                                    TextEdit::singleline(&mut draft.vat_rate)
                                        .hint_text("20")
                                        .desired_width(70.0),
                                )
                                .changed()
                            {
                                errors.clear(Field::VatRate);
                            }
                            ComboBox::from_id_salt("urun_form_kdv")
                                .selected_text("Hızlı seçim")
                                .width(120.0)
                                .show_ui(ui, |ui| {
                                    for rate in VAT_RATES {
                                        if ui.selectable_label(false, format!("%{rate}")).clicked()
                                        {
                                            draft.vat_rate = rate.to_string();
                                            errors.clear(Field::VatRate);
                                        }
                                    }
                                });
                        });
                        field_error(ui, errors, Field::VatRate);
                    });
                    ui.end_row();
                });
        });

        ui.add_space(metrics::SECTION_GAP);

        widgets::card(ui, "Stok", |ui| {
            Grid::new("urun_form_stok")
                .num_columns(2)
                .min_col_width(150.0)
                .spacing([16.0, 8.0])
                .show(ui, |ui| {
                    text_field_row(
                        ui,
                        &FieldSpec::new(
                            "Minimum stok",
                            false,
                            Field::MinStock,
                            "Örnek: 10 veya 1,5",
                            160.0,
                        ),
                        &mut draft.min_stock,
                        errors,
                    );

                    if is_new {
                        text_field_row(
                            ui,
                            &FieldSpec::new(
                                "Başlangıç stoğu",
                                false,
                                Field::StartingStock,
                                "Boş bırakılabilir; girilirse ilk stok hareketi oluşur.",
                                200.0,
                            ),
                            &mut draft.starting_stock,
                            errors,
                        );
                    } else {
                        ui.label(field_label("Başlangıç stoğu", false, false));
                        ui.label(
                            RichText::new(
                                "Stok, kayıt sonrası yalnızca stok hareketleriyle değiştirilir.",
                            )
                            .size(12.0)
                            .color(Palette::MUTED),
                        );
                        ui.end_row();
                    }
                });
        });

        ui.add_space(metrics::SECTION_GAP);

        widgets::card(ui, "Diğer bilgiler", |ui| {
            Grid::new("urun_form_diger")
                .num_columns(2)
                .min_col_width(150.0)
                .spacing([16.0, 8.0])
                .show(ui, |ui| {
                    multiline_field_row(
                        ui,
                        &FieldSpec::new("Notlar", false, Field::Notes, "", 420.0),
                        &mut draft.notes,
                        errors,
                        3,
                    );

                    ui.label(field_label("Durum", false, false));
                    ui.vertical(|ui| {
                        ui.add_enabled(can_manage, egui::Checkbox::new(&mut draft.active, "Aktif"));
                        ui.label(
                            RichText::new(if draft.active {
                                "Kayıt aktif olarak kaydedilecek."
                            } else {
                                "Kayıt pasif olarak kaydedilecek; listede kalır ve silinmez."
                            })
                            .size(11.5)
                            .color(Palette::MUTED),
                        );
                    });
                    ui.end_row();
                });
        });
    }

    ui.add_space(metrics::SECTION_GAP);

    ui.horizontal(|ui| {
        if ui
            .add(Button::new("Kaydet").min_size(vec2(130.0, 32.0)))
            .on_hover_text("Ctrl+S")
            .clicked()
        {
            save = true;
        }
        if ui
            .add(Button::new("İptal").min_size(vec2(130.0, 32.0)))
            .on_hover_text("Esc")
            .clicked()
        {
            cancel = true;
        }
        ui.add_space(10.0);
        ui.label(
            RichText::new("Kaydetmeden çıkarsanız girdiğiniz bilgiler kullanılmaz.")
                .size(11.5)
                .color(Palette::MUTED),
        );
    });

    if suggest_code {
        app.suggest_product_code();
    }
    if save {
        app.submit_product_form();
    }
    if cancel {
        app.cancel_product_form();
    }
    if let Some(action) = action {
        action.apply(app);
    }
}

// ---------------------------------------------------------------------------
// Ayrıntı ekranı
// ---------------------------------------------------------------------------

/// Ürün ayrıntısı ve stok hareketleri.
fn detail(ui: &mut Ui, app: &mut OnMuhasebeApp) {
    let mut action: Option<Action> = None;
    let mut edit = false;
    let mut toggle = false;
    let mut new_movement = false;

    ui.horizontal(|ui| {
        if ui.button("← Listeye dön").clicked() {
            action = Some(Action::BackToList);
        }
    });
    ui.add_space(8.0);

    messages(ui, app, &mut action);

    if app.product_state().selected.is_none() {
        widgets::error_card(
            ui,
            "Kayıt görüntülenemiyor",
            "Seçili ürün bulunamadı. Listeye dönüp kaydı yeniden açın.",
        );
        ui.add_space(metrics::SECTION_GAP);

        if ui.button("← Listeye dön").clicked() {
            action = Some(Action::BackToList);
        }
        if let Some(action) = action {
            action.apply(app);
        }
        return;
    }

    {
        let state = app.product_state();

        let Some(product) = state.selected.as_ref() else {
            return;
        };

        Sides::new().height(56.0).shrink_left().show(
            ui,
            |ui| {
                ui.vertical(|ui| {
                    ui.add(
                        Label::new(
                            RichText::new(&product.name)
                                .size(20.0)
                                .strong()
                                .color(Palette::TEXT),
                        )
                        .truncate(),
                    );
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(&product.code)
                                .monospace()
                                .size(12.5)
                                .color(Palette::ACCENT),
                        );
                        ui.label(
                            RichText::new(format!("· {}", product.unit.label()))
                                .size(12.5)
                                .color(Palette::MUTED),
                        );
                        ui.label(RichText::new("·").size(12.5).color(Palette::MUTED));
                        status_badge(ui, product.active);
                        if product.is_low_stock() {
                            low_stock_badge(ui);
                        }
                    });
                });
            },
            |ui| {
                // `right_to_left`: görsel sıra [Yeni Hareket][Düzenle][Durum].
                let toggle_text = if product.active {
                    "Pasife al"
                } else {
                    "Aktife al"
                };
                if app.is_manager()
                    && ui
                        .add(Button::new(toggle_text).min_size(vec2(100.0, 30.0)))
                        .on_hover_text("Kayıt silinmez, yalnızca durumu değişir.")
                        .clicked()
                {
                    toggle = true;
                }
                if ui
                    .add(Button::new("Düzenle").min_size(vec2(100.0, 30.0)))
                    .clicked()
                {
                    edit = true;
                }
                if ui
                    .add(Button::new("Yeni Hareket").min_size(vec2(120.0, 30.0)))
                    .clicked()
                {
                    new_movement = true;
                }
            },
        );

        ui.add_space(metrics::SECTION_GAP);

        widgets::card(ui, "Stok özeti", |ui| {
            Grid::new("urun_detay_stok")
                .num_columns(2)
                .min_col_width(150.0)
                .spacing([16.0, 6.0])
                .show(ui, |ui| {
                    widgets::row_label(ui, "Mevcut stok");
                    ui.label(
                        RichText::new(product.formatted_stock())
                            .size(17.0)
                            .strong()
                            .color(if product.is_low_stock() {
                                Palette::WARNING
                            } else {
                                Palette::TEXT
                            }),
                    );
                    ui.end_row();

                    widgets::row_label(ui, "Minimum stok");
                    ui.label(product.formatted_min_stock());
                    ui.end_row();

                    widgets::row_label(ui, "Durum");
                    ui.label(product.status_label());
                    ui.end_row();

                    widgets::row_label(ui, "Alış fiyatı");
                    ui.label(product.purchase_price.format_with_symbol());
                    ui.end_row();

                    widgets::row_label(ui, "Satış fiyatı");
                    ui.label(product.sale_price.format_with_symbol());
                    ui.end_row();

                    widgets::row_label(ui, "KDV oranı");
                    ui.label(product.vat_label());
                    ui.end_row();

                    widgets::row_label(ui, "Kategori");
                    ui.label(show_optional(product.category.as_deref()));
                    ui.end_row();

                    widgets::row_label(ui, "Oluşturulma tarihi");
                    ui.label(format!("{} (UTC)", product.created_at));
                    ui.end_row();

                    widgets::row_label(ui, "Son güncelleme");
                    ui.label(format!("{} (UTC)", product.updated_at));
                    ui.end_row();

                    widgets::row_label(ui, "Barkod");
                    ui.label(show_optional(product.barcode.as_deref()));
                    ui.end_row();
                });

            if product.is_low_stock() {
                ui.add_space(8.0);
                widgets::bullet(
                    ui,
                    &format!(
                        "Düşük stok uyarısı: mevcut stok ({}) minimumun ({}) altında.",
                        product.formatted_stock(),
                        product.formatted_min_stock()
                    ),
                );
            }

            if let Some(notes) = product.notes.as_deref() {
                ui.add_space(6.0);
                widgets::subheading(ui, "Notlar");
                ui.add_space(2.0);
                ui.add(Label::new(notes).wrap());
            }
        });
    }

    ui.add_space(metrics::SECTION_GAP);

    movements_card(ui, app);

    ui.add_space(metrics::SECTION_GAP);

    ui.horizontal(|ui| {
        if ui
            .add(Button::new("Yeni Hareket").min_size(vec2(140.0, 32.0)))
            .clicked()
        {
            new_movement = true;
        }
        if ui
            .add(Button::new("Düzenle").min_size(vec2(120.0, 32.0)))
            .clicked()
        {
            edit = true;
        }
        if ui
            .add(Button::new("← Listeye dön").min_size(vec2(150.0, 32.0)))
            .clicked()
        {
            action = Some(Action::BackToList);
        }
    });

    if let Some(id) = app.product_state().detail_id {
        if new_movement {
            action = Some(Action::NewMovement(id));
        }
        if edit {
            action = Some(Action::EditForm(id));
        }
        if toggle {
            action = Some(Action::ToggleActive(id));
        }
    }

    if let Some(action) = action {
        action.apply(app);
    }
}

/// Ayrıntı ekranındaki stok hareketleri listesi.
fn movements_card(ui: &mut Ui, app: &OnMuhasebeApp) {
    let state = app.product_state();

    widgets::card(ui, "Stok hareketleri", |ui| {
        if state.movements.is_empty() {
            widgets::bullet(ui, "Bu ürün için henüz stok hareketi bulunmuyor.");
            return;
        }

        for movement in &state.movements {
            Sides::new().height(24.0).shrink_left().show(
                ui,
                |ui| {
                    ui.label(
                        RichText::new(format_movement_date(&movement.movement_date))
                            .monospace()
                            .size(12.5)
                            .color(Palette::MUTED),
                    );
                    ui.label(
                        RichText::new(movement.movement_type.label())
                            .size(12.5)
                            .strong()
                            .color(Palette::TEXT),
                    );
                    ui.label(
                        RichText::new(show_optional(movement.description.as_deref()))
                            .size(12.5)
                            .color(Palette::MUTED),
                    );
                },
                |ui| {
                    let positive = movement.quantity_delta.is_positive();
                    ui.label(
                        RichText::new(movement.formatted_quantity())
                            .size(13.0)
                            .strong()
                            .color(if positive {
                                Palette::SUCCESS
                            } else {
                                Palette::DANGER
                            }),
                    );
                },
            );
        }

        ui.add_space(6.0);
        ui.add(
            Label::new(
                RichText::new(
                    "Hareketler doğrudan silinemez veya düzenlenemez; düzeltme, \
                     ters yönlü bir \"Düzeltme\" hareketiyle yapılır.",
                )
                .size(11.5)
                .color(Palette::MUTED),
            )
            .wrap(),
        );
    });
}

// ---------------------------------------------------------------------------
// Stok hareketi formu
// ---------------------------------------------------------------------------

/// Yeni stok hareketi formu (önizleme ve negatif stok onayı ile).
fn movement_form(ui: &mut Ui, app: &mut OnMuhasebeApp) {
    let mut action: Option<Action> = None;
    let mut save = false;
    let mut cancel = false;
    let mut today = false;

    let error_count = app.product_state().movement_errors.len();
    let confirmation = app.product_state().movement_confirmation;
    let is_adjust = app.product_state().movement_draft.movement_type == MovementType::Adjust;
    let product = app.product_state().selected.as_ref().map(|product| {
        (
            product.name.clone(),
            product.code.clone(),
            product.unit,
            product.current_stock,
        )
    });

    let Some((name, code, unit, current_stock)) = product else {
        widgets::error_card(
            ui,
            "Hareket eklenemiyor",
            "Seçili ürün bulunamadı. Listeye dönüp ürünü yeniden açın.",
        );
        ui.add_space(metrics::SECTION_GAP);
        if ui.button("← Listeye dön").clicked() {
            action = Some(Action::BackToList);
        }
        if let Some(action) = action {
            action.apply(app);
        }
        return;
    };

    let preview = app.movement_preview();

    ui.horizontal(|ui| {
        if ui.button("← Ürün ayrıntısına dön").clicked() {
            cancel = true;
        }
        ui.add_space(6.0);
        ui.add(
            Label::new(
                RichText::new("Yeni stok hareketi")
                    .size(20.0)
                    .strong()
                    .color(Palette::TEXT),
            )
            .truncate(),
        );
    });
    ui.add_space(4.0);
    ui.label(
        RichText::new(format!(
            "{code} · {name} · Mevcut stok: {}",
            current_stock.format_with_label(unit.short_label())
        ))
        .size(13.0)
        .color(Palette::MUTED),
    );
    ui.add_space(metrics::SECTION_GAP);

    if messages(ui, app, &mut action) {
        ui.add_space(metrics::SECTION_GAP);
    }

    if error_count > 0 {
        ui.add_space(metrics::SECTION_GAP);
        widgets::error_card(
            ui,
            "Hareket kaydedilemedi",
            &format!("{error_count} alan düzeltilmeli. Girdiğiniz bilgiler korundu."),
        );
    }

    // Negatif stok onayı: kullanıcı onaylayana kadar hareket yazılmaz.
    if let Some(new_stock) = confirmation {
        ui.add_space(metrics::SECTION_GAP);
        widgets::notice_card(
            ui,
            "Stok eksiye düşecek",
            &format!(
                "Bu hareketten sonra stok eksiye düşecek ({}). Devam edilsin mi?",
                new_stock.format_with_label(unit.short_label())
            ),
        );
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui
                .add(Button::new("Devam et").min_size(vec2(120.0, 30.0)))
                .clicked()
            {
                action = Some(Action::ConfirmMovement);
            }
            if ui
                .add(Button::new("Vazgeç").min_size(vec2(120.0, 30.0)))
                .clicked()
            {
                action = Some(Action::CancelMovementConfirmation);
            }
        });
    }

    ui.add_space(metrics::SECTION_GAP);

    let focus_quantity = app.take_focus(FocusTarget::MovementQuantity);

    {
        let state = app.product_state_mut();
        let draft = &mut state.movement_draft;
        let errors = &mut state.movement_errors;

        widgets::card(ui, "Hareket bilgileri", |ui| {
            Grid::new("hareket_form")
                .num_columns(2)
                .min_col_width(150.0)
                .spacing([16.0, 8.0])
                .show(ui, |ui| {
                    ui.label(field_label("Hareket türü", true, false));
                    ui.vertical(|ui| {
                        ComboBox::from_id_salt("hareket_form_tur")
                            .selected_text(draft.movement_type.label())
                            .width(200.0)
                            .show_ui(ui, |ui| {
                                for option in MovementType::FORM_OPTIONS {
                                    ui.selectable_value(
                                        &mut draft.movement_type,
                                        option,
                                        option.label(),
                                    );
                                }
                            });
                        ui.label(
                            RichText::new(match draft.movement_type {
                                MovementType::In => "Stok artırılır (+).",
                                MovementType::Out => "Stok azaltılır (-).",
                                MovementType::Adjust => {
                                    "Miktarın yönü aşağıdaki işaretle seçilir; açıklama zorunludur."
                                }
                                // İlk stok ve fatura hareketleri elle formda sunulmaz.
                                MovementType::Initial
                                | MovementType::PurchaseInvoice
                                | MovementType::SalesInvoice => "",
                            })
                            .size(11.5)
                            .color(Palette::MUTED),
                        );
                    });
                    ui.end_row();

                    ui.label(field_label(
                        "Miktar",
                        true,
                        errors.get(Field::MovementQuantity).is_some(),
                    ));
                    ui.vertical(|ui| {
                        let response = ui.add(
                            TextEdit::singleline(&mut draft.quantity)
                                .hint_text("Örnek: 10 veya 1,5")
                                .desired_width(200.0),
                        );
                        if focus_quantity {
                            response.request_focus();
                        }
                        if response.changed() {
                            errors.clear(Field::MovementQuantity);
                        }
                        if draft.movement_type == MovementType::Adjust {
                            ui.checkbox(
                                &mut draft.adjustment_negative,
                                "Miktar eksi yönlü (stok azaltır)",
                            );
                        }
                        field_error(ui, errors, Field::MovementQuantity);
                    });
                    ui.end_row();

                    ui.label(field_label(
                        "Tarih",
                        true,
                        errors.get(Field::MovementDate).is_some(),
                    ));
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            if ui
                                .add(
                                    TextEdit::singleline(&mut draft.date)
                                        .hint_text("YYYY-AA-GG")
                                        .desired_width(140.0),
                                )
                                .changed()
                            {
                                errors.clear(Field::MovementDate);
                            }
                            if ui.button("Bugün").clicked() {
                                today = true;
                            }
                        });
                        field_error(ui, errors, Field::MovementDate);
                    });
                    ui.end_row();

                    multiline_field_row(
                        ui,
                        &FieldSpec::new(
                            "Açıklama",
                            draft.movement_type == MovementType::Adjust,
                            Field::MovementDescription,
                            "Düzeltme hareketlerinde zorunludur.",
                            420.0,
                        ),
                        &mut draft.description,
                        errors,
                        2,
                    );
                });

            if is_adjust {
                ui.add_space(6.0);
                ui.label(
                    RichText::new(
                        "Düzeltme hareketleri sayım farkı gibi durumlar içindir; \
                         açıklama doldurulmalıdır.",
                    )
                    .size(11.5)
                    .color(Palette::MUTED),
                );
            }
        });
    }

    // Önizleme: kaydetmeden önce yeni stok gösterilir.
    if let Some(new_stock) = preview {
        ui.add_space(metrics::SECTION_GAP);
        let color = if new_stock.is_negative() {
            Palette::WARNING
        } else {
            Palette::TEXT
        };
        ui.label(
            RichText::new(format!(
                "Önizleme — Yeni stok: {}",
                new_stock.format_with_label(unit.short_label())
            ))
            .size(13.5)
            .strong()
            .color(color),
        );
    }

    ui.add_space(metrics::SECTION_GAP);

    ui.horizontal(|ui| {
        if ui
            .add(Button::new("Kaydet").min_size(vec2(130.0, 32.0)))
            .on_hover_text("Ctrl+S")
            .clicked()
        {
            save = true;
        }
        if ui
            .add(Button::new("İptal").min_size(vec2(130.0, 32.0)))
            .on_hover_text("Esc")
            .clicked()
        {
            cancel = true;
        }
    });

    if today {
        app.product_state_mut().movement_draft.date = crate::product::today_iso();
        app.product_state_mut()
            .movement_errors
            .clear(Field::MovementDate);
    }
    if save {
        app.submit_movement_form();
    }
    if cancel {
        action = Some(Action::CancelMovement);
    }
    if let Some(action) = action {
        action.apply(app);
    }
}
