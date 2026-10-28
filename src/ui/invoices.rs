// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Alış ve satış faturası ekranları: liste, form ve ayrıntı.
//!
//! Bu modül **yalnızca çizim yapar**. Veriyi [`OnMuhasebeApp::invoice_state`]
//! üzerinden okur ve kullanıcı eylemlerini [`Action`] değerlerine dönüştürüp
//! uygulama katmanının komutlarına (`open_invoice_new_form`,
//! `submit_invoice_form`, `confirm_invoice_save`, ...) devreder. Hiçbir SQL bu
//! dosyada bulunmaz.
//!
//! ## Ekran akışı
//!
//! ```text
//! Liste --(Yeni fatura)--> Form --(Kaydet)--> [Onay] --(Onayla)--> Ayrıntı
//!   ^                       |                   |                  |
//!   |                (İptal) |            (Vazgeç) |                |(İptal Et)
//!   +-----------------------+---------------------+                 v
//!   +------------------------(Listeye dön)-------------------- İptal onayı
//! ```
//!
//! ## Yerleşim notu
//!
//! Satır içi hizalamalarda [`egui::Sides`] kullanılır ve `shrink_left()` ile
//! sağ taraf (tutar, durum, düğmeler) korunur. Form alanları
//! `horizontal_wrapped` ile sarılır; böylece 900x600 gibi dar pencerelerde
//! taşma oluşmaz.

use egui::{Button, ComboBox, Grid, Label, RichText, ScrollArea, Sides, TextEdit, Ui, vec2};

use crate::app::{FocusTarget, InvoiceScreen, OnMuhasebeApp};
use crate::invoice::{
    Field, FieldErrors, Invoice, InvoiceDraft, InvoiceLine, InvoiceLineDraft, InvoiceStatus,
    InvoiceTotals, InvoiceType, PaymentState,
};
use crate::money::Money;
use crate::product::VAT_RATES;
use crate::ui::cash;
use crate::ui::format::format_iso_date_short;
use crate::ui::theme::{Palette, metrics};
use crate::ui::{transfer, widgets};

/// Kullanıcı eylemleri; çizim sırasında toplanır ve sonra uygulanır.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    /// Yeni fatura formunu açar.
    NewForm,
    /// Faturanın ayrıntısını açar.
    OpenDetail(i64),
    /// Listeden faturayı iptal etmek üzere ayrıntıyı ve iptal penceresini açar.
    CancelInvoice(i64),
    /// Liste ekranına döner.
    BackToList,
    /// Arama ve filtreleri temizler.
    ResetFilter,
    /// Bilgilendirme mesajını kapatır.
    DismissNotice,
    /// Kullanılmamış bir fatura numarası önerir.
    SuggestNumber,
    /// Yeni formun türünü değiştirir.
    SetFormType(InvoiceType),
    /// Forma boş satır ekler.
    AddLine,
    /// Formdan satır kaldırır.
    RemoveLine(usize),
    /// Formu doğrulayıp kaydetme onayı ister.
    Submit,
    /// Onaylanan faturayı kesinleştirir.
    ConfirmSave,
    /// Kaydetme onayından vazgeçer.
    DiscardConfirmation,
    /// İptal onay penceresini açar.
    StartCancel,
    /// İptal onay penceresini kapatır.
    AbortCancel,
    /// İptal nedenini doğrulayıp faturayı iptal eder.
    SubmitCancel,
    /// Bu fatura için önceden dağıtılmış tahsilat/ödeme formunu açar.
    OpenPayment(i64),
    /// Fatura listesini CSV olarak dışa aktarır.
    ExportCsv,
    /// Faturanın cari hesabını açar.
    OpenCustomer(i64),
    /// Fatura satırının ürününü açar.
    OpenProduct(i64),
}

impl Action {
    /// Eylemi uygulama katmanında uygular.
    fn apply(self, app: &mut OnMuhasebeApp) {
        match self {
            Self::NewForm => app.open_invoice_new_form(),
            Self::OpenDetail(id) => app.open_invoice_detail(id),
            Self::CancelInvoice(id) => app.open_invoice_cancel(id),
            Self::BackToList => app.open_invoice_list(),
            Self::ResetFilter => app.reset_invoice_filter(),
            Self::DismissNotice => app.dismiss_invoice_notice(),
            Self::SuggestNumber => app.suggest_invoice_number(),
            Self::SetFormType(invoice_type) => app.set_invoice_draft_type(invoice_type),
            Self::AddLine => app.add_invoice_line(),
            Self::RemoveLine(index) => app.remove_invoice_line(index),
            Self::Submit => app.submit_invoice_form(),
            Self::ConfirmSave => app.confirm_invoice_save(),
            Self::DiscardConfirmation => app.discard_invoice_confirmation(),
            Self::StartCancel => app.start_invoice_cancel(),
            Self::AbortCancel => app.cancel_invoice_cancel(),
            Self::SubmitCancel => app.submit_invoice_cancel(),
            Self::OpenPayment(id) => app.open_payment_form_for_invoice(id),
            Self::ExportCsv => {
                if let Some(path) = transfer::save_csv_dialog(&app.invoice_list_csv_name()) {
                    let path = OnMuhasebeApp::csv_path(path);
                    let _ = app.export_invoice_list_csv(path);
                }
            }
            Self::OpenCustomer(id) => app.show_customer_detail(id),
            Self::OpenProduct(id) => app.show_product_detail(id),
        }
    }
}

/// Aktif alt ekranı çizer.
pub fn show(ui: &mut Ui, app: &mut OnMuhasebeApp) {
    match app.invoice_state().screen {
        InvoiceScreen::List => list(ui, app),
        InvoiceScreen::Form => form(ui, app),
        InvoiceScreen::Detail => detail(ui, app),
    }
}

