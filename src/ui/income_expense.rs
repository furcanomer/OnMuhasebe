// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Gelir/gider ekranları: hareketler, kategoriler ve dönem özeti.
//!
//! Bu modül **yalnızca çizim yapar**. Veriyi
//! [`OnMuhasebeApp::income_expense_state`] üzerinden okur ve kullanıcı
//! eylemlerini [`Action`] değerlerine dönüştürüp uygulama katmanının
//! komutlarına devreder. Hiçbir SQL bu dosyada bulunmaz.
//!
//! ## Sekmeler
//!
//! * **Hareketler** (varsayılan): gelir/gider listesi, filtreler ve alt toplam.
//! * **Kategoriler**: kategori listesi, filtreler ve yeni/düzenle formu.
//! * **Özet**: seçili dönem için toplamlar, kategori ve hesap dağılımı.
//!
//! ## Yerleşim notu
//!
//! Form ve filtre alanları `Grid`/`horizontal_wrapped` ile sarılır; uzun
//! metinler `Label::truncate()` ile sınırlandırılır. Böylece 900x600 gibi dar
//! pencerelerde yatay taşma oluşmaz.

use egui::{Button, ComboBox, Grid, Label, RichText, ScrollArea, Sides, TextEdit, Ui, vec2};

use crate::app::{FocusTarget, IncomeExpenseScreen, IncomeExpenseTab, OnMuhasebeApp};
use crate::income_expense::{
    CategoryField, CategoryFieldErrors, EntryField, EntryFieldErrors, EntryStatus, EntryType,
    ExpenseCategory, IncomeExpenseEntry,
};
use crate::money::Money;
use crate::ui::format::format_iso_date_short;
use crate::ui::theme::{Palette, metrics};
use crate::ui::{transfer, widgets};

/// Kullanıcı eylemleri; çizim sırasında toplanır ve sonra uygulanır.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    /// Sekme değiştirir.
    OpenTab(IncomeExpenseTab),
    /// Yeni gelir/gider formunu açar.
    NewEntry(EntryType),
    /// Hareket ayrıntısını açar.
    OpenEntry(i64),
    /// Hareket listesine döner.
    BackToEntryList,
    /// Hareket filtrelerini temizler.
    ResetEntryFilter,
    /// Formun türünü değiştirir.
    SetEntryType(EntryType),
    /// Formda kategoriyi seçer/kaldırır.
    SetEntryCategory(Option<i64>),
    /// Formda kasa/banka hesabını seçer/kaldırır.
    SetEntryAccount(Option<i64>),
    /// Formu doğrulayıp kaydetme onayı ister.
    SubmitEntry,
    /// Onaylanan kaydı kesinleştirir.
    ConfirmEntrySave,
    /// Kaydetme onayından vazgeçer.
    DiscardEntryConfirmation,
    /// İptal onay penceresini açar.
    StartEntryCancel,
    /// İptal onay penceresini kapatır.
    AbortEntryCancel,
    /// İptal nedenini doğrulayıp kaydı iptal eder.
    SubmitEntryCancel,
    /// Yeni kategori formunu açar.
    NewCategory,
    /// Kategori düzenleme formunu açar.
    EditCategory(i64),
    /// Kategoriyi pasife/aktife alır.
    ToggleCategory(i64),
    /// Kategori formunu doğrulayıp kaydeder.
    SubmitCategory,
    /// Kategori formunu kaydetmeden kapatır.
    CancelCategoryForm,
    /// Kategori filtrelerini temizler.
    ResetCategoryFilter,
    /// Yeni kategoride türü değiştirir.
    SetCategoryType(EntryType),
    /// Özet tarih aralığını uygular.
    ApplySummaryRange,
    /// Özet dönemini bu aya ayarlar.
    SummaryThisMonth,
    /// Bilgilendirme mesajını kapatır.
    DismissNotice,
    /// Gelir/gider hareket listesini CSV olarak dışa aktarır.
    ExportCsv,
    /// Kayıttaki kasa/banka hesabını listede vurgular.
    HighlightAccount(i64),
}

impl Action {
    /// Eylemi uygulama katmanında uygular.
    fn apply(self, app: &mut OnMuhasebeApp) {
        match self {
            Self::OpenTab(tab) => app.set_income_expense_tab(tab),
            Self::NewEntry(entry_type) => app.open_income_expense_entry_form(entry_type),
            Self::OpenEntry(id) => app.open_income_expense_detail(id),
            Self::BackToEntryList => app.back_from_income_expense_detail(),
            Self::ResetEntryFilter => app.reset_income_expense_filter(),
            Self::SetEntryType(entry_type) => app.set_income_expense_entry_type(entry_type),
            Self::SetEntryCategory(id) => app.set_income_expense_category(id),
            Self::SetEntryAccount(id) => app.set_income_expense_cash_account(id),
            Self::SubmitEntry => app.submit_income_expense_entry_form(),
            Self::ConfirmEntrySave => app.confirm_income_expense_entry_save(),
            Self::DiscardEntryConfirmation => app.discard_income_expense_entry_confirmation(),
            Self::StartEntryCancel => app.start_income_expense_cancel(),
            Self::AbortEntryCancel => app.abort_income_expense_cancel(),
            Self::SubmitEntryCancel => app.submit_income_expense_cancel(),
            Self::NewCategory => app.open_income_expense_category_new_form(),
            Self::EditCategory(id) => app.open_income_expense_category_edit_form(id),
            Self::ToggleCategory(id) => app.toggle_income_expense_category_active(id),
            Self::SubmitCategory => app.submit_income_expense_category_form(),
            Self::CancelCategoryForm => app.cancel_income_expense_category_form(),
            Self::ResetCategoryFilter => app.reset_income_expense_category_filter(),
            Self::SetCategoryType(entry_type) => {
                app.set_income_expense_category_form_type(entry_type);
            }
            Self::ApplySummaryRange => app.refresh_income_expense_summary(),
            Self::SummaryThisMonth => app.set_income_expense_summary_this_month(),
            Self::DismissNotice => app.dismiss_income_expense_notice(),
            Self::ExportCsv => {
                if let Some(path) =
                    transfer::save_csv_dialog(&OnMuhasebeApp::income_expense_list_csv_name())
                {
                    let path = OnMuhasebeApp::csv_path(path);
                    let _ = app.export_income_expense_list_csv(path);
                }
            }
            Self::HighlightAccount(id) => app.show_cash_account_highlight(id),
        }
    }
}

