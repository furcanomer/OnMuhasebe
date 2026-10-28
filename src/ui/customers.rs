// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Cari hesap modülü ekranları: liste, form ve ayrıntı.
//!
//! Bu modül **yalnızca çizim yapar**. Veriyi [`OnMuhasebeApp::customer_state`]
//! üzerinden okur ve kullanıcı eylemlerini [`Action`] değerlerine dönüştürüp
//! uygulama katmanının komutlarına (`open_customer_new_form`,
//! `submit_customer_form`, ...) devreder. Hiçbir SQL bu dosyada bulunmaz.
//!
//! ## Ekran akışı
//!
//! ```text
//! Liste --(Yeni cari hesap)--> Form --(Kaydet)--> Ayrıntı
//!   ^                            |                  |
//!   |                     (İptal)|                  |(Düzenle)
//!   +----------------------------+                  |
//!   +-------(Listeye dön)---------------------------+
//! ```
//!
//! ## Yerleşim notu
//!
//! Satır içi hizalamalarda [`egui::Sides`] kullanılır. Liste satırlarında ve
//! ekran başlıklarında **sağ taraf korunmalıdır** (bakiye, durum, düğmeler);
//! bu nedenle `shrink_left()` kullanılır: sağ içerik önce ölçülür, sol içerik
//! kalan alana sığdırılır ve taşarsa kısaltılır. Varsayılan `extend()`
//! davranışı, sığmayan içerikte satırı pencereden taşırırdı.

use egui::{Button, ComboBox, Grid, Label, RichText, ScrollArea, Sides, TextEdit, Ui, vec2};

use crate::app::{CustomerScreen, FocusTarget, OnMuhasebeApp};
use crate::customer::{Customer, CustomerFilter, CustomerType, Field, FieldErrors};
use crate::money::Money;
use crate::payment::Payment;
use crate::ui::format::format_iso_date_short;
use crate::ui::theme::{Palette, metrics};
use crate::ui::{transfer, widgets};

/// Arayüzde kullanıcının başlattığı eylem.
///
/// Çizim sırasında yalnızca bu değer üretilir; uygulama durumu çizim bittikten
/// sonra değiştirilir. Böylece `&OnMuhasebeApp` (okuma) ve `&mut OnMuhasebeApp`
/// (komut) ödünçleri çakışmaz.
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
    /// Bu cari için tahsilat formunu açar.
    NewCollection(i64),
    /// Bu cari için ödeme formunu açar.
    NewPayment(i64),
    /// Cari listesini CSV olarak dışa aktarır.
    ExportCsv,
    /// CSV dosyası seçip içe aktarma önizlemesi açar.
    ImportCsv,
    /// Cari içe aktarma şablonunu indirir.
    DownloadTemplate,
    /// Carinin satış faturalarını açar.
    OpenInvoices(i64),
    /// Carinin ekstresini raporlarda açar.
    OpenStatement(i64),
}

impl Action {
    /// Eylemi uygulama katmanında uygular.
    fn apply(self, app: &mut OnMuhasebeApp) {
        match self {
            Self::NewForm => app.open_customer_new_form(),
            Self::OpenDetail(id) => app.open_customer_detail(id),
            Self::EditForm(id) => app.open_customer_edit_form(id),
            Self::ToggleActive(id) => app.toggle_customer_active(id),
            Self::BackToList => app.open_customer_list(),
            Self::ResetFilter => app.reset_customer_filter(),
            Self::DismissNotice => app.dismiss_customer_notice(),
            Self::NewCollection(id) => {
                app.open_payment_form_for_customer(id, crate::payment::PaymentType::Tahsilat);
            }
            Self::NewPayment(id) => {
                app.open_payment_form_for_customer(id, crate::payment::PaymentType::Odeme);
            }
            Self::ExportCsv => {
                if let Some(path) =
                    transfer::save_csv_dialog(&OnMuhasebeApp::customer_list_csv_name())
                {
                    let path = OnMuhasebeApp::csv_path(path);
                    let _ = app.export_customer_list_csv(path);
                }
            }
            Self::ImportCsv => {
                if let Some(path) = transfer::open_csv_dialog() {
                    let _ = app.preview_customer_import(path);
                }
            }
            Self::DownloadTemplate => {
                if let Some(path) = transfer::save_csv_dialog("cari_sablon.csv") {
                    let path = OnMuhasebeApp::csv_path(path);
                    let _ = app.write_customer_template(path);
                }
            }
            Self::OpenInvoices(id) => app.show_customer_sales_invoices(id),
            Self::OpenStatement(id) => app.show_customer_statement(id),
        }
    }
}