// ---------------------------------------------------------------------------
// Ortak parçalar
// ---------------------------------------------------------------------------

/// Bilgilendirme ve hata mesajlarını çizer.
fn messages(ui: &mut Ui, app: &OnMuhasebeApp, action: &mut Option<Action>) -> bool {
    let state = app.invoice_state();
    let mut drawn = false;

    if let Some(notice) = &state.notice {
        if widgets::success_card(ui, "İşlem başarılı", notice) {
            *action = Some(Action::DismissNotice);
        }
        drawn = true;
    }

    for (title, message) in [
        ("İşlem tamamlanamadı", state.general_error.as_deref()),
        ("Faturalar yüklenemedi", state.list_error.as_deref()),
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

/// Form geneli bir alanın hata mesajı.
fn field_error(ui: &mut Ui, errors: &FieldErrors, field: Field) {
    if let Some(message) = errors.get(field) {
        ui.label(RichText::new(message).size(11.5).color(Palette::DANGER));
    }
}

/// Satıra ait bir alanın hata mesajı.
fn line_error(ui: &mut Ui, errors: &FieldErrors, index: usize, field: Field) {
    if let Some(message) = errors.get_line(index, field) {
        ui.label(RichText::new(message).size(11.5).color(Palette::DANGER));
    }
}

/// Durum etiketi.
fn status_badge(ui: &mut Ui, status: InvoiceStatus) {
    let (color, text) = match status {
        InvoiceStatus::Confirmed => (Palette::SUCCESS, "Kesin"),
        InvoiceStatus::Cancelled => (Palette::DANGER, "İptal"),
    };
    ui.label(RichText::new(format!("● {text}")).size(12.5).color(color));
}

/// Tahsilat durumu etiketi (fatura iptal edilmişse nötr tire gösterilir).
fn payment_state_badge(ui: &mut Ui, state: PaymentState) {
    let (color, text) = match state {
        PaymentState::Unpaid => (Palette::DANGER, "Ödenmemiş"),
        PaymentState::Partial => (Palette::WARNING, "Kısmi"),
        PaymentState::Paid => (Palette::SUCCESS, "Ödendi"),
    };
    ui.label(RichText::new(format!("● {text}")).size(12.5).color(color));
}

/// Boş metin yerine "—" gösterir.
fn or_dash(value: &str) -> String {
    if value.trim().is_empty() {
        "—".to_owned()
    } else {
        value.to_owned()
    }
}

/// Para tutarını etiketle birlikte satır olarak yazar.
fn money_row(ui: &mut Ui, label: &str, amount: Money, emphasized: bool) {
    widgets::row_label(ui, label);
    let mut text = RichText::new(amount.format_with_symbol());
    if emphasized {
        text = text.size(15.0).strong().color(Palette::TEXT);
    } else {
        text = text.color(Palette::TEXT);
    }
    ui.label(text);
    ui.end_row();
}

// ---------------------------------------------------------------------------
// Liste ekranı
// ---------------------------------------------------------------------------

/// Fatura listesi, arama ve filtreleme.
fn list(ui: &mut Ui, app: &mut OnMuhasebeApp) {
    let mut action: Option<Action> = None;
    let title = app.invoice_state().invoice_type.list_title();
    let description = match app.invoice_state().invoice_type {
        InvoiceType::Purchase => {
            "Tedarikçilerden alınan faturalar. Kesinleşen fatura düzenlenemez."
        }
        InvoiceType::Sales => "Müşterilere kesilen faturalar. Kesinleşen fatura düzenlenemez.",
    };

    Sides::new().height(50.0).shrink_left().show(
        ui,
        |ui| {
            ui.vertical(|ui| {
                ui.add(
                    Label::new(
                        RichText::new(title)
                            .size(22.0)
                            .strong()
                            .color(Palette::TEXT),
                    )
                    .truncate(),
                );
                ui.add(
                    Label::new(RichText::new(description).size(13.0).color(Palette::MUTED))
                        .truncate(),
                );
            });
        },
        |ui| {
            if ui
                .add(Button::new("Yeni fatura").min_size(vec2(150.0, 30.0)))
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
    });
    ui.add_space(8.0);

    filter_bar(ui, app, &mut action);
    ui.add_space(10.0);

    {
        let state = app.invoice_state();
        let confirmed_total = Money::checked_sum(
            state
                .list
                .iter()
                .filter(|invoice| !invoice.is_cancelled())
                .map(|invoice| invoice.totals.grand_total),
        )
        .unwrap_or_default();

        ui.label(
            RichText::new(format!(
                "{} fatura, genel toplam: {}",
                state.list.len(),
                confirmed_total.format_with_symbol()
            ))
            .size(12.5)
            .color(Palette::MUTED),
        );
        ui.add_space(6.0);

        if state.list_error.is_none() {
            if state.list.is_empty() {
                empty_state(ui, &state.filter);
            } else {
                ScrollArea::vertical()
                    .id_salt("fatura_listesi")
                    .auto_shrink([false, false])
                    .max_height(430.0)
                    .show_rows(ui, 92.0, state.list.len(), |ui, range| {
                        for index in range {
                            invoice_row(ui, &state.list[index], app.is_manager(), &mut action);
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

/// Arama ve filtreleme çubuğu; ölçüt değişirse liste yenilenir.
fn filter_bar(ui: &mut Ui, app: &mut OnMuhasebeApp, action: &mut Option<Action>) {
    let before = app.invoice_state().filter.clone();
    let customer_label = app
        .invoice_state()
        .filter
        .customer_id
        .and_then(|id| app.invoice_customer_label(id))
        .unwrap_or_else(|| "Tümü".to_owned());
    let customer_options = app.invoice_all_customer_options();

    {
        let state = app.invoice_state_mut();

        widgets::card(ui, "Ara ve filtrele", |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Ara:").color(Palette::MUTED));
                ui.add(
                    TextEdit::singleline(&mut state.filter.search)
                        .hint_text("Fatura numarası veya cari adı")
                        .desired_width(200.0),
                );

                ui.add_space(12.0);
                ui.label(RichText::new("Cari:").color(Palette::MUTED));
                if let Some(id) = widgets::searchable_select(
                    ui,
                    "fatura_cari_filtresi",
                    &customer_label,
                    &customer_options,
                    200.0,
                ) {
                    state.filter.customer_id = Some(id);
                }
                if state.filter.customer_id.is_some()
                    && ui
                        .small_button("×")
                        .on_hover_text("Cari filtresini kaldır")
                        .clicked()
                {
                    state.filter.customer_id = None;
                }

                ui.add_space(12.0);
                ui.label(RichText::new("Durum:").color(Palette::MUTED));
                ComboBox::from_id_salt("fatura_durum_filtresi")
                    .selected_text(match state.filter.status {
                        None => "Tümü",
                        Some(status) => status.label(),
                    })
                    .width(110.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut state.filter.status, None, "Tümü");
                        for status in InvoiceStatus::ALL {
                            ui.selectable_value(
                                &mut state.filter.status,
                                Some(status),
                                status.label(),
                            );
                        }
                    });

                ui.add_space(12.0);
                ui.label(RichText::new("Tarih:").color(Palette::MUTED));
                ui.add(
                    TextEdit::singleline(&mut state.filter.date_from)
                        .hint_text("Başlangıç")
                        .desired_width(105.0),
                );
                ui.label(RichText::new("–").color(Palette::MUTED));
                ui.add(
                    TextEdit::singleline(&mut state.filter.date_to)
                        .hint_text("Bitiş")
                        .desired_width(105.0),
                );

                ui.add_space(12.0);
                if ui.button("Filtreleri temizle").clicked() {
                    *action = Some(Action::ResetFilter);
                }
            });
        });
    }

    if app.invoice_state().filter != before {
        app.refresh_invoice_list();
    }
}

/// Liste boşken gösterilen bilgilendirme.
fn empty_state(ui: &mut Ui, filter: &crate::invoice::InvoiceFilter) {
    let message = if filter.is_empty() {
        "Bu bölümde henüz fatura kaydı bulunmuyor. \"Yeni fatura\" ile ilk faturanızı \
         oluşturabilirsiniz."
    } else {
        "Arama ölçütlerine uyan fatura bulunamadı."
    };
    widgets::card(ui, "Kayıt yok", |ui| {
        ui.add(Label::new(RichText::new(message).color(Palette::MUTED)).wrap());
    });
}

/// Tek bir faturanın liste satırı.
fn invoice_row(ui: &mut Ui, invoice: &Invoice, can_cancel: bool, action: &mut Option<Action>) {
    let fill = if invoice.is_cancelled() {
        Palette::BACKGROUND
    } else {
        Palette::SURFACE
    };

    widgets::surface_tinted(ui, fill, Palette::BORDER, |ui| {
        Sides::new().height(24.0).shrink_left().show(
            ui,
            |ui| {
                ui.label(
                    RichText::new(&invoice.number)
                        .monospace()
                        .size(12.5)
                        .color(Palette::ACCENT),
                );
                ui.add(
                    Label::new(
                        RichText::new(&invoice.customer_name)
                            .size(14.5)
                            .strong()
                            .color(Palette::TEXT),
                    )
                    .truncate(),
                );
            },
            |ui| {
                ui.label(
                    RichText::new(invoice.totals.grand_total.format_with_symbol())
                        .size(14.0)
                        .strong()
                        .color(Palette::TEXT),
                );
                if let Some(state) = invoice.payment_state() {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(format!(
                            "Kalan: {}",
                            invoice.remaining().format_with_symbol()
                        ))
                        .size(12.5)
                        .color(Palette::MUTED),
                    );
                    ui.add_space(6.0);
                    payment_state_badge(ui, state);
                }
                ui.add_space(8.0);
                status_badge(ui, invoice.status);
            },
        );

        ui.add_space(2.0);

        Sides::new().height(24.0).shrink_left().show(
            ui,
            |ui| {
                ui.label(
                    RichText::new(format_iso_date_short(&invoice.invoice_date))
                        .size(12.5)
                        .color(Palette::MUTED),
                );
                ui.label(
                    RichText::new(format!(
                        "· {} · Matrah: {} · KDV: {}",
                        invoice.invoice_type.label(),
                        invoice.totals.net_total.format_with_symbol(),
                        invoice.totals.vat_total.format_with_symbol()
                    ))
                    .size(12.5)
                    .color(Palette::MUTED),
                );
            },
            |ui| {
                if ui.button("Ayrıntı").clicked() {
                    *action = Some(Action::OpenDetail(invoice.id));
                }
                if can_cancel
                    && invoice.status == InvoiceStatus::Confirmed
                    && ui.small_button("İptal Et").clicked()
                {
                    *action = Some(Action::CancelInvoice(invoice.id));
                }
            },
        );
    });
}

// ---------------------------------------------------------------------------
// Form ekranı (yalnızca yeni kayıt)
// ---------------------------------------------------------------------------

/// Yeni fatura formu.
fn form(ui: &mut Ui, app: &mut OnMuhasebeApp) {
    let mut action: Option<Action> = None;
    let mut product_change: Option<(usize, i64)> = None;

    let invoice_type = app.invoice_state().draft.invoice_type;

    Sides::new().height(46.0).shrink_left().show(
        ui,
        |ui| {
            ui.horizontal(|ui| {
                if ui.button("← Listeye dön").clicked() {
                    action = Some(Action::BackToList);
                }
                ui.add_space(6.0);
                ui.add(
                    Label::new(
                        RichText::new(format!("Yeni {} Faturası", invoice_type.label()))
                            .size(20.0)
                            .strong()
                            .color(Palette::TEXT),
                    )
                    .truncate(),
                );
            });
        },
        |_ui| {},
    );

    ui.add_space(metrics::SECTION_GAP);
    if messages(ui, app, &mut action) {
        ui.add_space(metrics::SECTION_GAP);
    }

    let error_count = app.invoice_state().errors.len();
    if error_count > 0 {
        widgets::error_card(
            ui,
            "Form kaydedilemedi",
            &format!(
                "{error_count} alan düzeltilmeli. Girdiğiniz bilgiler korundu; \
                 işaretli alanları düzenleyip yeniden kaydedin."
            ),
        );
        ui.add_space(metrics::SECTION_GAP);
    }

    // Kaydetme onayı: form dondurulur, kullanıcı onaylarsa kesinleşir.
    let pending = app.invoice_state().pending.clone();
    if let Some(pending) = pending {
        confirmation(ui, app, &pending, &mut action);
        if let Some(action) = action {
            action.apply(app);
        }
        return;
    }

    let customer_options = app.invoice_customer_options(invoice_type);
    let customer_label = app
        .invoice_state()
        .draft
        .customer_id
        .and_then(|id| app.invoice_customer_label(id))
        .unwrap_or_else(|| "Cari seçin".to_owned());
    let product_options = app.invoice_product_options();
    let product_labels: Vec<Option<String>> = app
        .invoice_state()
        .draft
        .lines
        .iter()
        .map(|line| line.product_id.and_then(|id| app.invoice_product_label(id)))
        .collect();
    let mismatch = app
        .invoice_selected_customer()
        .map(|customer| !invoice_type.is_customer_kind_suitable(customer.kind))
        .unwrap_or(false);
    let mismatch_message = app.invoice_selected_customer().map(|customer| {
        format!(
            "\"{}\" cari türü ({}) bu fatura türü için beklenen tür değil. Kaydetmeyi \
             engellemiyoruz; yine de kontrol edin.",
            customer.name,
            customer.kind.label()
        )
    });

    let focus_number =
        app.take_focus(FocusTarget::InvoiceNumber) || app.take_focus(FocusTarget::InvoiceCustomer);

    ui.add_space(metrics::SECTION_GAP);
    {
        let state = app.invoice_state_mut();
        header_fields(
            ui,
            state,
            &customer_options,
            &customer_label,
            focus_number,
            &mut action,
        );
    }

    if mismatch {
        ui.add_space(metrics::SECTION_GAP);
        if let Some(message) = &mismatch_message {
            widgets::notice_card(ui, "Cari türü uyarısı", message);
        }
    }

    ui.add_space(metrics::SECTION_GAP);
    {
        let state = app.invoice_state_mut();
        lines_editor(
            ui,
            state,
            &product_options,
            &product_labels,
            &mut product_change,
            &mut action,
        );
    }

    ui.add_space(metrics::SECTION_GAP);
    {
        let state = app.invoice_state_mut();
        totals_card(ui, &state.draft);
    }

    ui.add_space(metrics::SECTION_GAP);
    ui.horizontal(|ui| {
        if ui
            .add(Button::new("Kaydet").min_size(vec2(140.0, 30.0)))
            .on_hover_text("Ctrl+S")
            .clicked()
        {
            action = Some(Action::Submit);
        }
        if ui
            .add(Button::new("İptal").min_size(vec2(120.0, 30.0)))
            .on_hover_text("Esc")
            .clicked()
        {
            action = Some(Action::BackToList);
        }
    });

    if let Some((index, product_id)) = product_change {
        app.apply_invoice_line_product(index, product_id);
    }
    if let Some(action) = action {
        action.apply(app);
    }
}

/// Kaydetme onayı kartı (stok yetersizliği uyarılarıyla).
fn confirmation(
    ui: &mut Ui,
    app: &OnMuhasebeApp,
    pending: &crate::invoice::ValidatedInvoice,
    action: &mut Option<Action>,
) {
    let shortages = app.invoice_stock_shortages(pending);

    widgets::card(ui, "Faturayı kesinleştir", |ui| {
        ui.add(
            Label::new(
                RichText::new(
                    "Fatura kesinleşecek. Stok ve cari bakiyesi güncellenecek. \
                     Onaylıyor musunuz?",
                )
                .size(13.5)
                .color(Palette::TEXT),
            )
            .wrap(),
        );

        ui.add_space(6.0);
        ui.label(
            RichText::new(format!(
                "{} satır · Genel toplam: {}",
                pending.line_count(),
                pending.totals.grand_total.format_with_symbol()
            ))
            .size(12.5)
            .color(Palette::MUTED),
        );

        if !shortages.is_empty() {
            ui.add_space(8.0);
            widgets::surface_tinted(
                ui,
                Palette::DANGER.gamma_multiply(0.07),
                Palette::DANGER,
                |ui| {
                    ui.label(
                        RichText::new("Bazı ürünlerin stoğu eksiye düşecek:")
                            .strong()
                            .color(Palette::DANGER),
                    );
                    for (label, current, new_stock) in &shortages {
                        ui.label(
                            RichText::new(format!(
                                "• {label}: mevcut {} → {}",
                                current.format_turkish(),
                                new_stock.format_turkish()
                            ))
                            .size(12.5),
                        );
                    }
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new(
                            "Onaylarsanız fatura yine de kaydedilir; stok negatif görünür.",
                        )
                        .size(12.0)
                        .color(Palette::MUTED),
                    );
                },
            );
        }

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui
                .add(Button::new("Onayla ve Kaydet").min_size(vec2(170.0, 30.0)))
                .clicked()
            {
                *action = Some(Action::ConfirmSave);
            }
            if ui
                .add(Button::new("Vazgeç").min_size(vec2(120.0, 30.0)))
                .clicked()
            {
                *action = Some(Action::DiscardConfirmation);
            }
        });
    });
}