/// Aktif alt ekranı çizer.
pub fn show(ui: &mut Ui, app: &mut OnMuhasebeApp) {
    match app.income_expense_state().screen {
        IncomeExpenseScreen::EntryForm => entry_form(ui, app),
        IncomeExpenseScreen::EntryDetail => entry_detail(ui, app),
        IncomeExpenseScreen::CategoryForm => category_form(ui, app),
        IncomeExpenseScreen::EntryList | IncomeExpenseScreen::CategoryList => {
            match app.income_expense_state().tab {
                IncomeExpenseTab::Entries => entry_list(ui, app),
                IncomeExpenseTab::Categories => category_list(ui, app),
                IncomeExpenseTab::Summary => summary(ui, app),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Ortak parçalar
// ---------------------------------------------------------------------------

/// Bilgilendirme ve hata mesajlarını çizer; herhangi biri çizildiyse `true`.
fn messages(ui: &mut Ui, app: &OnMuhasebeApp, action: &mut Option<Action>) -> bool {
    let state = app.income_expense_state();
    let mut drawn = false;

    if let Some(notice) = &state.notice {
        if widgets::success_card(ui, "İşlem başarılı", notice) {
            *action = Some(Action::DismissNotice);
        }
        drawn = true;
    }

    for (title, message) in [
        ("İşlem tamamlanamadı", state.general_error.as_deref()),
        (
            "Gelir/gider kayıtları yüklenemedi",
            state.entry_list_error.as_deref(),
        ),
        (
            "Kategoriler yüklenemedi",
            state.category_list_error.as_deref(),
        ),
        ("Özet hesaplanamadı", state.summary_error.as_deref()),
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

/// Hareket formu alan hatası.
fn entry_field_error(ui: &mut Ui, errors: &EntryFieldErrors, field: EntryField) {
    if let Some(message) = errors.get(field) {
        ui.label(RichText::new(message).size(11.5).color(Palette::DANGER));
    }
}

/// Kategori formu alan hatası.
fn category_field_error(ui: &mut Ui, errors: &CategoryFieldErrors, field: CategoryField) {
    if let Some(message) = errors.get(field) {
        ui.label(RichText::new(message).size(11.5).color(Palette::DANGER));
    }
}

/// Gelir/gider türü etiketi.
fn entry_type_badge(ui: &mut Ui, entry_type: EntryType) {
    let color = match entry_type {
        EntryType::Gelir => Palette::SUCCESS,
        EntryType::Gider => Palette::DANGER,
    };
    ui.label(
        RichText::new(format!("● {}", entry_type.label()))
            .size(12.5)
            .strong()
            .color(color),
    );
}

/// Kayıt durumu etiketi.
fn entry_status_badge(ui: &mut Ui, status: EntryStatus) {
    let (color, text) = match status {
        EntryStatus::Kesin => (Palette::SUCCESS, "Kesin"),
        EntryStatus::Iptal => (Palette::DANGER, "İptal"),
    };
    ui.label(RichText::new(format!("● {text}")).size(12.5).color(color));
}

/// Kategori aktif/pasif etiketi.
fn category_status_badge(ui: &mut Ui, active: bool) {
    let (color, text) = if active {
        (Palette::SUCCESS, "Aktif")
    } else {
        (Palette::MUTED, "Pasif")
    };
    ui.label(RichText::new(format!("● {text}")).size(12.5).color(color));
}

/// Tutar rengi (negatif kırmızı, pozitif yeşil, sıfır nötr).
fn balance_color(amount: Money) -> egui::Color32 {
    if amount.kurus() > 0 {
        Palette::SUCCESS
    } else if amount.kurus() < 0 {
        Palette::DANGER
    } else {
        Palette::MUTED
    }
}

/// Sayfa başlığı (sol) ve isteğe bağlı eylem alanı (sağ).
fn page_header(ui: &mut Ui, title: &str, description: &str, right: impl FnOnce(&mut Ui)) {
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
        right,
    );
}

/// Bir alt ekran başlığı: geri dön düğmesi + başlık.
fn back_header(
    ui: &mut Ui,
    back: &str,
    title: &str,
    back_action: Action,
    action: &mut Option<Action>,
) {
    ui.horizontal(|ui| {
        if ui.button(back).clicked() {
            *action = Some(back_action);
        }
        ui.add_space(6.0);
        ui.add(
            Label::new(
                RichText::new(title)
                    .size(20.0)
                    .strong()
                    .color(Palette::TEXT),
            )
            .truncate(),
        );
    });
}

/// Sekme çubuğu (yalnızca liste ekranlarında).
fn tabs(ui: &mut Ui, app: &OnMuhasebeApp, action: &mut Option<Action>) {
    let current = app.income_expense_state().tab;
    ui.horizontal(|ui| {
        for tab in IncomeExpenseTab::ALL {
            if ui
                .selectable_label(current == tab, RichText::new(tab.label()).size(14.0))
                .clicked()
                && current != tab
            {
                *action = Some(Action::OpenTab(tab));
            }
        }
    });
}

/// Dar bir hücrede soldan hizalı, kısaltılmış etiket.
fn cell_label(ui: &mut Ui, text: &str, width: f32) {
    ui.add_sized(
        [width, 18.0],
        Label::new(RichText::new(text).size(12.5).color(Palette::TEXT)).truncate(),
    );
}

// ---------------------------------------------------------------------------
// Hareketler
// ---------------------------------------------------------------------------

/// Gelir/gider hareket listesi.
fn entry_list(ui: &mut Ui, app: &mut OnMuhasebeApp) {
    let mut action: Option<Action> = None;

    page_header(
        ui,
        "Gelir ve Gider",
        "Cari hesaba bağlı olmayan nakit giriş/çıkışları. Kayıtlar silinmez, iptal edilir.",
        |ui| {
            if ui
                .add(Button::new("Yeni Gelir").min_size(vec2(110.0, 30.0)))
                .clicked()
            {
                action = Some(Action::NewEntry(EntryType::Gelir));
            }
            if ui
                .add(Button::new("Yeni Gider").min_size(vec2(110.0, 30.0)))
                .clicked()
            {
                action = Some(Action::NewEntry(EntryType::Gider));
            }
        },
    );

    ui.add_space(6.0);
    tabs(ui, app, &mut action);
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

    entry_filter_bar(ui, app, &mut action);
    ui.add_space(10.0);

    let state = app.income_expense_state();
    let entries = state.entry_list.clone();
    let summary = state.entry_summary;
    let error = state.entry_list_error.is_some();

    ui.label(
        RichText::new(format!("{} kayıt görüntüleniyor", entries.len()))
            .size(12.5)
            .color(Palette::MUTED),
    );
    ui.add_space(6.0);

    if !error {
        if entries.is_empty() {
            let message = if app.income_expense_state().entry_filter.is_empty() {
                "Henüz gelir/gider kaydı yok. \"Yeni Gelir\" veya \"Yeni Gider\" ile başlayın."
            } else {
                "Arama ve filtre ölçütlerine uyan kayıt bulunamadı."
            };
            widgets::card(ui, "Kayıt yok", |ui| {
                ui.add(Label::new(RichText::new(message).color(Palette::MUTED)).wrap());
            });
        } else {
            ScrollArea::vertical()
                .id_salt("gelir_gider_listesi")
                .auto_shrink([false, false])
                .max_height(430.0)
                .show_rows(ui, 92.0, entries.len(), |ui, range| {
                    for index in range {
                        entry_row(ui, &entries[index], app.is_manager(), &mut action);
                        ui.add_space(6.0);
                    }
                });
        }
    }

    ui.add_space(8.0);
    widgets::card(ui, "Özet (iptaller hariç)", |ui| {
        Grid::new("gelir_gider_hareket_ozeti")
            .num_columns(2)
            .min_col_width(150.0)
            .spacing([16.0, 6.0])
            .show(ui, |ui| {
                widgets::row_label(ui, "Toplam gelir");
                ui.label(
                    RichText::new(summary.income_total.format_with_symbol())
                        .strong()
                        .color(Palette::SUCCESS),
                );
                ui.end_row();

                widgets::row_label(ui, "Toplam gider");
                ui.label(
                    RichText::new(summary.expense_total.format_with_symbol())
                        .strong()
                        .color(Palette::DANGER),
                );
                ui.end_row();

                widgets::row_label(ui, "Net");
                ui.label(
                    RichText::new(summary.net().format_with_symbol())
                        .size(15.0)
                        .strong()
                        .color(balance_color(summary.net())),
                );
                ui.end_row();
            });
    });

    if let Some(action) = action {
        action.apply(app);
    }
}

/// Hareket arama/filtre çubuğu.
fn entry_filter_bar(ui: &mut Ui, app: &mut OnMuhasebeApp, action: &mut Option<Action>) {
    let before = app.income_expense_state().entry_filter.clone();
    let category_label = app
        .income_expense_state()
        .entry_filter
        .category_id
        .and_then(|id| app.income_expense_category_label(id))
        .unwrap_or_else(|| "Tümü".to_owned());
    let category_options = app.income_expense_filter_category_options();
    let account_label = app
        .income_expense_state()
        .entry_filter
        .cash_account_id
        .and_then(|id| app.income_expense_account_label(id))
        .unwrap_or_else(|| "Tümü".to_owned());
    let account_options = app.income_expense_filter_account_options();

    {
        let state = app.income_expense_state_mut();
        let filter = &mut state.entry_filter;

        widgets::card(ui, "Ara ve filtrele", |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Ara:").color(Palette::MUTED));
                ui.add(
                    TextEdit::singleline(&mut filter.search)
                        .hint_text("Açıklama, kategori veya belge no")
                        .desired_width(190.0),
                );

                ui.add_space(12.0);
                ui.label(RichText::new("Tür:").color(Palette::MUTED));
                ComboBox::from_id_salt("gelir_gider_tur_filtresi")
                    .selected_text(match filter.entry_type {
                        None => "Tümü",
                        Some(entry_type) => entry_type.label(),
                    })
                    .width(100.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut filter.entry_type, None, "Tümü");
                        for entry_type in EntryType::ALL {
                            ui.selectable_value(
                                &mut filter.entry_type,
                                Some(entry_type),
                                entry_type.label(),
                            );
                        }
                    });

                ui.add_space(12.0);
                ui.label(RichText::new("Kategori:").color(Palette::MUTED));
                if let Some(id) = widgets::searchable_select(
                    ui,
                    "gelir_gider_kategori_filtresi",
                    &category_label,
                    &category_options,
                    180.0,
                ) {
                    filter.category_id = Some(id);
                }
                if filter.category_id.is_some() && ui.small_button("×").clicked() {
                    filter.category_id = None;
                }

                ui.add_space(12.0);
                ui.label(RichText::new("Hesap:").color(Palette::MUTED));
                if let Some(id) = widgets::searchable_select(
                    ui,
                    "gelir_gider_hesap_filtresi",
                    &account_label,
                    &account_options,
                    170.0,
                ) {
                    filter.cash_account_id = Some(id);
                }
                if filter.cash_account_id.is_some() && ui.small_button("×").clicked() {
                    filter.cash_account_id = None;
                }

                ui.add_space(12.0);
                ui.label(RichText::new("Durum:").color(Palette::MUTED));
                ComboBox::from_id_salt("gelir_gider_durum_filtresi")
                    .selected_text(match filter.status {
                        None => "Tümü",
                        Some(status) => status.label(),
                    })
                    .width(100.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut filter.status, None, "Tümü");
                        for status in EntryStatus::ALL {
                            ui.selectable_value(&mut filter.status, Some(status), status.label());
                        }
                    });

                ui.add_space(12.0);
                ui.label(RichText::new("Tarih:").color(Palette::MUTED));
                ui.add(
                    TextEdit::singleline(&mut filter.date_from)
                        .hint_text("Başlangıç")
                        .desired_width(105.0),
                );
                ui.label(RichText::new("–").color(Palette::MUTED));
                ui.add(
                    TextEdit::singleline(&mut filter.date_to)
                        .hint_text("Bitiş")
                        .desired_width(105.0),
                );

                ui.add_space(12.0);
                if ui.button("Filtreleri temizle").clicked() {
                    *action = Some(Action::ResetEntryFilter);
                }
            });
        });
    }

    if app.income_expense_state().entry_filter != before {
        app.refresh_income_expense_list();
    }
}