/// Aktif alt ekranı çizer.
pub fn show(ui: &mut Ui, app: &mut OnMuhasebeApp) {
    let screen = app.customer_state().screen;

    match screen {
        CustomerScreen::List => list(ui, app),
        CustomerScreen::Form => form(ui, app),
        CustomerScreen::Detail => detail(ui, app),
    }
}

// ---------------------------------------------------------------------------
// Ortak parçalar
// ---------------------------------------------------------------------------

/// Bilgilendirme, genel hata ve liste hatası mesajlarını çizer.
///
/// Herhangi bir mesaj çizildiyse `true` döner; çağıran taraf gerekli boşluğu
/// yalnızca bu durumda ekler.
fn messages(ui: &mut Ui, app: &OnMuhasebeApp, action: &mut Option<Action>) -> bool {
    let state = app.customer_state();
    let mut drawn = false;

    if let Some(notice) = &state.notice {
        if widgets::success_card(ui, "İşlem başarılı", notice) {
            *action = Some(Action::DismissNotice);
        }
        drawn = true;
    }

    for (title, message) in [
        ("İşlem tamamlanamadı", state.general_error.as_deref()),
        ("Cari hesaplar yüklenemedi", state.list_error.as_deref()),
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
///
/// Alanlar birden çok yardımcı fonksiyonda kullanıldığı için tanım tek yerde
/// toplanır; böylece etiket ile hata anahtarının birbirinden kopması engellenir.
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

/// Çok satırlı metin alanı (adres, notlar).
fn multiline_field_row(
    ui: &mut Ui,
    spec: &FieldSpec,
    value: &mut String,
    errors: &mut FieldErrors,
    rows: usize,
) {
    ui.label(field_label(
        spec.label,
        false,
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

/// Bakiye işaretine göre gösterim rengi.
///
/// * Pozitif (cari bize borçlu): yeşil.
/// * Negatif (biz cariye borçluyuz): kırmızı.
/// * Sıfır: nötr.
fn balance_color(balance: Money) -> egui::Color32 {
    if balance.kurus() > 0 {
        Palette::SUCCESS
    } else if balance.kurus() < 0 {
        Palette::DANGER
    } else {
        Palette::MUTED
    }
}

/// Boş bırakılmış alanlar için "—" gösteren yardımcı.
fn show_optional(value: Option<&str>) -> &str {
    match value {
        Some(value) if !value.trim().is_empty() => value,
        _ => "—",
    }
}

/// Listede satırın ikinci satırında gösterilen iletişim/vergi özeti.
fn details_line(customer: &Customer) -> String {
    let mut parts = vec![customer.kind.label().to_owned()];

    if let Some(phone) = &customer.phone {
        parts.push(format!("Tel: {phone}"));
    }
    if let Some(tax_office) = &customer.tax_office {
        parts.push(format!("V.D.: {tax_office}"));
    }
    if let Some((label, value)) = customer.tax_identifier() {
        parts.push(format!("{label}: {value}"));
    }
    if let Some(contact) = &customer.contact_person {
        parts.push(format!("Yetkili: {contact}"));
    }

    parts.join("  ·  ")
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
                        RichText::new("Cari Hesaplar")
                            .size(22.0)
                            .strong()
                            .color(Palette::TEXT),
                    )
                    .truncate(),
                );
                ui.add(
                    Label::new(
                        RichText::new(
                            "Müşteri ve tedarikçi kartlarını yönetin. Bakiye, kesinleşmiş \
                             alış/satış faturalarından hesaplanır.",
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
                .add(Button::new("Yeni cari hesap").min_size(vec2(150.0, 30.0)))
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

    let count = app.customer_state().list.len();
    ui.label(
        RichText::new(format!("{count} kayıt görüntüleniyor"))
            .size(12.5)
            .color(Palette::MUTED),
    );
    ui.add_space(6.0);

    {
        let state = app.customer_state();

        if state.list_error.is_none() {
            if state.list.is_empty() {
                empty_state(ui, &state.filter, &mut action);
            } else {
                // Yalnızca görünür satırlar çizilir; 1M kayıtta bile kare süresi
                // kayıt sayısından bağımsızdır.
                ScrollArea::vertical()
                    .id_salt("cari_listesi")
                    .auto_shrink([false, false])
                    .max_height(430.0)
                    .show_rows(ui, 92.0, state.list.len(), |ui, range| {
                        for index in range {
                            customer_row(ui, &state.list[index], app.is_manager(), &mut action);
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
///
/// Ölçüt değiştiğinde liste veritabanından yeniden okunur; böylece uzun
/// listelerde arama düzgün çalışır ve kare başına gereksiz sorgu yapılmaz.
fn filter_bar(ui: &mut Ui, app: &mut OnMuhasebeApp, action: &mut Option<Action>) {
    let before = app.customer_state().filter.clone();

    {
        let state = app.customer_state_mut();

        widgets::card(ui, "Ara ve filtrele", |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Ara:").color(Palette::MUTED));
                ui.add(
                    TextEdit::singleline(&mut state.filter.search)
                        .hint_text("Cari kodu, ad, telefon veya vergi no")
                        .desired_width(230.0),
                );

                ui.add_space(12.0);
                ui.label(RichText::new("Cari türü:").color(Palette::MUTED));
                ComboBox::from_id_salt("cari_tur_filtresi")
                    .selected_text(kind_filter_text(state.filter.kind))
                    .width(140.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut state.filter.kind, None, "Tümü");
                        for option in CustomerType::ALL {
                            ui.selectable_value(
                                &mut state.filter.kind,
                                Some(option),
                                option.label(),
                            );
                        }
                    });

                ui.add_space(12.0);
                ui.label(RichText::new("Durum:").color(Palette::MUTED));
                ComboBox::from_id_salt("cari_durum_filtresi")
                    .selected_text(status_filter_text(state.filter.active))
                    .width(120.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut state.filter.active, None, "Tümü");
                        ui.selectable_value(&mut state.filter.active, Some(true), "Aktif");
                        ui.selectable_value(&mut state.filter.active, Some(false), "Pasif");
                    });

                ui.add_space(12.0);
                if ui.button("Filtreleri temizle").clicked() {
                    *action = Some(Action::ResetFilter);
                }
            });
        });
    }

    if app.customer_state().filter != before {
        app.refresh_customer_list();
    }
}

fn kind_filter_text(kind: Option<CustomerType>) -> String {
    match kind {
        Some(kind) => format!("Tümü hariç: {}", kind.label()),
        None => "Tümü".to_owned(),
    }
}

fn status_filter_text(active: Option<bool>) -> &'static str {
    match active {
        None => "Tümü",
        Some(true) => "Aktif",
        Some(false) => "Pasif",
    }
}

/// Tek bir cari hesabın liste satırı.
fn customer_row(ui: &mut Ui, customer: &Customer, can_manage: bool, action: &mut Option<Action>) {
    widgets::surface(ui, |ui| {
        // 1. satır: kod + unvan (sol) | bakiye + durum (sağ)
        Sides::new().height(24.0).shrink_left().show(
            ui,
            |ui| {
                ui.label(
                    RichText::new(&customer.code)
                        .monospace()
                        .size(12.5)
                        .color(Palette::ACCENT),
                );
                ui.add(
                    Label::new(
                        RichText::new(&customer.name)
                            .size(14.5)
                            .strong()
                            .color(Palette::TEXT),
                    )
                    .truncate(),
                );
            },
            |ui| {
                ui.label(
                    RichText::new(format!(
                        "Bakiye: {}",
                        customer.balance().format_with_symbol()
                    ))
                    .size(12.5)
                    .strong()
                    .color(balance_color(customer.balance())),
                );
                ui.add_space(10.0);
                status_badge(ui, customer.active);
            },
        );

        // 2. satır: iletişim/vergi özeti (sol) | işlem düğmeleri (sağ)
        Sides::new().height(26.0).shrink_left().show(
            ui,
            |ui| {
                ui.add(
                    Label::new(
                        RichText::new(details_line(customer))
                            .size(12.5)
                            .color(Palette::MUTED),
                    )
                    .truncate(),
                );
            },
            |ui| {
                // `right_to_left` düzeninde ilk eklenen düğme en sağda yer alır;
                // bu nedenle görsel sıra tersine kurulur: [Ayrıntı][Düzenle][Durum].
                let toggle_text = if customer.active {
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
                    *action = Some(Action::ToggleActive(customer.id));
                }
                if ui.add(Button::new("Düzenle").small()).clicked() {
                    *action = Some(Action::EditForm(customer.id));
                }
                if ui.add(Button::new("Ayrıntı").small()).clicked() {
                    *action = Some(Action::OpenDetail(customer.id));
                }
            },
        );
    });
}

/// Sonuç bulunamadığında gösterilen bilgilendirme.
fn empty_state(ui: &mut Ui, filter: &CustomerFilter, action: &mut Option<Action>) {
    widgets::card(ui, "Kayıt bulunamadı", |ui| {
        if filter.is_empty() {
            widgets::subheading(ui, "Henüz cari hesap kaydı bulunmuyor");
            ui.add_space(4.0);
            ui.add(
                Label::new(
                    "Sağ üstteki \"Yeni cari hesap\" düğmesiyle ilk kaydı \
                     oluşturabilirsiniz.",
                )
                .wrap(),
            );
        } else {
            widgets::subheading(ui, "Arama ve filtre ölçütlerine uyan cari hesap bulunamadı");
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

    let is_new = app.customer_state().form_id.is_none();
    let error_count = app.customer_state().errors.len();

    ui.horizontal(|ui| {
        if ui.button("← Listeye dön").clicked() {
            cancel = true;
        }
        ui.add_space(6.0);
        ui.add(
            Label::new(
                RichText::new(if is_new {
                    "Yeni cari hesap"
                } else {
                    "Cari hesabı düzenle"
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

    let focus_code = app.take_focus(FocusTarget::CustomerCode);
    let focus_name = app.take_focus(FocusTarget::CustomerName);

    {
        let can_manage = app.is_manager();
        let state = app.customer_state_mut();
        let draft = &mut state.draft;
        let errors = &mut state.errors;

        widgets::card(ui, "Kimlik bilgileri", |ui| {
            Grid::new("cari_form_kimlik")
                .num_columns(2)
                .min_col_width(150.0)
                .spacing([16.0, 8.0])
                .show(ui, |ui| {
                    // Cari kodu: önerilir ama kullanıcı değiştirebilir.
                    ui.label(field_label(
                        "Cari kodu",
                        true,
                        errors.get(Field::Code).is_some(),
                    ));
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            let response = ui.add(
                                TextEdit::singleline(&mut draft.code)
                                    .hint_text("CAR-0001")
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

                    // Cari türü
                    ui.label(field_label("Cari türü", true, false));
                    ui.vertical(|ui| {
                        ComboBox::from_id_salt("cari_form_turu")
                            .selected_text(draft.kind.label())
                            .width(200.0)
                            .show_ui(ui, |ui| {
                                for option in CustomerType::ALL {
                                    ui.selectable_value(&mut draft.kind, option, option.label());
                                }
                            });
                    });
                    ui.end_row();

                    text_field_row_focused(
                        ui,
                        &FieldSpec::new(
                            "Ad veya firma unvanı",
                            true,
                            Field::Name,
                            "Örnek: Öztürk İnşaat Ltd. Şti.",
                            340.0,
                        ),
                        &mut draft.name,
                        errors,
                        focus_name,
                    );
                    text_field_row(
                        ui,
                        &FieldSpec::new(
                            "Yetkili kişi",
                            false,
                            Field::ContactPerson,
                            "Örnek: Ayşe Öztürk",
                            300.0,
                        ),
                        &mut draft.contact_person,
                        errors,
                    );
                });
        });

        ui.add_space(metrics::SECTION_GAP);

        widgets::card(ui, "İletişim bilgileri", |ui| {
            Grid::new("cari_form_iletisim")
                .num_columns(2)
                .min_col_width(150.0)
                .spacing([16.0, 8.0])
                .show(ui, |ui| {
                    text_field_row(
                        ui,
                        &FieldSpec::new(
                            "Telefon",
                            false,
                            Field::Phone,
                            "Örnek: 0212 555 12 34",
                            240.0,
                        ),
                        &mut draft.phone,
                        errors,
                    );
                    text_field_row(
                        ui,
                        &FieldSpec::new(
                            "E-posta",
                            false,
                            Field::Email,
                            "Örnek: info@firma.com",
                            300.0,
                        ),
                        &mut draft.email,
                        errors,
                    );
                    multiline_field_row(
                        ui,
                        &FieldSpec::new("Adres", false, Field::Address, "", 420.0),
                        &mut draft.address,
                        errors,
                        2,
                    );
                });
        });

        ui.add_space(metrics::SECTION_GAP);

        widgets::card(ui, "Vergi bilgileri", |ui| {
            Grid::new("cari_form_vergi")
                .num_columns(2)
                .min_col_width(150.0)
                .spacing([16.0, 8.0])
                .show(ui, |ui| {
                    text_field_row(
                        ui,
                        &FieldSpec::new(
                            "Vergi dairesi",
                            false,
                            Field::TaxOffice,
                            "Örnek: Beşiktaş",
                            240.0,
                        ),
                        &mut draft.tax_office,
                        errors,
                    );
                    text_field_row(
                        ui,
                        &FieldSpec::new(
                            "Vergi numarası",
                            false,
                            Field::TaxNumber,
                            "10 veya 11 hane",
                            200.0,
                        ),
                        &mut draft.tax_number,
                        errors,
                    );
                    text_field_row(
                        ui,
                        &FieldSpec::new(
                            "T.C. kimlik numarası",
                            false,
                            Field::NationalId,
                            "11 hane",
                            200.0,
                        ),
                        &mut draft.national_id,
                        errors,
                    );
                });
        });

        ui.add_space(metrics::SECTION_GAP);

        widgets::card(ui, "Diğer bilgiler", |ui| {
            Grid::new("cari_form_diger")
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
        app.suggest_customer_code();
    }
    if save {
        app.submit_customer_form();
    }
    if cancel {
        app.cancel_customer_form();
    }
    if let Some(action) = action {
        action.apply(app);
    }
}

// ---------------------------------------------------------------------------
// Ayrıntı ekranı
// ---------------------------------------------------------------------------

/// Kayıt ayrıntısı ve hesap özeti.
/// Cari ayrıntısındaki tahsilat/ödeme listesi (tarih, hesap, tutar).
fn payments_card(ui: &mut Ui, title: &str, payments: &[Payment], empty_text: &str) {
    widgets::card(ui, title, |ui| {
        if payments.is_empty() {
            widgets::bullet(ui, empty_text);
            return;
        }

        for payment in payments {
            Sides::new().height(22.0).shrink_left().show(
                ui,
                |ui| {
                    ui.label(
                        RichText::new(format_iso_date_short(&payment.payment_date))
                            .monospace()
                            .size(12.5)
                            .color(Palette::MUTED),
                    );
                    ui.add(
                        Label::new(
                            RichText::new(&payment.cash_account_name)
                                .size(12.5)
                                .color(Palette::TEXT),
                        )
                        .truncate(),
                    );
                    if payment.is_cancelled() {
                        ui.label(RichText::new("İptal").size(11.5).color(Palette::DANGER));
                    } else if payment.is_advance() {
                        ui.label(RichText::new("Avans").size(11.5).color(Palette::ACCENT));
                    }
                },
                |ui| {
                    ui.label(
                        RichText::new(payment.amount.format_with_symbol())
                            .size(12.5)
                            .strong()
                            .color(if payment.is_cancelled() {
                                Palette::MUTED
                            } else {
                                Palette::TEXT
                            }),
                    );
                },
            );
        }
    });
}

fn detail(ui: &mut Ui, app: &mut OnMuhasebeApp) {
    let mut action: Option<Action> = None;
    let mut edit = false;
    let mut toggle = false;
    let mut collect = false;
    let mut pay = false;

    ui.horizontal(|ui| {
        if ui.button("← Listeye dön").clicked() {
            action = Some(Action::BackToList);
        }
    });
    ui.add_space(8.0);

    messages(ui, app, &mut action);

    if app.customer_state().selected.is_none() {
        widgets::error_card(
            ui,
            "Kayıt görüntülenemiyor",
            "Seçili cari hesap bulunamadı. Listeye dönüp kaydı yeniden açın.",
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

    let customer_id = app
        .customer_state()
        .selected
        .as_ref()
        .map(|customer| customer.id);
    if let Some(customer_id) = customer_id {
        ui.horizontal_wrapped(|ui| {
            if ui.button("Faturaları gör").clicked() {
                action = Some(Action::OpenInvoices(customer_id));
            }
            if ui.button("Ekstresini gör").clicked() {
                action = Some(Action::OpenStatement(customer_id));
            }
        });
        ui.add_space(8.0);
    }

    {
        let state = app.customer_state();

        // Yukarıda `selected` alanının dolu olduğu doğrulandı; buradaki kontrol
        // yalnızca savunma amaçlıdır.
        let Some(customer) = state.selected.as_ref() else {
            return;
        };

        Sides::new().height(56.0).shrink_left().show(
            ui,
            |ui| {
                ui.vertical(|ui| {
                    ui.add(
                        Label::new(
                            RichText::new(&customer.name)
                                .size(20.0)
                                .strong()
                                .color(Palette::TEXT),
                        )
                        .truncate(),
                    );
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(&customer.code)
                                .monospace()
                                .size(12.5)
                                .color(Palette::ACCENT),
                        );
                        ui.label(
                            RichText::new(format!("· {}", customer.kind.label()))
                                .size(12.5)
                                .color(Palette::MUTED),
                        );
                        ui.label(RichText::new("·").size(12.5).color(Palette::MUTED));
                        status_badge(ui, customer.active);
                    });
                });
            },
            |ui| {
                // `right_to_left`: görsel sıra [Düzenle][Durum] olacak şekilde
                // önce durum düğmesi eklenir.
                let toggle_text = if customer.active {
                    "Pasife al"
                } else {
                    "Aktife al"
                };
                if app.is_manager()
                    && ui
                        .add(Button::new(toggle_text).min_size(vec2(110.0, 30.0)))
                        .on_hover_text("Kayıt silinmez, yalnızca durumu değişir.")
                        .clicked()
                {
                    toggle = true;
                }
                if ui
                    .add(Button::new("Düzenle").min_size(vec2(110.0, 30.0)))
                    .clicked()
                {
                    edit = true;
                }
            },
        );

        ui.add_space(metrics::SECTION_GAP);

        widgets::card(ui, "Hesap özeti", |ui| {
            Grid::new("cari_detay_ozet")
                .num_columns(2)
                .min_col_width(150.0)
                .spacing([16.0, 6.0])
                .show(ui, |ui| {
                    widgets::row_label(ui, "Güncel bakiye");
                    ui.label(
                        RichText::new(customer.balance().format_with_symbol())
                            .size(16.0)
                            .strong()
                            .color(balance_color(customer.balance())),
                    );
                    ui.end_row();

                    widgets::row_label(ui, "Durum");
                    ui.label(customer.status_label());
                    ui.end_row();

                    widgets::row_label(ui, "Oluşturulma tarihi");
                    ui.label(format!("{} (UTC)", customer.created_at));
                    ui.end_row();

                    widgets::row_label(ui, "Son güncelleme");
                    ui.label(format!("{} (UTC)", customer.updated_at));
                    ui.end_row();
                });

            ui.add_space(10.0);
            ui.add(
                Label::new(
                    RichText::new(
                        "Bakiye = kesin satış faturaları − kesin alış faturaları − kesin \
                         tahsilatlar + kesin ödemeler. Pozitif değer cari bize borçlu, \
                         negatif değer biz cariye borçluyuz demektir. İptal edilen \
                         fatura ve ödemeler hesaba katılmaz.",
                    )
                    .size(12.0)
                    .color(Palette::MUTED),
                )
                .wrap(),
            );
        });

        ui.add_space(metrics::SECTION_GAP);

        let invoices = app.customer_invoices(customer.id);
        widgets::card(ui, "Faturalar", |ui| {
            if invoices.is_empty() {
                widgets::bullet(ui, "Bu cari için henüz fatura kaydı bulunmuyor.");
                return;
            }

            for invoice in &invoices {
                Sides::new().height(22.0).shrink_left().show(
                    ui,
                    |ui| {
                        ui.label(
                            RichText::new(format_iso_date_short(&invoice.invoice_date))
                                .monospace()
                                .size(12.5)
                                .color(Palette::MUTED),
                        );
                        ui.label(
                            RichText::new(invoice.invoice_type.label())
                                .size(12.0)
                                .color(Palette::MUTED),
                        );
                        ui.add(
                            Label::new(
                                RichText::new(&invoice.number)
                                    .size(12.5)
                                    .color(Palette::ACCENT),
                            )
                            .truncate(),
                        );
                        if invoice.is_cancelled() {
                            ui.label(RichText::new("İptal").size(11.5).color(Palette::DANGER));
                        }
                    },
                    |ui| {
                        ui.label(
                            RichText::new(invoice.totals.grand_total.format_with_symbol())
                                .size(12.5)
                                .strong()
                                .color(if invoice.is_cancelled() {
                                    Palette::MUTED
                                } else {
                                    Palette::TEXT
                                }),
                        );
                    },
                );
            }
        });

        ui.add_space(metrics::SECTION_GAP);

        let (collections, outgoings) = app.customer_payments(customer.id);
        payments_card(
            ui,
            "Tahsilatlar",
            &collections,
            "Bu cari için henüz tahsilat kaydı bulunmuyor.",
        );

        ui.add_space(metrics::SECTION_GAP);

        payments_card(
            ui,
            "Ödemeler",
            &outgoings,
            "Bu cari için henüz ödeme kaydı bulunmuyor.",
        );

        ui.add_space(metrics::SECTION_GAP);

        widgets::card(ui, "Cari bilgileri", |ui| {
            Grid::new("cari_detay_bilgi")
                .num_columns(2)
                .min_col_width(150.0)
                .spacing([16.0, 6.0])
                .show(ui, |ui| {
                    for (label, value) in [
                        ("Cari kodu", Some(customer.code.as_str())),
                        ("Cari türü", Some(customer.kind.label())),
                        ("Ad / firma unvanı", Some(customer.name.as_str())),
                        ("Telefon", customer.phone.as_deref()),
                        ("E-posta", customer.email.as_deref()),
                        ("Yetkili kişi", customer.contact_person.as_deref()),
                        ("Vergi dairesi", customer.tax_office.as_deref()),
                        ("Vergi numarası", customer.tax_number.as_deref()),
                        ("T.C. kimlik numarası", customer.national_id.as_deref()),
                        ("Adres", customer.address.as_deref()),
                        ("Notlar", customer.notes.as_deref()),
                    ] {
                        widgets::row_label(ui, label);
                        ui.add(Label::new(show_optional(value)).wrap());
                        ui.end_row();
                    }
                });
        });
    }

    ui.add_space(metrics::SECTION_GAP);

    ui.horizontal(|ui| {
        if ui
            .add(Button::new("Tahsilat Al").min_size(vec2(130.0, 32.0)))
            .clicked()
        {
            collect = true;
        }
        if ui
            .add(Button::new("Ödeme Yap").min_size(vec2(130.0, 32.0)))
            .clicked()
        {
            pay = true;
        }
        if ui
            .add(Button::new("Düzenle").min_size(vec2(130.0, 32.0)))
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

    if let Some(id) = app.customer_state().detail_id {
        if edit {
            action = Some(Action::EditForm(id));
        }
        if toggle {
            action = Some(Action::ToggleActive(id));
        }
        if collect {
            action = Some(Action::NewCollection(id));
        }
        if pay {
            action = Some(Action::NewPayment(id));
        }
    }

    if let Some(action) = action {
        action.apply(app);
    }
}