/// Form üst bilgisi: tür, numara, tarihler, cari ve notlar.
fn header_fields(
    ui: &mut Ui,
    state: &mut crate::app::InvoiceState,
    customer_options: &[(i64, String)],
    customer_label: &str,
    focus_number: bool,
    action: &mut Option<Action>,
) {
    widgets::card(ui, "Fatura bilgileri", |ui| {
        Grid::new("fatura_form_ust")
            .num_columns(2)
            .min_col_width(150.0)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                // Tür (yalnızca yeni faturada seçilir).
                ui.label(field_label("Fatura türü", true, false));
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        let mut chosen = state.draft.invoice_type;
                        for option in InvoiceType::ALL {
                            ui.selectable_value(&mut chosen, option, option.label());
                        }
                        if chosen != state.draft.invoice_type {
                            *action = Some(Action::SetFormType(chosen));
                        }
                    });
                    ui.label(
                        RichText::new("Kesinleştikten sonra tür değiştirilemez.")
                            .size(11.5)
                            .color(Palette::MUTED),
                    );
                });
                ui.end_row();

                // Numara.
                ui.label(field_label(
                    "Fatura numarası",
                    true,
                    state.errors.get(Field::Number).is_some(),
                ));
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        let response = ui.add(
                            TextEdit::singleline(&mut state.draft.number)
                                .hint_text("SF-2026-0001")
                                .desired_width(200.0),
                        );
                        if focus_number {
                            response.request_focus();
                        }
                        if response.changed() {
                            state.errors.clear(Field::Number);
                        }
                        if ui
                            .button("Yeni numara öner")
                            .on_hover_text("Seri ve yıla göre kullanılmayan bir numara üretir.")
                            .clicked()
                        {
                            *action = Some(Action::SuggestNumber);
                        }
                    });
                    field_error(ui, &state.errors, Field::Number);
                });
                ui.end_row();

                // Tarih.
                ui.label(field_label(
                    "Fatura tarihi",
                    true,
                    state.errors.get(Field::Date).is_some(),
                ));
                ui.vertical(|ui| {
                    if ui
                        .add(
                            TextEdit::singleline(&mut state.draft.invoice_date)
                                .hint_text("2026-10-18")
                                .desired_width(140.0),
                        )
                        .changed()
                    {
                        state.errors.clear(Field::Date);
                    }
                    field_error(ui, &state.errors, Field::Date);
                });
                ui.end_row();

                // Vade.
                ui.label(field_label(
                    "Vade tarihi (isteğe bağlı)",
                    false,
                    state.errors.get(Field::DueDate).is_some(),
                ));
                ui.vertical(|ui| {
                    if ui
                        .add(
                            TextEdit::singleline(&mut state.draft.due_date)
                                .hint_text("2026-11-18")
                                .desired_width(140.0),
                        )
                        .changed()
                    {
                        state.errors.clear(Field::DueDate);
                    }
                    field_error(ui, &state.errors, Field::DueDate);
                });
                ui.end_row();

                // Cari.
                ui.label(field_label(
                    "Cari hesap",
                    true,
                    state.errors.get(Field::Customer).is_some(),
                ));
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        if let Some(id) = widgets::searchable_select(
                            ui,
                            "fatura_form_cari",
                            customer_label,
                            customer_options,
                            320.0,
                        ) {
                            state.draft.customer_id = Some(id);
                            state.errors.clear(Field::Customer);
                        }
                        if state.draft.customer_id.is_some() && ui.small_button("×").clicked() {
                            state.draft.customer_id = None;
                        }
                    });
                    field_error(ui, &state.errors, Field::Customer);
                });
                ui.end_row();

                // Notlar.
                ui.label(field_label(
                    "Notlar",
                    false,
                    state.errors.get(Field::Notes).is_some(),
                ));
                ui.vertical(|ui| {
                    if ui
                        .add(
                            TextEdit::multiline(&mut state.draft.notes)
                                .desired_width(340.0)
                                .desired_rows(2),
                        )
                        .changed()
                    {
                        state.errors.clear(Field::Notes);
                    }
                    field_error(ui, &state.errors, Field::Notes);
                });
                ui.end_row();
            });
    });
}