/// Tek bir gelir/gider satırı.
fn entry_row(
    ui: &mut Ui,
    entry: &IncomeExpenseEntry,
    can_cancel: bool,
    action: &mut Option<Action>,
) {
    let fill = if entry.is_cancelled() {
        Palette::BACKGROUND
    } else {
        Palette::SURFACE
    };
    let type_color = match entry.entry_type {
        EntryType::Gelir => Palette::SUCCESS,
        EntryType::Gider => Palette::DANGER,
    };

    widgets::surface_tinted(ui, fill, Palette::BORDER, |ui| {
        Sides::new().height(24.0).shrink_left().show(
            ui,
            |ui| {
                ui.label(
                    RichText::new(format_iso_date_short(&entry.entry_date))
                        .monospace()
                        .size(12.5)
                        .color(Palette::MUTED),
                );
                ui.label(
                    RichText::new(entry.entry_type.label())
                        .size(12.5)
                        .strong()
                        .color(type_color),
                );
                ui.add(
                    Label::new(
                        RichText::new(&entry.category_name)
                            .size(13.5)
                            .color(Palette::TEXT),
                    )
                    .truncate(),
                );
            },
            |ui| {
                ui.label(
                    RichText::new(entry.amount.format_with_symbol())
                        .size(14.0)
                        .strong()
                        .color(if entry.is_cancelled() {
                            Palette::MUTED
                        } else {
                            amount_color(entry.entry_type)
                        }),
                );
                ui.add_space(8.0);
                entry_status_badge(ui, entry.status);
            },
        );

        ui.add_space(2.0);

        Sides::new().height(24.0).shrink_left().show(
            ui,
            |ui| {
                ui.label(
                    RichText::new(&entry.cash_account_name)
                        .size(12.5)
                        .color(Palette::MUTED),
                );
                if let Some(document_ref) = &entry.document_ref {
                    ui.label(RichText::new("·").size(12.5).color(Palette::MUTED));
                    ui.add(
                        Label::new(
                            RichText::new(format!("Belge: {document_ref}"))
                                .size(12.0)
                                .color(Palette::MUTED),
                        )
                        .truncate(),
                    );
                }
                if let Some(description) = &entry.description {
                    ui.label(RichText::new("·").size(12.5).color(Palette::MUTED));
                    ui.add(
                        Label::new(RichText::new(description).size(12.0).color(Palette::MUTED))
                            .truncate(),
                    );
                }
            },
            |ui| {
                if ui.button("Ayrıntı").clicked() {
                    *action = Some(Action::OpenEntry(entry.id));
                }
                if can_cancel
                    && entry.status == EntryStatus::Kesin
                    && ui.small_button("İptal et").clicked()
                {
                    *action = Some(Action::OpenEntry(entry.id));
                }
            },
        );
    });
}