/// Fatura satırları düzenleyicisi.
fn lines_editor(
    ui: &mut Ui,
    state: &mut crate::app::InvoiceState,
    product_options: &[(i64, String)],
    product_labels: &[Option<String>],
    product_change: &mut Option<(usize, i64)>,
    action: &mut Option<Action>,
) {
    let line_count = state.draft.lines.len();

    widgets::card(ui, "Fatura satırları", |ui| {
        for index in 0..line_count {
            let selected = product_labels.get(index).cloned().flatten();
            line_editor(
                ui,
                index,
                &mut state.draft.lines[index],
                &mut state.errors,
                product_options,
                selected,
                product_change,
                action,
            );
            ui.add_space(8.0);
        }

        if line_count == 0 {
            ui.label(
                RichText::new("Henüz satır yok. \"Satır ekle\" ile başlayın.")
                    .color(Palette::MUTED),
            );
            ui.add_space(6.0);
        }

        if let Some(message) = state.errors.get(Field::Lines) {
            ui.label(RichText::new(message).size(12.0).color(Palette::DANGER));
            ui.add_space(4.0);
        }

        if ui.button("Satır ekle").clicked() {
            *action = Some(Action::AddLine);
        }
    });
}

/// Tek bir fatura satırı düzenleyicisi.
#[allow(clippy::too_many_arguments)]
fn line_editor(
    ui: &mut Ui,
    index: usize,
    line: &mut InvoiceLineDraft,
    errors: &mut FieldErrors,
    product_options: &[(i64, String)],
    selected_product: Option<String>,
    product_change: &mut Option<(usize, i64)>,
    action: &mut Option<Action>,
) {
    widgets::surface(ui, |ui| {
        Sides::new().height(26.0).shrink_left().show(
            ui,
            |ui| {
                ui.label(
                    RichText::new(format!("Satır {}", index + 1))
                        .strong()
                        .color(Palette::TEXT),
                );
                ui.add_space(6.0);
                let selected_text = selected_product
                    .clone()
                    .unwrap_or_else(|| "Ürün seçin".to_owned());
                if let Some(id) = widgets::searchable_select(
                    ui,
                    ("fatura_urun", index),
                    &selected_text,
                    product_options,
                    280.0,
                ) {
                    *product_change = Some((index, id));
                }
            },
            |ui| {
                if ui.small_button("Satırı sil").clicked() {
                    *action = Some(Action::RemoveLine(index));
                }
            },
        );

        ui.add_space(6.0);
        Grid::new(("fatura_satir", index))
            .num_columns(2)
            .min_col_width(130.0)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                // Salt okunur satır bilgileri.
                ui.label(RichText::new("Ürün kodu").size(13.0).color(Palette::MUTED));
                ui.label(
                    RichText::new(or_dash(&line.product_code))
                        .monospace()
                        .size(12.5)
                        .color(Palette::MUTED),
                );
                ui.end_row();

                ui.label(RichText::new("Birim").size(13.0).color(Palette::MUTED));
                ui.label(RichText::new(line.unit.short_label()).size(12.5));
                ui.end_row();

                // Miktar.
                ui.label(field_label(
                    "Miktar",
                    true,
                    errors.get_line(index, Field::LineQuantity).is_some(),
                ));
                ui.vertical(|ui| {
                    if ui
                        .add(
                            TextEdit::singleline(&mut line.quantity)
                                .hint_text("1")
                                .desired_width(120.0),
                        )
                        .changed()
                    {
                        errors.clear_line(index, Field::LineQuantity);
                    }
                    line_error(ui, errors, index, Field::LineQuantity);
                });
                ui.end_row();

                // Birim fiyat.
                ui.label(field_label(
                    "Birim fiyat (₺)",
                    true,
                    errors.get_line(index, Field::LineUnitPrice).is_some(),
                ));
                ui.vertical(|ui| {
                    if ui
                        .add(
                            TextEdit::singleline(&mut line.unit_price)
                                .hint_text("0,00")
                                .desired_width(140.0),
                        )
                        .changed()
                    {
                        errors.clear_line(index, Field::LineUnitPrice);
                    }
                    line_error(ui, errors, index, Field::LineUnitPrice);
                });
                ui.end_row();

                // İskonto.
                ui.label(field_label(
                    "İskonto %",
                    false,
                    errors.get_line(index, Field::LineDiscount).is_some(),
                ));
                ui.vertical(|ui| {
                    if ui
                        .add(
                            TextEdit::singleline(&mut line.discount_rate)
                                .hint_text("0")
                                .desired_width(90.0),
                        )
                        .changed()
                    {
                        errors.clear_line(index, Field::LineDiscount);
                    }
                    line_error(ui, errors, index, Field::LineDiscount);
                });
                ui.end_row();

                // KDV oranı (elle veya hazır seçeneklerle).
                ui.label(field_label(
                    "KDV %",
                    true,
                    errors.get_line(index, Field::LineVat).is_some(),
                ));
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        if ui
                            .add(
                                TextEdit::singleline(&mut line.vat_rate)
                                    .hint_text("20")
                                    .desired_width(70.0),
                            )
                            .changed()
                        {
                            errors.clear_line(index, Field::LineVat);
                        }
                        ComboBox::from_id_salt(("fatura_kdv", index))
                            .selected_text("Hızlı")
                            .width(80.0)
                            .show_ui(ui, |ui| {
                                for rate in VAT_RATES {
                                    if ui.selectable_label(false, format!("%{rate}")).clicked() {
                                        line.vat_rate = rate.to_string();
                                        errors.clear_line(index, Field::LineVat);
                                    }
                                }
                            });
                    });
                    line_error(ui, errors, index, Field::LineVat);
                });
                ui.end_row();

                // Satır net tutarı (salt okunur).
                ui.label(
                    RichText::new("Satır net (KDV hariç)")
                        .size(13.0)
                        .color(Palette::MUTED),
                );
                let net_text = line
                    .preview_amounts()
                    .map(|amounts| amounts.net.format_with_symbol())
                    .unwrap_or_else(|| "—".to_owned());
                ui.label(RichText::new(net_text).color(Palette::TEXT));
                ui.end_row();

                // Açıklama (isteğe bağlı).
                ui.label(field_label(
                    "Açıklama (isteğe bağlı)",
                    false,
                    errors.get_line(index, Field::LineDescription).is_some(),
                ));
                ui.vertical(|ui| {
                    if ui
                        .add(
                            TextEdit::singleline(&mut line.description)
                                .hint_text("Satır açıklaması")
                                .desired_width(320.0),
                        )
                        .changed()
                    {
                        errors.clear_line(index, Field::LineDescription);
                    }
                    line_error(ui, errors, index, Field::LineDescription);
                });
                ui.end_row();
            });
    });
}

/// Canlı toplamlar kartı.
fn totals_card(ui: &mut Ui, draft: &InvoiceDraft) {
    let totals = draft.preview_totals();
    widgets::card(ui, "Toplamlar", |ui| totals_grid(ui, &totals));
}

/// Toplam tablosu (form ve ayrıntı ekranlarında ortak).
fn totals_grid(ui: &mut Ui, totals: &InvoiceTotals) {
    Grid::new("fatura_toplamlar")
        .num_columns(2)
        .min_col_width(150.0)
        .spacing([16.0, 6.0])
        .show(ui, |ui| {
            money_row(ui, "Brüt toplam", totals.subtotal, false);
            money_row(ui, "İskonto", totals.discount_total, false);
            money_row(ui, "Matrah (KDV hariç)", totals.net_total, false);
            money_row(ui, "KDV", totals.vat_total, false);
            money_row(ui, "Genel toplam", totals.grand_total, true);
        });
}

// ---------------------------------------------------------------------------
// Ayrıntı ekranı
// ---------------------------------------------------------------------------

/// Fatura ayrıntısı.
fn detail(ui: &mut Ui, app: &mut OnMuhasebeApp) {
    let mut action: Option<Action> = None;

    Sides::new().height(46.0).shrink_left().show(
        ui,
        |ui| {
            ui.horizontal(|ui| {
                if ui.button("← Fatura listesi").clicked() {
                    action = Some(Action::BackToList);
                }
                ui.add_space(6.0);
                ui.add(
                    Label::new(
                        RichText::new("Fatura ayrıntısı")
                            .size(20.0)
                            .strong()
                            .color(Palette::TEXT),
                    )
                    .truncate(),
                );
            });
        },
        |_ui| {},
    );

    ui.add_space(metrics::SECTION_GAP);
    if messages(ui, app, &mut action) {
        ui.add_space(metrics::SECTION_GAP);
    }

    let invoice = app.invoice_state().selected.clone();
    let lines = app.invoice_state().selected_lines.clone();
    let cancelling = app.invoice_state().cancelling;

    let Some(invoice) = invoice else {
        widgets::notice_card(ui, "Fatura bulunamadı", "Kayıt okunamadı; listeye dönün.");
        if let Some(action) = action {
            action.apply(app);
        }
        return;
    };

    invoice_header(ui, &invoice, &mut action);
    ui.add_space(metrics::SECTION_GAP);
    invoice_lines_card(ui, &lines, &mut action);
    ui.add_space(metrics::SECTION_GAP);
    widgets::card(ui, "Toplamlar", |ui| totals_grid(ui, &invoice.totals));
    ui.add_space(metrics::SECTION_GAP);
    invoice_payment_card(ui, app, &invoice, &mut action);
    ui.add_space(metrics::SECTION_GAP);
    cancellation_card(ui, app, &invoice, cancelling, &mut action);

    if let Some(action) = action {
        action.apply(app);
    }
}