/// Gelir yeşil, gider kırmızı.
fn amount_color(entry_type: EntryType) -> egui::Color32 {
    match entry_type {
        EntryType::Gelir => Palette::SUCCESS,
        EntryType::Gider => Palette::DANGER,
    }
}

// ---------------------------------------------------------------------------
// Hareket formu ve ayrıntısı
// ---------------------------------------------------------------------------

/// Yeni gelir/gider formu.
fn entry_form(ui: &mut Ui, app: &mut OnMuhasebeApp) {
    let mut action: Option<Action> = None;

    let entry_type = app.income_expense_state().entry_draft.entry_type;
    back_header(
        ui,
        "← Hareketler",
        entry_type.new_label(),
        Action::BackToEntryList,
        &mut action,
    );

    ui.add_space(metrics::SECTION_GAP);
    if messages(ui, app, &mut action) {
        ui.add_space(metrics::SECTION_GAP);
    }

    let error_count = app.income_expense_state().entry_errors.len();
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

    // Kaydetme onayı: form dondurulur.
    if app.income_expense_state().entry_pending.is_some() {
        if let Some(action_value) = entry_confirmation(ui, app) {
            action = Some(action_value);
        }
        if let Some(action) = action {
            action.apply(app);
        }
        return;
    }

    let category_options = app.income_expense_entry_category_options();
    let category_label = app
        .income_expense_state()
        .entry_draft
        .category_id
        .and_then(|id| {
            category_options
                .iter()
                .find(|(option_id, _)| *option_id == id)
                .map(|(_, name)| name.clone())
        })
        .unwrap_or_else(|| "Kategori seçin".to_owned());
    let account_options = app.income_expense_entry_account_options();
    let account_label = app
        .income_expense_state()
        .entry_draft
        .cash_account_id
        .and_then(|id| {
            account_options
                .iter()
                .find(|(option_id, _)| *option_id == id)
                .map(|(_, name)| name.clone())
        })
        .unwrap_or_else(|| "Hesap seçin".to_owned());
    let selected_account_balance = app
        .income_expense_state()
        .entry_draft
        .cash_account_id
        .and_then(|id| {
            app.income_expense_state()
                .accounts
                .iter()
                .find(|account| account.id == id)
                .map(|account| account.current_balance)
        });
    let preview = app.income_expense_preview_balance();

    let focus_category = app.take_focus(FocusTarget::EntryCategory);
    let focus_amount = app.take_focus(FocusTarget::EntryAmount) || focus_category;

    widgets::card(ui, "Gelir/gider bilgileri", |ui| {
        let state = app.income_expense_state_mut();
        let draft = &mut state.entry_draft;
        let errors = &state.entry_errors;

        Grid::new("gelir_gider_formu")
            .num_columns(2)
            .min_col_width(150.0)
            .spacing([16.0, 10.0])
            .show(ui, |ui| {
                // Tür (yalnızca oluştururken; bu form hiçbir zaman düzenleme yapmaz).
                ui.label(field_label("Tür", true, false));
                let mut chosen = draft.entry_type;
                ui.horizontal(|ui| {
                    for option in EntryType::ALL {
                        ui.selectable_value(&mut chosen, option, option.label());
                    }
                });
                if chosen != draft.entry_type {
                    action = Some(Action::SetEntryType(chosen));
                }
                ui.end_row();

                // Kategori.
                ui.label(field_label(
                    "Kategori",
                    true,
                    errors.get(EntryField::Category).is_some(),
                ));
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        if let Some(id) = widgets::searchable_select(
                            ui,
                            "gelir_gider_kategori",
                            &category_label,
                            &category_options,
                            300.0,
                        ) {
                            action = Some(Action::SetEntryCategory(Some(id)));
                        }
                        if draft.category_id.is_some() && ui.small_button("×").clicked() {
                            action = Some(Action::SetEntryCategory(None));
                        }
                    });
                    if category_options.is_empty() {
                        ui.label(
                            RichText::new(
                                "Bu türde aktif kategori yok. Önce Kategoriler sekmesinden ekleyin.",
                            )
                            .size(11.5)
                            .color(Palette::MUTED),
                        );
                    }
                    entry_field_error(ui, errors, EntryField::Category);
                });
                ui.end_row();

                // Kasa/banka hesabı.
                ui.label(field_label(
                    "Kasa/banka hesabı",
                    true,
                    errors.get(EntryField::CashAccount).is_some(),
                ));
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        if let Some(id) = widgets::searchable_select(
                            ui,
                            "gelir_gider_hesap",
                            &account_label,
                            &account_options,
                            300.0,
                        ) {
                            action = Some(Action::SetEntryAccount(Some(id)));
                        }
                        if draft.cash_account_id.is_some() && ui.small_button("×").clicked() {
                            action = Some(Action::SetEntryAccount(None));
                        }
                    });
                    if account_options.is_empty() {
                        ui.label(
                            RichText::new(
                                "Aktif kasa/banka hesabı yok. Önce Kasa ve Banka bölümünden ekleyin.",
                            )
                            .size(11.5)
                            .color(Palette::MUTED),
                        );
                    }
                    entry_field_error(ui, errors, EntryField::CashAccount);
                });
                ui.end_row();

                // Tarih.
                ui.label(field_label(
                    "Tarih",
                    true,
                    errors.get(EntryField::Date).is_some(),
                ));
                ui.vertical(|ui| {
                    ui.add(
                        TextEdit::singleline(&mut draft.entry_date)
                            .hint_text("YYYY-MM-DD")
                            .desired_width(140.0),
                    );
                    entry_field_error(ui, errors, EntryField::Date);
                });
                ui.end_row();

                // Tutar.
                ui.label(field_label(
                    "Tutar",
                    true,
                    errors.get(EntryField::Amount).is_some(),
                ));
                ui.vertical(|ui| {
                    let response = ui.add(
                        TextEdit::singleline(&mut draft.amount)
                            .hint_text("Örnek: 2.500,00")
                            .desired_width(180.0),
                    );
                    if focus_amount {
                        response.request_focus();
                    }
                    entry_field_error(ui, errors, EntryField::Amount);
                });
                ui.end_row();

                // Belge/fiş no.
                ui.label(field_label(
                    "Belge/fiş no",
                    false,
                    errors.get(EntryField::DocumentRef).is_some(),
                ));
                ui.vertical(|ui| {
                    ui.add(
                        TextEdit::singleline(&mut draft.document_ref)
                            .hint_text("İsteğe bağlı")
                            .desired_width(240.0),
                    );
                    entry_field_error(ui, errors, EntryField::DocumentRef);
                });
                ui.end_row();

                // Açıklama.
                ui.label(field_label(
                    "Açıklama",
                    false,
                    errors.get(EntryField::Description).is_some(),
                ));
                ui.vertical(|ui| {
                    ui.add(
                        TextEdit::multiline(&mut draft.description)
                            .hint_text("İsteğe bağlı")
                            .desired_width(340.0)
                            .desired_rows(2),
                    );
                    entry_field_error(ui, errors, EntryField::Description);
                });
                ui.end_row();
            });
    });

    ui.add_space(8.0);
    widgets::card(ui, "Kasa/banka etkisi", |ui| {
        Grid::new("gelir_gider_bakiye_onizleme")
            .num_columns(2)
            .min_col_width(170.0)
            .spacing([16.0, 6.0])
            .show(ui, |ui| {
                widgets::row_label(ui, "Mevcut bakiye");
                ui.label(match selected_account_balance {
                    Some(balance) => balance.format_with_symbol(),
                    None => "—".to_owned(),
                });
                ui.end_row();

                widgets::row_label(ui, "Yeni kasa/banka bakiyesi");
                ui.label(
                    RichText::new(match preview {
                        Some(balance) => balance.format_with_symbol(),
                        None => "—".to_owned(),
                    })
                    .strong()
                    .color(match preview {
                        Some(balance) => balance_color(balance),
                        None => Palette::MUTED,
                    }),
                );
                ui.end_row();
            });
        ui.add_space(4.0);
        ui.add(
            Label::new(
                RichText::new("Bakiye eksiye düşebilir; bu durum engellenmez.")
                    .size(11.5)
                    .color(Palette::MUTED),
            )
            .wrap(),
        );
    });

    ui.add_space(10.0);
    ui.horizontal(|ui| {
        if ui
            .add(Button::new(entry_type.save_label()).min_size(vec2(150.0, 30.0)))
            .on_hover_text("Ctrl+S")
            .clicked()
        {
            action = Some(Action::SubmitEntry);
        }
        if ui.button("İptal").on_hover_text("Esc").clicked() {
            action = Some(Action::BackToEntryList);
        }
    });

    if let Some(action) = action {
        action.apply(app);
    }
}