/// Faturanın üst bilgi kartı.
fn invoice_header(ui: &mut Ui, invoice: &Invoice, action: &mut Option<Action>) {
    widgets::card(ui, "Fatura bilgileri", |ui| {
        Sides::new().height(24.0).shrink_left().show(
            ui,
            |ui| {
                ui.label(
                    RichText::new(&invoice.number)
                        .monospace()
                        .size(13.5)
                        .color(Palette::ACCENT),
                );
                ui.label(
                    RichText::new(format!("{} faturası", invoice.invoice_type.label()))
                        .color(Palette::MUTED),
                );
            },
            |ui| status_badge(ui, invoice.status),
        );

        ui.add_space(8.0);

        Grid::new("fatura_detay_bilgi")
            .num_columns(2)
            .min_col_width(150.0)
            .spacing([16.0, 6.0])
            .show(ui, |ui| {
                widgets::row_label(ui, "Cari hesap");
                if widgets::link(
                    ui,
                    RichText::new(format!(
                        "{} — {}",
                        invoice.customer_code, invoice.customer_name
                    )),
                )
                .clicked()
                {
                    *action = Some(Action::OpenCustomer(invoice.customer_id));
                }
                ui.end_row();

                widgets::row_label(ui, "Fatura tarihi");
                ui.label(format_iso_date_short(&invoice.invoice_date));
                ui.end_row();

                widgets::row_label(ui, "Vade tarihi");
                ui.label(
                    invoice
                        .due_date
                        .as_deref()
                        .map(format_iso_date_short)
                        .unwrap_or_else(|| "—".to_owned()),
                );
                ui.end_row();

                widgets::row_label(ui, "Kayıt zamanı");
                ui.label(format!("{} (UTC)", invoice.created_at));
                ui.end_row();

                if let Some(notes) = &invoice.notes {
                    widgets::row_label(ui, "Notlar");
                    ui.add(Label::new(notes).wrap());
                    ui.end_row();
                }
            });
    });
}

/// Fatura satırları tablosu.
fn invoice_lines_card(ui: &mut Ui, lines: &[InvoiceLine], action: &mut Option<Action>) {
    widgets::card(ui, "Satırlar", |ui| {
        if lines.is_empty() {
            ui.label(RichText::new("Bu faturada satır bulunmuyor.").color(Palette::MUTED));
            return;
        }

        Grid::new("fatura_detay_satirlar")
            .num_columns(2)
            .min_col_width(150.0)
            .spacing([16.0, 6.0])
            .show(ui, |ui| {
                for line in lines {
                    widgets::row_label(ui, &format!("{}. satır", line.line_no));
                    ui.vertical(|ui| {
                        if widgets::link(
                            ui,
                            RichText::new(format!("{} — {}", line.product_code, line.product_name))
                                .strong(),
                        )
                        .clicked()
                        {
                            *action = Some(Action::OpenProduct(line.product_id));
                        }
                        ui.label(
                            RichText::new(format!(
                                "{} × {} · İskonto {} · KDV {}",
                                line.formatted_quantity(),
                                line.unit_price.format_with_symbol(),
                                line.discount_label(),
                                line.vat_label()
                            ))
                            .size(12.5)
                            .color(Palette::MUTED),
                        );
                        if let Some(description) = &line.description {
                            ui.label(RichText::new(description).size(12.0).color(Palette::MUTED));
                        }
                    });
                    ui.end_row();

                    widgets::row_label(ui, "Tutar");
                    ui.label(
                        RichText::new(line.amounts.total.format_with_symbol())
                            .strong()
                            .color(Palette::TEXT),
                    );
                    ui.end_row();
                }
            });
    });
}