/// Kaydetme onayı kartı.
fn entry_confirmation(ui: &mut Ui, app: &OnMuhasebeApp) -> Option<Action> {
    let Some(entry) = &app.income_expense_state().entry_pending else {
        return None;
    };
    let mut action = None;

    widgets::card(ui, "Kaydı onayla", |ui| {
        ui.add(
            Label::new(
                RichText::new(
                    "Gelir/Gider kaydı kesinleşecek. Kasa/banka bakiyesi güncellenecek. \
                     Onaylıyor musunuz?",
                )
                .size(13.5)
                .color(Palette::TEXT),
            )
            .wrap(),
        );
        ui.add_space(8.0);
        Grid::new("gelir_gider_onay_ozeti")
            .num_columns(2)
            .min_col_width(150.0)
            .spacing([16.0, 5.0])
            .show(ui, |ui| {
                widgets::row_label(ui, "Tür");
                ui.label(entry.entry_type.label());
                ui.end_row();
                widgets::row_label(ui, "Tarih");
                ui.label(format_iso_date_short(&entry.entry_date));
                ui.end_row();
                widgets::row_label(ui, "Tutar");
                ui.label(entry.amount.format_with_symbol());
                ui.end_row();
            });
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui
                .add(Button::new("Onayla ve Kaydet").min_size(vec2(150.0, 30.0)))
                .clicked()
            {
                action = Some(Action::ConfirmEntrySave);
            }
            if ui.button("Vazgeç").clicked() {
                action = Some(Action::DiscardEntryConfirmation);
            }
        });
    });

    action
}