/// Faturaya bağlı tahsilat/ödeme kartı: ödenen/kalan, bağlı kayıtlar ve hızlı eylem.
fn invoice_payment_card(
    ui: &mut Ui,
    app: &OnMuhasebeApp,
    invoice: &Invoice,
    action: &mut Option<Action>,
) {
    let links = if invoice.is_cancelled() {
        Vec::new()
    } else {
        app.invoice_payments(invoice.id)
    };

    widgets::card(ui, "Tahsilat / ödeme", |ui| {
        Grid::new("fatura_detay_odeme")
            .num_columns(2)
            .min_col_width(150.0)
            .spacing([16.0, 6.0])
            .show(ui, |ui| {
                widgets::row_label(ui, "Ödenen");
                ui.label(
                    RichText::new(invoice.paid_amount.format_with_symbol())
                        .strong()
                        .color(Palette::TEXT),
                );
                ui.end_row();

                widgets::row_label(ui, "Kalan");
                ui.label(
                    RichText::new(invoice.remaining().format_with_symbol())
                        .strong()
                        .color(Palette::TEXT),
                );
                ui.end_row();

                widgets::row_label(ui, "Durum");
                match invoice.payment_state() {
                    Some(state) => payment_state_badge(ui, state),
                    None => {
                        ui.label(RichText::new("—").color(Palette::MUTED));
                    }
                }
                ui.end_row();
            });

        if invoice.is_cancelled() {
            ui.add_space(8.0);
            widgets::bullet(ui, "İptal edilmiş faturaya tahsilat/ödeme bağlanamaz.");
            return;
        }

        ui.add_space(10.0);
        widgets::subheading(ui, "Bağlı kayıtlar");
        ui.add_space(4.0);
        cash::invoice_payment_links(ui, &links);

        ui.add_space(12.0);
        let label = match invoice.invoice_type {
            InvoiceType::Sales => "Tahsilat Al",
            InvoiceType::Purchase => "Ödeme Yap",
        };
        if ui
            .add(Button::new(label).min_size(vec2(150.0, 30.0)))
            .on_hover_text("Bu faturaya kalan tutar önceden dağıtılmış olarak açılır.")
            .clicked()
        {
            *action = Some(Action::OpenPayment(invoice.id));
        }
    });
}

/// İptal bölümü: iptal edilmişse bilgi, değilse iptal düğmesi/onayı.
fn cancellation_card(
    ui: &mut Ui,
    app: &mut OnMuhasebeApp,
    invoice: &Invoice,
    cancelling: bool,
    action: &mut Option<Action>,
) {
    if invoice.is_cancelled() {
        widgets::card(ui, "İptal bilgisi", |ui| {
            Grid::new("fatura_iptal_bilgi")
                .num_columns(2)
                .min_col_width(150.0)
                .spacing([16.0, 6.0])
                .show(ui, |ui| {
                    widgets::row_label(ui, "İptal zamanı");
                    ui.label(format!(
                        "{} (UTC)",
                        invoice.cancelled_at.as_deref().unwrap_or("—")
                    ));
                    ui.end_row();

                    widgets::row_label(ui, "İptal nedeni");
                    ui.add(Label::new(invoice.cancel_reason.as_deref().unwrap_or("—")).wrap());
                    ui.end_row();
                });
        });
        return;
    }

    if !app.is_manager() {
        return;
    }

    if !cancelling {
        if ui
            .add(Button::new("Fatura İptal Et").min_size(vec2(160.0, 30.0)))
            .on_hover_text("Fatura silinmez; iptal edilir ve stok/cari etkisi geri alınır.")
            .clicked()
        {
            *action = Some(Action::StartCancel);
        }
        return;
    }

    widgets::card(ui, "Faturayı iptal et", |ui| {
        ui.add(
            Label::new(
                RichText::new(
                    "Bu fatura iptal edilecek; stok ve cari bakiyesi geri alınacak. \
                     İptal nedeni zorunludur.",
                )
                .size(13.5)
                .color(Palette::TEXT),
            )
            .wrap(),
        );
        ui.add_space(6.0);

        let state = app.invoice_state_mut();
        ui.label(field_label(
            "İptal nedeni",
            true,
            state.cancel_error.is_some(),
        ));
        if ui
            .add(
                TextEdit::multiline(&mut state.cancel_reason)
                    .desired_width(420.0)
                    .desired_rows(2),
            )
            .changed()
        {
            state.cancel_error = None;
        }
        if let Some(message) = &state.cancel_error {
            ui.label(RichText::new(message).size(11.5).color(Palette::DANGER));
        }

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui
                .add(Button::new("İptali Onayla").min_size(vec2(150.0, 30.0)))
                .clicked()
            {
                *action = Some(Action::SubmitCancel);
            }
            if ui
                .add(Button::new("Vazgeç").min_size(vec2(120.0, 30.0)))
                .clicked()
            {
                *action = Some(Action::AbortCancel);
            }
        });
    });
}