/// Gelir/gider ayrıntısı.
fn entry_detail(ui: &mut Ui, app: &mut OnMuhasebeApp) {
    let mut action: Option<Action> = None;

    let Some(entry) = app.income_expense_state().entry_detail.clone() else {
        widgets::error_card(ui, "Kayıt bulunamadı", "Gösterilecek kayıt yok.");
        if ui.button("Listeye dön").clicked() {
            action = Some(Action::BackToEntryList);
        }
        if let Some(action) = action {
            action.apply(app);
        }
        return;
    };

    back_header(
        ui,
        "← Listeye dön",
        &format!("{} Ayrıntısı", entry.entry_type.label()),
        Action::BackToEntryList,
        &mut action,
    );

    ui.add_space(metrics::SECTION_GAP);
    if messages(ui, app, &mut action) {
        ui.add_space(metrics::SECTION_GAP);
    }

    widgets::card(ui, "Kayıt bilgileri", |ui| {
        Sides::new().height(24.0).shrink_left().show(
            ui,
            |ui| {
                entry_type_badge(ui, entry.entry_type);
                ui.label(
                    RichText::new(format_iso_date_short(&entry.entry_date))
                        .monospace()
                        .size(13.0)
                        .color(Palette::MUTED),
                );
            },
            |ui| entry_status_badge(ui, entry.status),
        );
        ui.add_space(8.0);

        Grid::new("gelir_gider_ayrinti")
            .num_columns(2)
            .min_col_width(150.0)
            .spacing([16.0, 7.0])
            .show(ui, |ui| {
                for (label, value) in [
                    ("Tür", entry.entry_type.label().to_owned()),
                    ("Tarih", format_iso_date_short(&entry.entry_date)),
                    ("Kategori", entry.category_name.clone()),
                    ("Kasa/banka", entry.cash_account_name.clone()),
                    ("Tutar", entry.amount.format_with_symbol()),
                    (
                        "Belge/fiş no",
                        entry.document_ref.clone().unwrap_or_else(|| "—".to_owned()),
                    ),
                    (
                        "Açıklama",
                        entry.description.clone().unwrap_or_else(|| "—".to_owned()),
                    ),
                    ("Durum", entry.status.label().to_owned()),
                ] {
                    widgets::row_label(ui, label);
                    if label == "Kasa/banka" {
                        if widgets::link(ui, RichText::new(value)).clicked() {
                            action = Some(Action::HighlightAccount(entry.cash_account_id));
                        }
                    } else {
                        ui.add(Label::new(RichText::new(value).color(Palette::TEXT)).wrap());
                    }
                    ui.end_row();
                }
            });
    });

    if entry.is_cancelled() {
        ui.add_space(8.0);
        widgets::card(ui, "İptal bilgisi", |ui| {
            Grid::new("gelir_gider_iptal_bilgisi")
                .num_columns(2)
                .min_col_width(150.0)
                .spacing([16.0, 6.0])
                .show(ui, |ui| {
                    widgets::row_label(ui, "İptal zamanı");
                    ui.label(entry.cancelled_at.clone().unwrap_or_else(|| "—".to_owned()));
                    ui.end_row();
                    widgets::row_label(ui, "İptal nedeni");
                    ui.add(
                        Label::new(
                            RichText::new(
                                entry
                                    .cancel_reason
                                    .clone()
                                    .unwrap_or_else(|| "—".to_owned()),
                            )
                            .color(Palette::TEXT),
                        )
                        .wrap(),
                    );
                    ui.end_row();
                });
        });
    }

    ui.add_space(8.0);
    widgets::card(ui, "İlişkili kayıtlar", |ui| {
        ui.add(
            Label::new(
                RichText::new(
                    "Bu kayıt cari hesap veya fatura ile ilişkili değildir. Cari bakiyesine ve \
                     fatura ödenen tutarına etkisi yoktur.",
                )
                .size(12.5)
                .color(Palette::MUTED),
            )
            .wrap(),
        );
    });

    if app.is_manager() && app.income_expense_state().cancelling {
        ui.add_space(8.0);
        widgets::card(ui, "İptal onayı", |ui| {
            ui.add(
                Label::new(
                    RichText::new(
                        "İptal, kasa/banka bakiyesine olan etkiyi geri alır. Kayıt silinmez, \
                         listede iptal olarak kalır. İptal nedeni zorunludur.",
                    )
                    .size(12.5)
                    .color(Palette::TEXT),
                )
                .wrap(),
            );
            ui.add_space(6.0);
            ui.add(
                TextEdit::singleline(&mut app.income_expense_state_mut().cancel_reason)
                    .hint_text("İptal nedeni")
                    .desired_width(340.0),
            );
            if let Some(error) = &app.income_expense_state().cancel_error {
                ui.label(RichText::new(error).size(11.5).color(Palette::DANGER));
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui
                    .add(Button::new("İptal işlemini onayla").min_size(vec2(160.0, 30.0)))
                    .clicked()
                {
                    action = Some(Action::SubmitEntryCancel);
                }
                if ui.button("Vazgeç").clicked() {
                    action = Some(Action::AbortEntryCancel);
                }
            });
        });
    } else {
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if app.is_manager()
                && !entry.is_cancelled()
                && ui
                    .add(Button::new("İptal et").min_size(vec2(110.0, 30.0)))
                    .clicked()
            {
                action = Some(Action::StartEntryCancel);
            }
            if ui.button("Listeye dön").clicked() {
                action = Some(Action::BackToEntryList);
            }
        });
    }

    if let Some(action) = action {
        action.apply(app);
    }
}

// ---------------------------------------------------------------------------
// Kategoriler
// ---------------------------------------------------------------------------

/// Kategori listesi.
fn category_list(ui: &mut Ui, app: &mut OnMuhasebeApp) {
    let mut action: Option<Action> = None;

    page_header(
        ui,
        "Gelir ve Gider",
        "Kategoriler silinmez, pasife alınır. Pasif kategoriler yeni kayıtlarda seçilemez.",
        |ui| {
            if ui
                .add(Button::new("Yeni kategori").min_size(vec2(140.0, 30.0)))
                .clicked()
            {
                action = Some(Action::NewCategory);
            }
        },
    );

    ui.add_space(6.0);
    tabs(ui, app, &mut action);
    ui.add_space(metrics::SECTION_GAP);

    if messages(ui, app, &mut action) {
        ui.add_space(metrics::SECTION_GAP);
    }

    category_filter_bar(ui, app, &mut action);
    ui.add_space(10.0);

    let state = app.income_expense_state();
    let categories = state.category_list.clone();
    let error = state.category_list_error.is_some();

    ui.label(
        RichText::new(format!("{} kategori görüntüleniyor", categories.len()))
            .size(12.5)
            .color(Palette::MUTED),
    );
    ui.add_space(6.0);

    if !error {
        if categories.is_empty() {
            let message = if app.income_expense_state().category_filter.is_empty() {
                "Henüz gelir/gider kategorisi yok. \"Yeni kategori\" ile başlayın."
            } else {
                "Arama ve filtre ölçütlerine uyan kategori bulunamadı."
            };
            widgets::card(ui, "Kategori yok", |ui| {
                ui.add(Label::new(RichText::new(message).color(Palette::MUTED)).wrap());
            });
        } else {
            ScrollArea::vertical()
                .id_salt("kategori_listesi")
                .auto_shrink([false, false])
                .max_height(400.0)
                .show_rows(ui, 86.0, categories.len(), |ui, range| {
                    for index in range {
                        category_row(ui, &categories[index], app.is_manager(), &mut action);
                        ui.add_space(6.0);
                    }
                });
        }
    }

    if let Some(action) = action {
        action.apply(app);
    }
}

/// Kategori arama/filtre çubuğu.
fn category_filter_bar(ui: &mut Ui, app: &mut OnMuhasebeApp, action: &mut Option<Action>) {
    let before = app.income_expense_state().category_filter.clone();

    {
        let state = app.income_expense_state_mut();
        let filter = &mut state.category_filter;

        widgets::card(ui, "Ara ve filtrele", |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Ara:").color(Palette::MUTED));
                ui.add(
                    TextEdit::singleline(&mut filter.search)
                        .hint_text("Kategori adı veya açıklama")
                        .desired_width(200.0),
                );

                ui.add_space(12.0);
                ui.label(RichText::new("Tür:").color(Palette::MUTED));
                ComboBox::from_id_salt("kategori_tur_filtresi")
                    .selected_text(match filter.category_type {
                        None => "Tümü",
                        Some(category_type) => category_type.label(),
                    })
                    .width(100.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut filter.category_type, None, "Tümü");
                        for category_type in EntryType::ALL {
                            ui.selectable_value(
                                &mut filter.category_type,
                                Some(category_type),
                                category_type.label(),
                            );
                        }
                    });

                ui.add_space(12.0);
                ui.label(RichText::new("Durum:").color(Palette::MUTED));
                ComboBox::from_id_salt("kategori_durum_filtresi")
                    .selected_text(match filter.active {
                        None => "Tümü",
                        Some(true) => "Aktif",
                        Some(false) => "Pasif",
                    })
                    .width(100.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut filter.active, None, "Tümü");
                        ui.selectable_value(&mut filter.active, Some(true), "Aktif");
                        ui.selectable_value(&mut filter.active, Some(false), "Pasif");
                    });

                ui.add_space(12.0);
                if ui.button("Filtreleri temizle").clicked() {
                    *action = Some(Action::ResetCategoryFilter);
                }
            });
        });
    }

    if app.income_expense_state().category_filter != before {
        app.refresh_income_expense_category_list();
    }
}

/// Tek bir kategori satırı.
fn category_row(
    ui: &mut Ui,
    category: &ExpenseCategory,
    can_manage: bool,
    action: &mut Option<Action>,
) {
    let fill = if category.active {
        Palette::SURFACE
    } else {
        Palette::BACKGROUND
    };

    widgets::surface_tinted(ui, fill, Palette::BORDER, |ui| {
        Sides::new().height(24.0).shrink_left().show(
            ui,
            |ui| {
                ui.add(
                    Label::new(
                        RichText::new(&category.name)
                            .size(14.0)
                            .strong()
                            .color(Palette::TEXT),
                    )
                    .truncate(),
                );
                entry_type_badge(ui, category.category_type);
            },
            |ui| {
                ui.label(
                    RichText::new(format!("{} kullanım", category.usage_count))
                        .size(12.0)
                        .color(Palette::MUTED),
                );
                ui.add_space(8.0);
                category_status_badge(ui, category.active);
            },
        );

        ui.add_space(2.0);

        Sides::new().height(24.0).shrink_left().show(
            ui,
            |ui| {
                if let Some(description) = &category.description {
                    ui.add(
                        Label::new(RichText::new(description).size(12.0).color(Palette::MUTED))
                            .truncate(),
                    );
                } else {
                    ui.label(
                        RichText::new("Açıklama yok")
                            .size(12.0)
                            .color(Palette::MUTED),
                    );
                }
            },
            |ui| {
                if ui.button("Düzenle").clicked() {
                    *action = Some(Action::EditCategory(category.id));
                }
                let toggle_label = if category.active {
                    "Pasife al"
                } else {
                    "Aktife al"
                };
                if can_manage && ui.small_button(toggle_label).clicked() {
                    *action = Some(Action::ToggleCategory(category.id));
                }
            },
        );
    });
}

/// Yeni/düzenle kategori formu.
fn category_form(ui: &mut Ui, app: &mut OnMuhasebeApp) {
    let mut action: Option<Action> = None;
    let editing = app.income_expense_state().category_form_id.is_some();
    let title = if editing {
        "Kategoriyi Düzenle"
    } else {
        "Yeni Kategori"
    };

    back_header(
        ui,
        "← Kategoriler",
        title,
        Action::CancelCategoryForm,
        &mut action,
    );
    ui.add_space(metrics::SECTION_GAP);

    if messages(ui, app, &mut action) {
        ui.add_space(metrics::SECTION_GAP);
    }

    let error_count = app.income_expense_state().category_errors.len();
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

    let focus_name = app.take_focus(FocusTarget::CategoryName);

    widgets::card(ui, "Kategori bilgileri", |ui| {
        let can_manage = app.is_manager();
        let state = app.income_expense_state_mut();
        let draft = &mut state.category_draft;
        let errors = &state.category_errors;

        Grid::new("kategori_formu")
            .num_columns(2)
            .min_col_width(150.0)
            .spacing([16.0, 10.0])
            .show(ui, |ui| {
                ui.label(field_label(
                    "Ad",
                    true,
                    errors.get(CategoryField::Name).is_some(),
                ));
                ui.vertical(|ui| {
                    let response = ui.add(
                        TextEdit::singleline(&mut draft.name)
                            .hint_text("Örnek: Kira")
                            .desired_width(300.0),
                    );
                    if focus_name {
                        response.request_focus();
                    }
                    category_field_error(ui, errors, CategoryField::Name);
                });
                ui.end_row();

                ui.label(field_label("Tür", true, false));
                ui.vertical(|ui| {
                    if editing {
                        ui.label(
                            RichText::new(draft.category_type.label())
                                .size(13.0)
                                .color(Palette::TEXT),
                        );
                        ui.label(
                            RichText::new("Tür, kategori oluşturulduktan sonra değiştirilemez.")
                                .size(11.5)
                                .color(Palette::MUTED),
                        );
                    } else {
                        let mut chosen = draft.category_type;
                        ui.horizontal(|ui| {
                            for option in EntryType::ALL {
                                ui.selectable_value(&mut chosen, option, option.label());
                            }
                        });
                        if chosen != draft.category_type {
                            action = Some(Action::SetCategoryType(chosen));
                        }
                    }
                });
                ui.end_row();

                ui.label(field_label(
                    "Açıklama",
                    false,
                    errors.get(CategoryField::Description).is_some(),
                ));
                ui.vertical(|ui| {
                    ui.add(
                        TextEdit::multiline(&mut draft.description)
                            .hint_text("İsteğe bağlı")
                            .desired_width(340.0)
                            .desired_rows(2),
                    );
                    category_field_error(ui, errors, CategoryField::Description);
                });
                ui.end_row();

                ui.label(field_label("Durum", false, false));
                ui.add_enabled(can_manage, egui::Checkbox::new(&mut draft.active, "Aktif"));
                ui.end_row();
            });
    });

    ui.add_space(10.0);
    ui.horizontal(|ui| {
        if ui
            .add(Button::new("Kategoriyi Kaydet").min_size(vec2(150.0, 30.0)))
            .on_hover_text("Ctrl+S")
            .clicked()
        {
            action = Some(Action::SubmitCategory);
        }
        if ui.button("İptal").on_hover_text("Esc").clicked() {
            action = Some(Action::CancelCategoryForm);
        }
    });

    if let Some(action) = action {
        action.apply(app);
    }
}

// ---------------------------------------------------------------------------
// Özet
// ---------------------------------------------------------------------------

/// Dönem özeti.
fn summary(ui: &mut Ui, app: &mut OnMuhasebeApp) {
    let mut action: Option<Action> = None;

    page_header(
        ui,
        "Gelir ve Gider",
        "Seçili dönemin gelir/gider toplamları ve dağılımı (iptaller hariç).",
        |ui| {
            if ui
                .add(Button::new("Bu ay").min_size(vec2(100.0, 30.0)))
                .clicked()
            {
                action = Some(Action::SummaryThisMonth);
            }
        },
    );

    ui.add_space(6.0);
    tabs(ui, app, &mut action);
    ui.add_space(metrics::SECTION_GAP);

    if messages(ui, app, &mut action) {
        ui.add_space(metrics::SECTION_GAP);
    }

    summary_filter_bar(ui, app, &mut action);
    ui.add_space(10.0);

    let state = app.income_expense_state();
    let summary = state.summary.clone();
    let has_error = state.summary_error.is_some();

    if has_error {
        if let Some(action) = action {
            action.apply(app);
        }
        return;
    }

    if summary.is_empty() {
        widgets::card(ui, "Kayıt yok", |ui| {
            ui.add(
                Label::new(
                    RichText::new("Seçili dönemde gelir/gider kaydı bulunmuyor.")
                        .color(Palette::MUTED),
                )
                .wrap(),
            );
        });
        if let Some(action) = action {
            action.apply(app);
        }
        return;
    }

    if !summary.has_confirmed() {
        widgets::card(ui, "Kesin kayıt yok", |ui| {
            ui.add(
                Label::new(
                    RichText::new(
                        "Seçili dönemde kesin gelir/gider kaydı bulunmuyor \
                         (iptal edilenler hariç).",
                    )
                    .color(Palette::MUTED),
                )
                .wrap(),
            );
        });
        if let Some(action) = action {
            action.apply(app);
        }
        return;
    }

    widgets::card(ui, "Dönem toplamları (iptaller hariç)", |ui| {
        Grid::new("gelir_gider_donem_toplamlari")
            .num_columns(2)
            .min_col_width(150.0)
            .spacing([16.0, 6.0])
            .show(ui, |ui| {
                widgets::row_label(ui, "Toplam gelir");
                ui.label(
                    RichText::new(summary.income_total.format_with_symbol())
                        .strong()
                        .color(Palette::SUCCESS),
                );
                ui.end_row();

                widgets::row_label(ui, "Toplam gider");
                ui.label(
                    RichText::new(summary.expense_total.format_with_symbol())
                        .strong()
                        .color(Palette::DANGER),
                );
                ui.end_row();

                widgets::row_label(ui, "Net");
                ui.label(
                    RichText::new(summary.net().format_with_symbol())
                        .size(15.0)
                        .strong()
                        .color(balance_color(summary.net())),
                );
                ui.end_row();

                widgets::row_label(ui, "Kesin kayıt");
                ui.label(format!("{} adet", summary.confirmed_count));
                ui.end_row();
            });
    });

    ui.add_space(8.0);
    widgets::card(ui, "Kategori dağılımı", |ui| {
        Grid::new("gelir_gider_kategori_dagilimi")
            .num_columns(4)
            .min_col_width(70.0)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label(
                    RichText::new("Kategori")
                        .size(12.0)
                        .strong()
                        .color(Palette::MUTED),
                );
                ui.label(
                    RichText::new("Tür")
                        .size(12.0)
                        .strong()
                        .color(Palette::MUTED),
                );
                ui.label(
                    RichText::new("Tutar")
                        .size(12.0)
                        .strong()
                        .color(Palette::MUTED),
                );
                ui.label(
                    RichText::new("Yüzde")
                        .size(12.0)
                        .strong()
                        .color(Palette::MUTED),
                );
                ui.end_row();

                for row in &summary.categories {
                    cell_label(ui, &row.category_name, 170.0);
                    ui.label(RichText::new(row.entry_type.label()).size(12.5).color(
                        match row.entry_type {
                            EntryType::Gelir => Palette::SUCCESS,
                            EntryType::Gider => Palette::DANGER,
                        },
                    ));
                    ui.label(
                        RichText::new(row.total.format_with_symbol())
                            .size(12.5)
                            .strong()
                            .color(Palette::TEXT),
                    );
                    ui.label(
                        RichText::new(format_percentage(row.percentage))
                            .size(12.5)
                            .color(Palette::MUTED),
                    );
                    ui.end_row();
                }
            });
        ui.add_space(4.0);
        ui.add(
            Label::new(
                RichText::new(
                    "Yüzde, kategorinin kendi türünün toplamına göre tam sayı olarak hesaplanır.",
                )
                .size(11.5)
                .color(Palette::MUTED),
            )
            .wrap(),
        );
    });

    ui.add_space(8.0);
    widgets::card(ui, "Kasa/banka dağılımı", |ui| {
        if summary.accounts.is_empty() {
            ui.add(
                Label::new(
                    RichText::new("Bu dönemde kasa/banka hareketi yok.").color(Palette::MUTED),
                )
                .wrap(),
            );
        } else {
            Grid::new("gelir_gider_hesap_dagilimi")
                .num_columns(4)
                .min_col_width(70.0)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("Hesap")
                            .size(12.0)
                            .strong()
                            .color(Palette::MUTED),
                    );
                    ui.label(
                        RichText::new("Gelir")
                            .size(12.0)
                            .strong()
                            .color(Palette::MUTED),
                    );
                    ui.label(
                        RichText::new("Gider")
                            .size(12.0)
                            .strong()
                            .color(Palette::MUTED),
                    );
                    ui.label(
                        RichText::new("Net")
                            .size(12.0)
                            .strong()
                            .color(Palette::MUTED),
                    );
                    ui.end_row();

                    for row in &summary.accounts {
                        cell_label(ui, &row.cash_account_name, 170.0);
                        ui.label(
                            RichText::new(row.income_total.format_with_symbol())
                                .size(12.5)
                                .color(Palette::SUCCESS),
                        );
                        ui.label(
                            RichText::new(row.expense_total.format_with_symbol())
                                .size(12.5)
                                .color(Palette::DANGER),
                        );
                        ui.label(
                            RichText::new(row.net().format_with_symbol())
                                .size(12.5)
                                .strong()
                                .color(balance_color(row.net())),
                        );
                        ui.end_row();
                    }
                });
        }
    });

    if let Some(action) = action {
        action.apply(app);
    }
}

/// Özet tarih aralığı çubuğu.
fn summary_filter_bar(ui: &mut Ui, app: &mut OnMuhasebeApp, action: &mut Option<Action>) {
    widgets::card(ui, "Dönem", |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Başlangıç:").color(Palette::MUTED));
            ui.add(
                TextEdit::singleline(&mut app.income_expense_state_mut().summary_date_from)
                    .hint_text("YYYY-MM-DD")
                    .desired_width(120.0),
            );
            ui.label(RichText::new("Bitiş:").color(Palette::MUTED));
            ui.add(
                TextEdit::singleline(&mut app.income_expense_state_mut().summary_date_to)
                    .hint_text("YYYY-MM-DD")
                    .desired_width(120.0),
            );
            if ui.button("Uygula").clicked() {
                *action = Some(Action::ApplySummaryRange);
            }
            if ui.button("Bu ay").clicked() {
                *action = Some(Action::SummaryThisMonth);
            }
        });
    });
}

/// Yüzde gösterimi: taban sıfırsa `—`, aksi hâlde tam sayı + `%`.
fn format_percentage(percentage: Option<i64>) -> String {
    match percentage {
        Some(value) => format!("{value}%"),
        None => "—".to_owned(),
    }
}
