// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Kasa/banka ve tahsilat/ödeme ekranları: liste, form ve ayrıntı.
//!
//! Bu modül **yalnızca çizim yapar**. Veriyi [`OnMuhasebeApp::cash_state`]
//! üzerinden okur ve kullanıcı eylemlerini [`Action`] değerlerine dönüştürüp
//! uygulama katmanının komutlarına devreder. Hiçbir SQL bu dosyada bulunmaz.
//!
//! ## Sekmeler
//!
//! Bölüm iki sekmeden oluşur: **Hesaplar** (varsayılan) ve **Hareketler**.
//! Hareketler sekmesi kendi içinde `Tümü | Tahsilatlar | Ödemeler | Gelirler |
//! Giderler` alt sekmelerine ayrılır; `Tümü` tahsilat/ödemeler ile
//! gelir/giderleri tarihe göre birlikte gösterir.
//! Sekmeler yalnızca liste ekranlarında gösterilir; form ve ayrıntı ekranları
//! kendi "listeye dön" düğmesini taşır. Böylece form doldurulurken yanlışlıkla
//! sekme değiştirip girdi kaybı oluşmaz.
//!
//! ## Yerleşim notu
//!
//! Satır içi hizalamalarda [`egui::Sides`] + `shrink_left()` kullanılır; form
//! ve filtre alanları `Grid`/`horizontal_wrapped` ile sarılır. Bu nedenle
//! 900x600 gibi dar pencerelerde yatay taşma oluşmaz.

use egui::{Button, ComboBox, Grid, Label, RichText, ScrollArea, Sides, TextEdit, Ui, vec2};

use crate::app::{CashMovement, CashScreen, CashTab, FocusTarget, MovementTab, OnMuhasebeApp};
use crate::cash::{AccountType, CashAccount, Field as CashField, FieldErrors as CashFieldErrors};
use crate::money::Money;
use crate::payment::{
    Field as PaymentField, FieldErrors as PaymentFieldErrors, InvoicePaymentLink, Payment,
    PaymentAllocation, PaymentStatus, PaymentType,
};
use crate::ui::format::format_iso_date_short;
use crate::ui::theme::{Palette, metrics};
use crate::ui::{transfer, widgets};

/// Kullanıcı eylemleri; çizim sırasında toplanır ve sonra uygulanır.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    /// Sekme değiştirir (Hesaplar / Hareketler).
    OpenTab(CashTab),
    /// Yeni hesap formunu açar.
    NewAccount,
    /// Hesabın düzenleme formunu açar.
    EditAccount(i64),
    /// Hesabı pasife/aktife alır.
    ToggleAccount(i64),
    /// Hesap formunu doğrulayıp kaydeder.
    SubmitAccount,
    /// Hesap formunu kaydetmeden kapatır.
    CancelAccountForm,
    /// Hesap filtrelerini temizler.
    ResetAccountFilter,
    /// Yeni tahsilat/ödeme formunu açar.
    NewMovement(PaymentType),
    /// Hareket ayrıntısını açar.
    OpenMovementDetail(i64),
    /// Gelir/gider hareket ayrıntısını açar.
    OpenIncomeExpenseDetail(i64),
    /// Hareket listesine döner.
    BackToMovementList,
    /// Hareket filtrelerini temizler.
    ResetMovementFilter,
    /// Hareket alt sekmesini (tür filtresi) değiştirir.
    SetMovementTab(MovementTab),
    /// Formun türünü değiştirir.
    SetPaymentType(PaymentType),
    /// Formda cariyi seçer/kaldırır.
    SetPaymentCustomer(Option<i64>),
    /// Formda kasa/banka hesabını seçer/kaldırır.
    SetPaymentAccount(Option<i64>),
    /// Tüm dağıtımları kalan tutara eşitler.
    AllocateAll,
    /// Formu doğrulayıp kaydetme onayı ister.
    SubmitPayment,
    /// Onaylanan kaydı kesinleştirir.
    ConfirmPaymentSave,
    /// Kaydetme onayından vazgeçer.
    DiscardPaymentConfirmation,
    /// İptal onay penceresini açar.
    StartPaymentCancel,
    /// İptal onay penceresini kapatır.
    AbortPaymentCancel,
    /// İptal nedenini doğrulayıp kaydı iptal eder.
    SubmitPaymentCancel,
    /// Bilgilendirme mesajını kapatır.
    DismissNotice,
    /// Kasa/banka hesap listesini CSV olarak dışa aktarır.
    ExportAccountsCsv,
    /// Tahsilat/ödeme hareket listesini CSV olarak dışa aktarır.
    ExportPaymentsCsv,
    /// Gelir/gider hareket listesini CSV olarak dışa aktarır.
    ExportIncomeExpenseCsv,
    /// Kayıttaki cari hesabın ayrıntısını açar.
    OpenCustomer(i64),
    /// Kayıttaki kasa/banka hesabını listede vurgular.
    HighlightAccount(i64),
    /// Dağıtımdaki faturayı açar.
    OpenInvoice(i64),
}

impl Action {
    /// Eylemi uygulama katmanında uygular.
    fn apply(self, app: &mut OnMuhasebeApp) {
        match self {
            Self::OpenTab(tab) => app.set_cash_tab(tab),
            Self::NewAccount => app.open_cash_account_new_form(),
            Self::EditAccount(id) => app.open_cash_account_edit_form(id),
            Self::ToggleAccount(id) => app.toggle_cash_account_active(id),
            Self::SubmitAccount => app.submit_cash_account_form(),
            Self::CancelAccountForm => app.cancel_cash_account_form(),
            Self::ResetAccountFilter => app.reset_cash_account_filter(),
            Self::NewMovement(payment_type) => app.open_payment_form(payment_type),
            Self::OpenMovementDetail(id) => app.open_payment_detail(id),
            Self::OpenIncomeExpenseDetail(id) => app.open_income_expense_detail_from_cash(id),
            Self::BackToMovementList => app.open_payment_list(),
            Self::ResetMovementFilter => app.reset_payment_filter(),
            Self::SetMovementTab(tab) => app.set_movement_tab(tab),
            Self::SetPaymentType(payment_type) => app.set_payment_draft_type(payment_type),
            Self::SetPaymentCustomer(id) => app.set_payment_customer(id),
            Self::SetPaymentAccount(id) => app.set_payment_cash_account(id),
            Self::AllocateAll => app.allocate_payment_all(),
            Self::SubmitPayment => app.submit_payment_form(),
            Self::ConfirmPaymentSave => app.confirm_payment_save(),
            Self::DiscardPaymentConfirmation => app.discard_payment_confirmation(),
            Self::StartPaymentCancel => app.start_payment_cancel(),
            Self::AbortPaymentCancel => app.abort_payment_cancel(),
            Self::SubmitPaymentCancel => app.submit_payment_cancel(),
            Self::DismissNotice => app.dismiss_cash_notice(),
            Self::ExportAccountsCsv => {
                if let Some(path) =
                    transfer::save_csv_dialog(&OnMuhasebeApp::cash_account_list_csv_name())
                {
                    let path = OnMuhasebeApp::csv_path(path);
                    let _ = app.export_cash_account_list_csv(path);
                }
            }
            Self::ExportPaymentsCsv => {
                if let Some(path) =
                    transfer::save_csv_dialog(&OnMuhasebeApp::payment_list_csv_name())
                {
                    let path = OnMuhasebeApp::csv_path(path);
                    let _ = app.export_payment_list_csv(path);
                }
            }
            Self::ExportIncomeExpenseCsv => {
                if let Some(path) =
                    transfer::save_csv_dialog(&OnMuhasebeApp::income_expense_list_csv_name())
                {
                    let path = OnMuhasebeApp::csv_path(path);
                    let _ = app.export_income_expense_list_csv(path);
                }
            }
            Self::OpenCustomer(id) => app.show_customer_detail(id),
            Self::HighlightAccount(id) => app.show_cash_account_highlight(id),
            Self::OpenInvoice(id) => app.show_invoice_detail(id),
        }
    }
}

/// Aktif alt ekranı çizer.
pub fn show(ui: &mut Ui, app: &mut OnMuhasebeApp) {
    match app.cash_state().screen {
        CashScreen::AccountList => account_list(ui, app),
        CashScreen::AccountForm => account_form(ui, app),
        CashScreen::MovementList => movement_list(ui, app),
        CashScreen::MovementForm => movement_form(ui, app),
        CashScreen::MovementDetail => movement_detail(ui, app),
    }
}

// ---------------------------------------------------------------------------
// Ortak parçalar
// ---------------------------------------------------------------------------

/// Bilgilendirme ve hata mesajlarını çizer; herhangi biri çizildiyse `true`.
fn messages(ui: &mut Ui, app: &OnMuhasebeApp, action: &mut Option<Action>) -> bool {
    let state = app.cash_state();
    let mut drawn = false;

    if let Some(notice) = &state.notice {
        if widgets::success_card(ui, "İşlem başarılı", notice) {
            *action = Some(Action::DismissNotice);
        }
        drawn = true;
    }

    for (title, message) in [
        ("İşlem tamamlanamadı", state.general_error.as_deref()),
        ("Hesaplar yüklenemedi", state.account_list_error.as_deref()),
        (
            "Hareketler yüklenemedi",
            state.movement_list_error.as_deref(),
        ),
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

/// Hesap formu alan hatası.
fn cash_field_error(ui: &mut Ui, errors: &CashFieldErrors, field: CashField) {
    if let Some(message) = errors.get(field) {
        ui.label(RichText::new(message).size(11.5).color(Palette::DANGER));
    }
}

/// Tahsilat/ödeme formu alan hatası.
fn payment_field_error(ui: &mut Ui, errors: &PaymentFieldErrors, field: PaymentField) {
    if let Some(message) = errors.get(field) {
        ui.label(RichText::new(message).size(11.5).color(Palette::DANGER));
    }
}

/// Aktif/pasif durum etiketi.
fn active_badge(ui: &mut Ui, active: bool) {
    let (color, text) = if active {
        (Palette::SUCCESS, "Aktif")
    } else {
        (Palette::MUTED, "Pasif")
    };
    ui.label(RichText::new(format!("● {text}")).size(12.5).color(color));
}

/// Tahsilat/ödeme durum etiketi.
fn payment_status_badge(ui: &mut Ui, status: PaymentStatus) {
    let (color, text) = match status {
        PaymentStatus::Kesin => (Palette::SUCCESS, "Kesin"),
        PaymentStatus::Iptal => (Palette::DANGER, "İptal"),
    };
    ui.label(RichText::new(format!("● {text}")).size(12.5).color(color));
}

/// Güncel bakiye rengi (negatifse kırmızı, pozitifse yeşil, sıfırsa nötr).
fn balance_color(amount: Money) -> egui::Color32 {
    if amount.kurus() > 0 {
        Palette::SUCCESS
    } else if amount.kurus() < 0 {
        Palette::DANGER
    } else {
        Palette::MUTED
    }
}

/// Sekme çubuğu (yalnızca liste ekranlarında).
fn tabs(ui: &mut Ui, app: &OnMuhasebeApp, action: &mut Option<Action>) {
    let current = app.cash_state().tab;
    ui.horizontal(|ui| {
        for tab in CashTab::ALL {
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
fn back_header(ui: &mut Ui, back: &str, title: &str, action: &mut Option<Action>) {
    ui.horizontal(|ui| {
        if ui.button(back).clicked() {
            *action = Some(Action::BackToMovementList);
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

// ---------------------------------------------------------------------------
// Hesaplar sekmesi
// ---------------------------------------------------------------------------

/// Kasa/banka hesap listesi.
fn account_list(ui: &mut Ui, app: &mut OnMuhasebeApp) {
    let mut action: Option<Action> = None;

    page_header(
        ui,
        "Kasa ve Banka",
        "Nakit kasa ve banka hesapları. Kayıtlar silinmez, pasife alınır.",
        |ui| {
            if ui
                .add(Button::new("Yeni hesap").min_size(vec2(140.0, 30.0)))
                .clicked()
            {
                action = Some(Action::NewAccount);
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
            action = Some(Action::ExportAccountsCsv);
        }
    });
    ui.add_space(8.0);

    let totals = app.cash_totals();
    widgets::card(ui, "Bakiye özeti", |ui| {
        Grid::new("kasa_bakiye_ozeti")
            .num_columns(2)
            .min_col_width(150.0)
            .spacing([16.0, 6.0])
            .show(ui, |ui| {
                widgets::row_label(ui, "Toplam kasa");
                ui.label(
                    RichText::new(totals.cash.format_with_symbol())
                        .strong()
                        .color(Palette::TEXT),
                );
                ui.end_row();

                widgets::row_label(ui, "Toplam banka");
                ui.label(
                    RichText::new(totals.bank.format_with_symbol())
                        .strong()
                        .color(Palette::TEXT),
                );
                ui.end_row();

                widgets::row_label(ui, "Genel toplam");
                ui.label(
                    RichText::new(totals.total().format_with_symbol())
                        .size(15.0)
                        .strong()
                        .color(balance_color(totals.total())),
                );
                ui.end_row();
            });
        ui.add_space(4.0);
        ui.label(
            RichText::new("Toplamlar yalnızca aktif hesapları kapsar.")
                .size(11.5)
                .color(Palette::MUTED),
        );
    });

    ui.add_space(metrics::SECTION_GAP);
    account_filter_bar(ui, app, &mut action);
    ui.add_space(10.0);

    let count = app.cash_state().account_list.len();
    ui.label(
        RichText::new(format!("{count} hesap görüntüleniyor"))
            .size(12.5)
            .color(Palette::MUTED),
    );
    ui.add_space(6.0);

    {
        let state = app.cash_state();
        if state.account_list_error.is_none() {
            if state.account_list.is_empty() {
                widgets::card(ui, "Kayıt yok", |ui| {
                    let message = if state.account_filter.is_empty() {
                        "Henüz kasa/banka hesabı yok. \"Yeni hesap\" ile ilk hesabı oluşturun."
                    } else {
                        "Arama ve filtre ölçütlerine uyan hesap bulunamadı."
                    };
                    ui.add(Label::new(RichText::new(message).color(Palette::MUTED)).wrap());
                });
            } else {
                ScrollArea::vertical()
                    .id_salt("kasa_hesap_listesi")
                    .auto_shrink([false, false])
                    .max_height(400.0)
                    .show_rows(ui, 92.0, state.account_list.len(), |ui, range| {
                        for index in range {
                            let account = &state.account_list[index];
                            let highlighted = state.highlight_account_id == Some(account.id);
                            account_row(ui, account, highlighted, app.is_manager(), &mut action);
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

/// Hesap arama/filtre çubuğu.
fn account_filter_bar(ui: &mut Ui, app: &mut OnMuhasebeApp, action: &mut Option<Action>) {
    let before = app.cash_state().account_filter.clone();

    {
        let state = app.cash_state_mut();
        widgets::card(ui, "Ara ve filtrele", |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Ara:").color(Palette::MUTED));
                ui.add(
                    TextEdit::singleline(&mut state.account_filter.search)
                        .hint_text("Hesap adı, banka veya IBAN")
                        .desired_width(220.0),
                );

                ui.add_space(12.0);
                ui.label(RichText::new("Tür:").color(Palette::MUTED));
                ComboBox::from_id_salt("kasa_tur_filtresi")
                    .selected_text(match state.account_filter.account_type {
                        None => "Tümü",
                        Some(account_type) => account_type.label(),
                    })
                    .width(120.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut state.account_filter.account_type, None, "Tümü");
                        for account_type in AccountType::ALL {
                            ui.selectable_value(
                                &mut state.account_filter.account_type,
                                Some(account_type),
                                account_type.label(),
                            );
                        }
                    });

                ui.add_space(12.0);
                ui.label(RichText::new("Durum:").color(Palette::MUTED));
                ComboBox::from_id_salt("kasa_durum_filtresi")
                    .selected_text(match state.account_filter.active {
                        None => "Tümü",
                        Some(true) => "Aktif",
                        Some(false) => "Pasif",
                    })
                    .width(120.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut state.account_filter.active, None, "Tümü");
                        ui.selectable_value(&mut state.account_filter.active, Some(true), "Aktif");
                        ui.selectable_value(&mut state.account_filter.active, Some(false), "Pasif");
                    });

                ui.add_space(12.0);
                if ui.button("Filtreleri temizle").clicked() {
                    *action = Some(Action::ResetAccountFilter);
                }
            });
        });
    }

    if app.cash_state().account_filter != before {
        app.refresh_cash_account_list();
    }
}

/// Tek bir hesabın liste satırı.
fn account_row(
    ui: &mut Ui,
    account: &CashAccount,
    highlighted: bool,
    can_manage: bool,
    action: &mut Option<Action>,
) {
    let fill = if highlighted {
        Palette::ACCENT_SOFT
    } else if account.active {
        Palette::SURFACE
    } else {
        Palette::BACKGROUND
    };
    let border = if highlighted {
        Palette::ACCENT
    } else {
        Palette::BORDER
    };

    widgets::surface_tinted(ui, fill, border, |ui| {
        Sides::new().height(24.0).shrink_left().show(
            ui,
            |ui| {
                ui.add(
                    Label::new(
                        RichText::new(&account.name)
                            .size(14.5)
                            .strong()
                            .color(Palette::TEXT),
                    )
                    .truncate(),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new(account.account_type.label())
                        .size(12.0)
                        .color(Palette::ACCENT),
                );
                if highlighted {
                    ui.label(
                        RichText::new("Seçili")
                            .size(11.5)
                            .strong()
                            .color(Palette::ACCENT),
                    );
                }
            },
            |ui| {
                ui.label(
                    RichText::new(format!(
                        "Güncel: {}",
                        account.current_balance.format_with_symbol()
                    ))
                    .size(13.0)
                    .strong()
                    .color(balance_color(account.current_balance)),
                );
                ui.add_space(10.0);
                active_badge(ui, account.active);
            },
        );

        ui.add_space(2.0);

        Sides::new().height(24.0).shrink_left().show(
            ui,
            |ui| {
                if account.is_bank() {
                    ui.label(
                        RichText::new(format!(
                            "Banka: {}",
                            account.bank_name.as_deref().unwrap_or("—")
                        ))
                        .size(12.5)
                        .color(Palette::MUTED),
                    );
                    if let Some(iban) = account.shortened_iban() {
                        ui.label(RichText::new("·").size(12.5).color(Palette::MUTED));
                        ui.label(
                            RichText::new(format!("IBAN: {iban}"))
                                .size(12.5)
                                .color(Palette::MUTED),
                        );
                    }
                } else {
                    ui.label(RichText::new("Nakit kasa").size(12.5).color(Palette::MUTED));
                }
                ui.label(RichText::new("·").size(12.5).color(Palette::MUTED));
                ui.label(
                    RichText::new(format!(
                        "Açılış: {}",
                        account.opening_balance.format_with_symbol()
                    ))
                    .size(12.5)
                    .color(Palette::MUTED),
                );
            },
            |ui| {
                let toggle_text = if account.active {
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
                    *action = Some(Action::ToggleAccount(account.id));
                }
                if ui.add(Button::new("Düzenle").small()).clicked() {
                    *action = Some(Action::EditAccount(account.id));
                }
            },
        );
    });
}

// ---------------------------------------------------------------------------
// Hesap formu
// ---------------------------------------------------------------------------

/// Yeni/düzenleme hesap formu.
fn account_form(ui: &mut Ui, app: &mut OnMuhasebeApp) {
    let mut action: Option<Action> = None;
    let is_new = app.cash_state().account_form_id.is_none();
    let error_count = app.cash_state().account_errors.len();

    ui.horizontal(|ui| {
        if ui.button("← Listeye dön").clicked() {
            action = Some(Action::CancelAccountForm);
        }
        ui.add_space(6.0);
        ui.add(
            Label::new(
                RichText::new(if is_new {
                    "Yeni kasa/banka hesabı"
                } else {
                    "Hesabı düzenle"
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

    let opening_editable = app.cash_state().account_opening_editable;
    let is_bank = app.cash_state().account_draft.account_type == AccountType::Banka;
    let focus_name = app.take_focus(FocusTarget::CashName);

    {
        let can_manage = app.is_manager();
        let state = app.cash_state_mut();
        let draft = &mut state.account_draft;
        let errors = &mut state.account_errors;

        widgets::card(ui, "Hesap bilgileri", |ui| {
            Grid::new("kasa_form_bilgi")
                .num_columns(2)
                .min_col_width(150.0)
                .spacing([16.0, 8.0])
                .show(ui, |ui| {
                    // Ad.
                    ui.label(field_label(
                        "Hesap adı",
                        true,
                        errors.get(CashField::Name).is_some(),
                    ));
                    ui.vertical(|ui| {
                        let response = ui.add(
                            TextEdit::singleline(&mut draft.name)
                                .hint_text("Örnek: Merkez Kasa")
                                .desired_width(320.0),
                        );
                        if focus_name {
                            response.request_focus();
                        }
                        if response.changed() {
                            errors.clear(CashField::Name);
                        }
                        cash_field_error(ui, errors, CashField::Name);
                    });
                    ui.end_row();

                    // Tür.
                    ui.label(field_label("Hesap türü", true, false));
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            for account_type in AccountType::ALL {
                                ui.selectable_value(
                                    &mut draft.account_type,
                                    account_type,
                                    account_type.label(),
                                );
                            }
                        });
                        ui.label(
                            RichText::new("Kasa: nakit. Banka: banka adı ve IBAN girilebilir.")
                                .size(11.5)
                                .color(Palette::MUTED),
                        );
                    });
                    ui.end_row();

                    if is_bank {
                        ui.label(field_label(
                            "Banka adı",
                            false,
                            errors.get(CashField::BankName).is_some(),
                        ));
                        ui.vertical(|ui| {
                            if ui
                                .add(
                                    TextEdit::singleline(&mut draft.bank_name)
                                        .hint_text("Örnek: Ziraat Bankası")
                                        .desired_width(300.0),
                                )
                                .changed()
                            {
                                errors.clear(CashField::BankName);
                            }
                            cash_field_error(ui, errors, CashField::BankName);
                        });
                        ui.end_row();

                        ui.label(field_label(
                            "IBAN",
                            false,
                            errors.get(CashField::Iban).is_some(),
                        ));
                        ui.vertical(|ui| {
                            if ui
                                .add(
                                    TextEdit::singleline(&mut draft.iban)
                                        .hint_text("TR33 0006 1005 1978 6457 8413 26")
                                        .desired_width(340.0),
                                )
                                .changed()
                            {
                                errors.clear(CashField::Iban);
                            }
                            ui.label(
                                RichText::new("İsteğe bağlı. Doluysa TR ile başlayan 26 karakter.")
                                    .size(11.5)
                                    .color(Palette::MUTED),
                            );
                            cash_field_error(ui, errors, CashField::Iban);
                        });
                        ui.end_row();
                    }

                    // Açılış bakiyesi.
                    ui.label(field_label(
                        "Açılış bakiyesi (₺)",
                        false,
                        errors.get(CashField::OpeningBalance).is_some(),
                    ));
                    ui.vertical(|ui| {
                        if ui
                            .add_enabled(
                                opening_editable,
                                TextEdit::singleline(&mut draft.opening_balance)
                                    .hint_text("0,00")
                                    .desired_width(160.0),
                            )
                            .changed()
                        {
                            errors.clear(CashField::OpeningBalance);
                        }
                        if !opening_editable {
                            ui.label(
                                RichText::new(
                                    "Bu hesapta hareket var; açılış bakiyesi \
                                     artık değiştirilemez.",
                                )
                                .size(11.5)
                                .color(Palette::MUTED),
                            );
                        }
                        cash_field_error(ui, errors, CashField::OpeningBalance);
                    });
                    ui.end_row();

                    // Notlar.
                    ui.label(field_label(
                        "Notlar",
                        false,
                        errors.get(CashField::Notes).is_some(),
                    ));
                    ui.vertical(|ui| {
                        if ui
                            .add(
                                TextEdit::multiline(&mut draft.notes)
                                    .desired_width(340.0)
                                    .desired_rows(2),
                            )
                            .changed()
                        {
                            errors.clear(CashField::Notes);
                        }
                        cash_field_error(ui, errors, CashField::Notes);
                    });
                    ui.end_row();

                    // Durum.
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
            action = Some(Action::SubmitAccount);
        }
        if ui
            .add(Button::new("İptal").min_size(vec2(130.0, 32.0)))
            .on_hover_text("Esc")
            .clicked()
        {
            action = Some(Action::CancelAccountForm);
        }
    });

    if let Some(action) = action {
        action.apply(app);
    }
}

// ---------------------------------------------------------------------------
// Hareketler sekmesi
// ---------------------------------------------------------------------------

/// Tahsilat/ödeme listesi.
fn movement_list(ui: &mut Ui, app: &mut OnMuhasebeApp) {
    let mut action: Option<Action> = None;

    page_header(
        ui,
        "Kasa ve Banka",
        "Tahsilat, ödeme, gelir ve gider hareketleri. Kayıtlar silinmez, iptal edilir.",
        |ui| {
            if ui
                .add(Button::new("Yeni Tahsilat").min_size(vec2(130.0, 30.0)))
                .clicked()
            {
                action = Some(Action::NewMovement(PaymentType::Tahsilat));
            }
            if ui
                .add(Button::new("Yeni Ödeme").min_size(vec2(130.0, 30.0)))
                .clicked()
            {
                action = Some(Action::NewMovement(PaymentType::Odeme));
            }
        },
    );

    ui.add_space(6.0);
    tabs(ui, app, &mut action);
    ui.add_space(6.0);

    // Alt sekmeler (tür filtresi).
    ui.horizontal_wrapped(|ui| {
        let current = app.cash_state().movement_tab;
        for tab in MovementTab::ALL {
            if ui
                .selectable_label(current == tab, RichText::new(tab.label()).size(13.0))
                .clicked()
                && current != tab
            {
                action = Some(Action::SetMovementTab(tab));
            }
        }
    });

    ui.add_space(metrics::SECTION_GAP);
    if messages(ui, app, &mut action) {
        ui.add_space(metrics::SECTION_GAP);
    }

    ui.horizontal_wrapped(|ui| {
        if ui.button("Tahsilat/ödemeleri CSV").clicked() {
            action = Some(Action::ExportPaymentsCsv);
        }
        if ui.button("Gelir/giderleri CSV").clicked() {
            action = Some(Action::ExportIncomeExpenseCsv);
        }
    });
    ui.add_space(8.0);

    movement_filter_bar(ui, app, &mut action);
    ui.add_space(10.0);

    let movement_tab = app.cash_state().movement_tab;
    let combined = matches!(
        movement_tab,
        MovementTab::All | MovementTab::Incomes | MovementTab::Expenses
    );
    let movements = app.cash_movement_list();
    let count = if combined {
        movements.len()
    } else {
        app.cash_state().movement_list.len()
    };
    let (payment_summary, entry_summary) = {
        let state = app.cash_state();
        (state.movement_summary, state.movement_entry_summary)
    };
    let net_effect = app.cash_movement_net();

    ui.label(
        RichText::new(format!("{count} kayıt görüntüleniyor"))
            .size(12.5)
            .color(Palette::MUTED),
    );
    ui.add_space(6.0);

    if app.cash_state().movement_list_error.is_none() {
        if count == 0 {
            let message = match movement_tab {
                MovementTab::All => {
                    "Henüz hareket kaydı yok. Tahsilat, ödeme, gelir veya gider ekleyin."
                }
                MovementTab::Collections => "Bu sekmede tahsilat kaydı yok.",
                MovementTab::Payments => "Bu sekmede ödeme kaydı yok.",
                MovementTab::Incomes => {
                    "Bu sekmede gelir kaydı yok. Gelirleri Gelir ve Gider bölümünden ekleyin."
                }
                MovementTab::Expenses => {
                    "Bu sekmede gider kaydı yok. Giderleri Gelir ve Gider bölümünden ekleyin."
                }
            };
            widgets::card(ui, "Kayıt yok", |ui| {
                ui.add(Label::new(RichText::new(message).color(Palette::MUTED)).wrap());
            });
        } else if combined {
            ScrollArea::vertical()
                .id_salt("kasa_hareket_listesi")
                .auto_shrink([false, false])
                .max_height(430.0)
                .show_rows(ui, 92.0, movements.len(), |ui, range| {
                    for index in range {
                        cash_movement_row(ui, &movements[index], app.is_manager(), &mut action);
                        ui.add_space(6.0);
                    }
                });
        } else {
            let movement_list = &app.cash_state().movement_list;
            ScrollArea::vertical()
                .id_salt("odeme_hareket_listesi")
                .auto_shrink([false, false])
                .max_height(430.0)
                .show_rows(ui, 92.0, movement_list.len(), |ui, range| {
                    for index in range {
                        movement_row(ui, &movement_list[index], app.is_manager(), &mut action);
                        ui.add_space(6.0);
                    }
                });
        }
    }

    ui.add_space(8.0);
    widgets::card(ui, "Özet (iptaller hariç)", |ui| {
        Grid::new("kasa_hareket_ozeti")
            .num_columns(2)
            .min_col_width(150.0)
            .spacing([16.0, 6.0])
            .show(ui, |ui| {
                if movement_tab.shows_payments() {
                    widgets::row_label(ui, "Toplam tahsilat");
                    ui.label(
                        RichText::new(payment_summary.collection_total.format_with_symbol())
                            .strong()
                            .color(Palette::SUCCESS),
                    );
                    ui.end_row();

                    widgets::row_label(ui, "Toplam ödeme");
                    ui.label(
                        RichText::new(payment_summary.payment_total.format_with_symbol())
                            .strong()
                            .color(Palette::DANGER),
                    );
                    ui.end_row();
                }

                if movement_tab.shows_entries() {
                    widgets::row_label(ui, "Toplam gelir");
                    ui.label(
                        RichText::new(entry_summary.income_total.format_with_symbol())
                            .strong()
                            .color(Palette::SUCCESS),
                    );
                    ui.end_row();

                    widgets::row_label(ui, "Toplam gider");
                    ui.label(
                        RichText::new(entry_summary.expense_total.format_with_symbol())
                            .strong()
                            .color(Palette::DANGER),
                    );
                    ui.end_row();
                }

                let net = if movement_tab.shows_payments() && movement_tab.shows_entries() {
                    net_effect
                } else if movement_tab.shows_entries() {
                    entry_summary.net()
                } else {
                    payment_summary.net()
                };
                widgets::row_label(
                    ui,
                    if movement_tab.shows_payments() && movement_tab.shows_entries() {
                        "Net nakit etkisi"
                    } else {
                        "Net"
                    },
                );
                ui.label(
                    RichText::new(net.format_with_symbol())
                        .size(15.0)
                        .strong()
                        .color(balance_color(net)),
                );
                ui.end_row();
            });
    });

    if let Some(action) = action {
        action.apply(app);
    }
}

/// Hareket arama/filtre çubuğu.
fn movement_filter_bar(ui: &mut Ui, app: &mut OnMuhasebeApp, action: &mut Option<Action>) {
    let before = app.cash_state().movement_filter.clone();
    let category_before = app.cash_state().movement_category_id;
    let movement_tab = app.cash_state().movement_tab;
    let show_customer = matches!(
        movement_tab,
        MovementTab::Collections | MovementTab::Payments
    );
    let show_category = matches!(movement_tab, MovementTab::Incomes | MovementTab::Expenses);

    let customer_label = app
        .cash_state()
        .movement_filter
        .customer_id
        .and_then(|id| app.cash_customer_label(id))
        .unwrap_or_else(|| "Tümü".to_owned());
    let customer_options = app.movement_customer_options();
    let category_label = app
        .cash_state()
        .movement_category_id
        .and_then(|id| app.income_expense_category_label(id))
        .unwrap_or_else(|| "Tümü".to_owned());
    let category_options = app.income_expense_filter_category_options();
    let account_label = app
        .cash_state()
        .movement_filter
        .cash_account_id
        .and_then(|id| app.cash_account_label(id))
        .unwrap_or_else(|| "Tümü".to_owned());
    let account_options = app.movement_cash_account_options();

    {
        let state = app.cash_state_mut();
        let filter = &mut state.movement_filter;

        widgets::card(ui, "Ara ve filtrele", |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Ara:").color(Palette::MUTED));
                ui.add(
                    TextEdit::singleline(&mut filter.search)
                        .hint_text(if show_category {
                            "Açıklama, kategori veya belge no"
                        } else {
                            "Açıklama veya cari adı"
                        })
                        .desired_width(190.0),
                );

                if show_customer {
                    ui.add_space(12.0);
                    ui.label(RichText::new("Cari:").color(Palette::MUTED));
                    if let Some(id) = widgets::searchable_select(
                        ui,
                        "kasa_cari_filtresi",
                        &customer_label,
                        &customer_options,
                        190.0,
                    ) {
                        filter.customer_id = Some(id);
                    }
                    if filter.customer_id.is_some() && ui.small_button("×").clicked() {
                        filter.customer_id = None;
                    }
                }

                if show_category {
                    ui.add_space(12.0);
                    ui.label(RichText::new("Kategori:").color(Palette::MUTED));
                    if let Some(id) = widgets::searchable_select(
                        ui,
                        "kasa_kategori_filtresi",
                        &category_label,
                        &category_options,
                        180.0,
                    ) {
                        state.movement_category_id = Some(id);
                    }
                    if state.movement_category_id.is_some() && ui.small_button("×").clicked() {
                        state.movement_category_id = None;
                    }
                }

                ui.add_space(12.0);
                ui.label(RichText::new("Hesap:").color(Palette::MUTED));
                if let Some(id) = widgets::searchable_select(
                    ui,
                    "kasa_hesap_filtresi",
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
                ComboBox::from_id_salt("kasa_hareket_durum_filtresi")
                    .selected_text(match filter.status {
                        None => "Tümü",
                        Some(status) => status.label(),
                    })
                    .width(110.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut filter.status, None, "Tümü");
                        for status in PaymentStatus::ALL {
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
                    *action = Some(Action::ResetMovementFilter);
                }
            });
        });
    }

    if app.cash_state().movement_filter != before
        || app.cash_state().movement_category_id != category_before
    {
        app.refresh_payment_list();
    }
}

/// "Tümü" ve gelir/gider sekmelerindeki birleşik satır.
fn cash_movement_row(
    ui: &mut Ui,
    movement: &CashMovement,
    can_cancel: bool,
    action: &mut Option<Action>,
) {
    let cancelled = movement.is_cancelled();
    let fill = if cancelled {
        Palette::BACKGROUND
    } else {
        Palette::SURFACE
    };
    let source = movement.source_label();
    let type_color = match source {
        "Tahsilat" | "Gelir" => Palette::SUCCESS,
        _ => Palette::DANGER,
    };
    let open_action = match movement {
        CashMovement::Payment(payment) => Action::OpenMovementDetail(payment.id),
        CashMovement::Entry(entry) => Action::OpenIncomeExpenseDetail(entry.id),
    };

    widgets::surface_tinted(ui, fill, Palette::BORDER, |ui| {
        Sides::new().height(24.0).shrink_left().show(
            ui,
            |ui| {
                ui.label(
                    RichText::new(format_iso_date_short(movement.date()))
                        .monospace()
                        .size(12.5)
                        .color(Palette::MUTED),
                );
                ui.label(RichText::new(source).size(12.5).strong().color(type_color));
                ui.add(
                    Label::new(
                        RichText::new(movement.counterparty())
                            .size(13.5)
                            .color(Palette::TEXT),
                    )
                    .truncate(),
                );
            },
            |ui| {
                ui.label(
                    RichText::new(movement.amount().format_with_symbol())
                        .size(14.0)
                        .strong()
                        .color(if cancelled {
                            Palette::MUTED
                        } else {
                            Palette::TEXT
                        }),
                );
                ui.add_space(8.0);
                let (status_color, status_text) = if cancelled {
                    (Palette::DANGER, "İptal")
                } else {
                    (Palette::SUCCESS, "Kesin")
                };
                ui.label(
                    RichText::new(format!("● {status_text}"))
                        .size(12.5)
                        .color(status_color),
                );
            },
        );

        ui.add_space(2.0);

        Sides::new().height(24.0).shrink_left().show(
            ui,
            |ui| {
                ui.label(
                    RichText::new(movement.account_name())
                        .size(12.5)
                        .color(Palette::MUTED),
                );
                if let Some(description) = movement.description() {
                    ui.label(RichText::new("·").size(12.5).color(Palette::MUTED));
                    ui.add(
                        Label::new(RichText::new(description).size(12.0).color(Palette::MUTED))
                            .truncate(),
                    );
                }
            },
            |ui| {
                if ui.button("Ayrıntı").clicked() {
                    *action = Some(open_action);
                }
                if can_cancel && !cancelled && ui.small_button("İptal et").clicked() {
                    *action = Some(open_action);
                }
            },
        );
    });
}

/// Tek bir tahsilat/ödeme satırı.
fn movement_row(ui: &mut Ui, payment: &Payment, can_cancel: bool, action: &mut Option<Action>) {
    let fill = if payment.is_cancelled() {
        Palette::BACKGROUND
    } else {
        Palette::SURFACE
    };
    let type_color = match payment.payment_type {
        PaymentType::Tahsilat => Palette::SUCCESS,
        PaymentType::Odeme => Palette::DANGER,
    };

    widgets::surface_tinted(ui, fill, Palette::BORDER, |ui| {
        Sides::new().height(24.0).shrink_left().show(
            ui,
            |ui| {
                ui.label(
                    RichText::new(format_iso_date_short(&payment.payment_date))
                        .monospace()
                        .size(12.5)
                        .color(Palette::MUTED),
                );
                ui.label(
                    RichText::new(payment.payment_type.label())
                        .size(12.5)
                        .strong()
                        .color(type_color),
                );
                ui.add(
                    Label::new(
                        RichText::new(format!(
                            "{} — {}",
                            payment.customer_code, payment.customer_name
                        ))
                        .size(13.5)
                        .color(Palette::TEXT),
                    )
                    .truncate(),
                );
            },
            |ui| {
                ui.label(
                    RichText::new(payment.amount.format_with_symbol())
                        .size(14.0)
                        .strong()
                        .color(if payment.is_cancelled() {
                            Palette::MUTED
                        } else {
                            Palette::TEXT
                        }),
                );
                ui.add_space(8.0);
                payment_status_badge(ui, payment.status);
            },
        );

        ui.add_space(2.0);

        Sides::new().height(24.0).shrink_left().show(
            ui,
            |ui| {
                ui.label(
                    RichText::new(&payment.cash_account_name)
                        .size(12.5)
                        .color(Palette::MUTED),
                );
                if payment.is_advance() {
                    ui.label(RichText::new("·").size(12.5).color(Palette::MUTED));
                    ui.label(RichText::new("Avans").size(12.0).color(Palette::ACCENT));
                } else if payment.remainder() > Money::ZERO {
                    ui.label(
                        RichText::new(format!(
                            "· Avans: {}",
                            payment.remainder().format_with_symbol()
                        ))
                        .size(12.0)
                        .color(Palette::ACCENT),
                    );
                }
                if let Some(description) = &payment.description {
                    ui.label(RichText::new("·").size(12.5).color(Palette::MUTED));
                    ui.add(
                        Label::new(RichText::new(description).size(12.0).color(Palette::MUTED))
                            .truncate(),
                    );
                }
            },
            |ui| {
                if ui.button("Ayrıntı").clicked() {
                    *action = Some(Action::OpenMovementDetail(payment.id));
                }
                if can_cancel
                    && payment.status == PaymentStatus::Kesin
                    && ui.small_button("İptal et").clicked()
                {
                    *action = Some(Action::OpenMovementDetail(payment.id));
                }
            },
        );
    });
}

// ---------------------------------------------------------------------------
// Tahsilat/ödeme formu
// ---------------------------------------------------------------------------

/// Klavye odağı kayması olmadan menü seçimlerini çizen yardımcı.
fn movement_type_selector(ui: &mut Ui, selected: PaymentType, action: &mut Option<Action>) {
    let mut chosen = selected;
    ui.horizontal(|ui| {
        for option in PaymentType::ALL {
            ui.selectable_value(&mut chosen, option, option.label());
        }
    });
    if chosen != selected {
        *action = Some(Action::SetPaymentType(chosen));
    }
}

/// Yeni tahsilat/ödeme formu.
fn movement_form(ui: &mut Ui, app: &mut OnMuhasebeApp) {
    let mut action: Option<Action> = None;

    let payment_type = app.cash_state().payment_draft.payment_type;
    back_header(
        ui,
        "← Hareketler",
        &format!("Yeni {}", payment_type.label()),
        &mut action,
    );

    ui.add_space(metrics::SECTION_GAP);
    if messages(ui, app, &mut action) {
        ui.add_space(metrics::SECTION_GAP);
    }

    let error_count = app.cash_state().payment_errors.len();
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
    if app.cash_state().payment_pending.is_some() {
        if let Some(action_value) = payment_confirmation(ui, app) {
            action = Some(action_value);
        }
        if let Some(action) = action {
            action.apply(app);
        }
        return;
    }

    let customer_options = app.payment_customer_options();
    let customer_label = app
        .cash_state()
        .payment_draft
        .customer_id
        .and_then(|id| app.cash_customer_label(id))
        .unwrap_or_else(|| "Cari seçin".to_owned());
    let account_options = app.payment_cash_account_options();
    let account_label = app
        .cash_state()
        .payment_draft
        .cash_account_id
        .and_then(|id| app.cash_account_label(id))
        .unwrap_or_else(|| "Hesap seçin".to_owned());
    let mismatch = app
        .payment_selected_customer()
        .and_then(|customer| payment_type.customer_kind_warning(customer.kind));

    let focus_customer = app.take_focus(FocusTarget::PaymentCustomer);
    let focus_amount = app.take_focus(FocusTarget::PaymentAmount) || focus_customer;

    ui.add_space(metrics::SECTION_GAP);
    {
        let state = app.cash_state_mut();
        let draft = &mut state.payment_draft;
        let errors = &mut state.payment_errors;

        widgets::card(ui, "Kayıt bilgileri", |ui| {
            Grid::new("kasa_form_ust")
                .num_columns(2)
                .min_col_width(150.0)
                .spacing([16.0, 8.0])
                .show(ui, |ui| {
                    // Tür (yalnızca yeni kayıtta seçilir).
                    ui.label(field_label("Tür", true, false));
                    movement_type_selector(ui, draft.payment_type, &mut action);
                    ui.end_row();

                    // Cari.
                    ui.label(field_label(
                        "Cari hesap",
                        true,
                        errors.get(PaymentField::Customer).is_some(),
                    ));
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            if let Some(id) = widgets::searchable_select(
                                ui,
                                "kasa_form_cari",
                                &customer_label,
                                &customer_options,
                                320.0,
                            ) {
                                action = Some(Action::SetPaymentCustomer(Some(id)));
                            }
                            if draft.customer_id.is_some() && ui.small_button("×").clicked() {
                                action = Some(Action::SetPaymentCustomer(None));
                            }
                        });
                        payment_field_error(ui, errors, PaymentField::Customer);
                    });
                    ui.end_row();

                    // Kasa/banka.
                    ui.label(field_label(
                        "Kasa/banka",
                        true,
                        errors.get(PaymentField::CashAccount).is_some(),
                    ));
                    ui.vertical(|ui| {
                        if let Some(id) = widgets::searchable_select(
                            ui,
                            "kasa_form_hesap",
                            &account_label,
                            &account_options,
                            280.0,
                        ) {
                            action = Some(Action::SetPaymentAccount(Some(id)));
                        }
                        payment_field_error(ui, errors, PaymentField::CashAccount);
                    });
                    ui.end_row();

                    // Tarih.
                    ui.label(field_label(
                        "Tarih",
                        true,
                        errors.get(PaymentField::Date).is_some(),
                    ));
                    ui.vertical(|ui| {
                        if ui
                            .add(
                                TextEdit::singleline(&mut draft.payment_date)
                                    .hint_text("2026-10-18")
                                    .desired_width(140.0),
                            )
                            .changed()
                        {
                            errors.clear(PaymentField::Date);
                        }
                        payment_field_error(ui, errors, PaymentField::Date);
                    });
                    ui.end_row();

                    // Tutar.
                    ui.label(field_label(
                        "Tutar (₺)",
                        true,
                        errors.get(PaymentField::Amount).is_some(),
                    ));
                    ui.vertical(|ui| {
                        let response = ui.add(
                            TextEdit::singleline(&mut draft.amount)
                                .hint_text("0,00")
                                .desired_width(160.0),
                        );
                        if focus_amount {
                            response.request_focus();
                        }
                        if response.changed() {
                            errors.clear(PaymentField::Amount);
                        }
                        payment_field_error(ui, errors, PaymentField::Amount);
                    });
                    ui.end_row();

                    // Açıklama.
                    ui.label(field_label(
                        "Açıklama",
                        false,
                        errors.get(PaymentField::Description).is_some(),
                    ));
                    ui.vertical(|ui| {
                        if ui
                            .add(
                                TextEdit::multiline(&mut draft.description)
                                    .desired_width(340.0)
                                    .desired_rows(2),
                            )
                            .changed()
                        {
                            errors.clear(PaymentField::Description);
                        }
                        payment_field_error(ui, errors, PaymentField::Description);
                    });
                    ui.end_row();
                });
            ui.add_space(6.0);
            ui.add(
                Label::new(
                    RichText::new("Kesinleşen kayıt düzenlenemez; düzeltme iptal + yeni kayıttır.")
                        .size(11.5)
                        .color(Palette::MUTED),
                )
                .wrap(),
            );
        });
    }

    if let Some(message) = &mismatch {
        ui.add_space(metrics::SECTION_GAP);
        widgets::notice_card(ui, "Cari türü uyarısı", message);
    }

    ui.add_space(metrics::SECTION_GAP);
    allocation_panel(ui, app, &mut action);

    ui.add_space(metrics::SECTION_GAP);
    payment_footer(ui, app);

    ui.add_space(metrics::SECTION_GAP);
    ui.horizontal(|ui| {
        if ui
            .add(Button::new(payment_type.save_label()).min_size(vec2(170.0, 32.0)))
            .on_hover_text("Ctrl+S")
            .clicked()
        {
            action = Some(Action::SubmitPayment);
        }
        if ui
            .add(Button::new("İptal").min_size(vec2(120.0, 32.0)))
            .on_hover_text("Esc")
            .clicked()
        {
            action = Some(Action::BackToMovementList);
        }
    });

    if let Some(action) = action {
        action.apply(app);
    }
}

/// Kaydetme onayı kartı; bir eylem döner.
fn payment_confirmation(ui: &mut Ui, app: &OnMuhasebeApp) -> Option<Action> {
    let pending = app.cash_state().payment_pending.as_ref()?;
    let mut action = None;

    widgets::card(ui, "Kaydı kesinleştir", |ui| {
        ui.add(
            Label::new(
                RichText::new(
                    "Tahsilat/Ödeme kesinleşecek. Kasa bakiyesi ve fatura ödemeleri \
                     güncellenecek. Onaylıyor musunuz?",
                )
                .size(13.5)
                .color(Palette::TEXT),
            )
            .wrap(),
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new(format!(
                "{} · Tutar: {} · Dağıtılan: {} · Avans: {}",
                pending.payment_type.label(),
                pending.amount.format_with_symbol(),
                pending.allocated_total().format_with_symbol(),
                pending.remainder().format_with_symbol()
            ))
            .size(12.5)
            .color(Palette::MUTED),
        );

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui
                .add(Button::new("Onayla ve Kaydet").min_size(vec2(170.0, 30.0)))
                .clicked()
            {
                action = Some(Action::ConfirmPaymentSave);
            }
            if ui
                .add(Button::new("Vazgeç").min_size(vec2(120.0, 30.0)))
                .clicked()
            {
                action = Some(Action::DiscardPaymentConfirmation);
            }
        });
    });

    action
}

/// Fatura dağıtım paneli.
fn allocation_panel(ui: &mut Ui, app: &mut OnMuhasebeApp, action: &mut Option<Action>) {
    let mut allocate_all = false;

    widgets::card(ui, "Fatura dağıtımı", |ui| {
        ui.add(
            Label::new(
                RichText::new(
                    "Seçili cariye ait, türü uyan kesin faturalar listelenir. Dağıtılacak \
                     miktarı elle girebilirsiniz; dağıtılmayan tutar avans sayılır.",
                )
                .size(12.0)
                .color(Palette::MUTED),
            )
            .wrap(),
        );
        ui.add_space(6.0);

        if let Some(message) = app
            .cash_state()
            .payment_errors
            .get(PaymentField::Allocations)
        {
            ui.add(Label::new(RichText::new(message).size(12.0).color(Palette::DANGER)).wrap());
            ui.add_space(4.0);
        }

        let empty = app.cash_state().payment_draft.allocations.is_empty();
        if empty {
            widgets::bullet(ui, "Bu cari için dağıtılabilecek fatura yok.");
            return;
        }

        {
            let state = app.cash_state_mut();
            let allocations = &mut state.payment_draft.allocations;
            let errors = &mut state.payment_errors;

            Grid::new("kasa_dagitim")
                .num_columns(6)
                .min_col_width(60.0)
                .spacing([8.0, 6.0])
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("No")
                            .size(11.5)
                            .strong()
                            .color(Palette::MUTED),
                    );
                    ui.label(
                        RichText::new("Tarih")
                            .size(11.5)
                            .strong()
                            .color(Palette::MUTED),
                    );
                    ui.label(
                        RichText::new("Genel")
                            .size(11.5)
                            .strong()
                            .color(Palette::MUTED),
                    );
                    ui.label(
                        RichText::new("Ödenen")
                            .size(11.5)
                            .strong()
                            .color(Palette::MUTED),
                    );
                    ui.label(
                        RichText::new("Kalan")
                            .size(11.5)
                            .strong()
                            .color(Palette::MUTED),
                    );
                    ui.label(
                        RichText::new("Dağıtılacak")
                            .size(11.5)
                            .strong()
                            .color(Palette::MUTED),
                    );
                    ui.end_row();

                    for (index, allocation) in allocations.iter_mut().enumerate() {
                        ui.label(
                            RichText::new(&allocation.invoice_number)
                                .monospace()
                                .size(11.5)
                                .color(Palette::ACCENT),
                        );
                        ui.label(
                            RichText::new(format_iso_date_short(&allocation.invoice_date))
                                .size(11.5)
                                .color(Palette::MUTED),
                        );
                        ui.label(RichText::new(allocation.grand_total.format_turkish()).size(11.5));
                        ui.label(
                            RichText::new(allocation.paid_amount.format_turkish())
                                .size(11.5)
                                .color(Palette::MUTED),
                        );
                        ui.label(
                            RichText::new(allocation.remaining().format_turkish())
                                .size(11.5)
                                .strong(),
                        );
                        ui.vertical(|ui| {
                            if ui
                                .add(
                                    TextEdit::singleline(&mut allocation.amount)
                                        .hint_text("0,00")
                                        .desired_width(80.0),
                                )
                                .changed()
                            {
                                errors.clear_allocation(index);
                            }
                            if let Some(message) = errors.allocation(index) {
                                ui.add(
                                    Label::new(
                                        RichText::new(message).size(11.0).color(Palette::DANGER),
                                    )
                                    .wrap(),
                                );
                            }
                        });
                        ui.end_row();
                    }
                });
        }

        ui.add_space(6.0);
        if ui.button("Tümünü dağıt").clicked() {
            allocate_all = true;
        }
    });

    if allocate_all {
        *action = Some(Action::AllocateAll);
    }
}

/// Form alt toplamları (ödeme tutarı, dağıtılan, kalan) ve uyarılar.
fn payment_footer(ui: &mut Ui, app: &OnMuhasebeApp) {
    let draft = &app.cash_state().payment_draft;
    let amount = draft.input_amount();
    let allocated = draft.allocation_input_total();
    let remainder = draft.input_remainder();

    let over = match (amount, allocated) {
        (Ok(amount), Some(allocated)) => allocated > amount,
        _ => false,
    };

    widgets::card(ui, "Özet", |ui| {
        Grid::new("kasa_form_ozet")
            .num_columns(2)
            .min_col_width(150.0)
            .spacing([16.0, 6.0])
            .show(ui, |ui| {
                widgets::row_label(ui, "Ödeme tutarı");
                ui.label(
                    RichText::new(match amount {
                        Ok(amount) => amount.format_with_symbol(),
                        Err(_) => "—".to_owned(),
                    })
                    .strong()
                    .color(Palette::TEXT),
                );
                ui.end_row();

                widgets::row_label(ui, "Dağıtılan toplam");
                ui.label(
                    RichText::new(match allocated {
                        Some(allocated) => allocated.format_with_symbol(),
                        None => "—".to_owned(),
                    })
                    .strong()
                    .color(Palette::TEXT),
                );
                ui.end_row();

                widgets::row_label(ui, "Kalan (avans)");
                ui.label(
                    RichText::new(match remainder {
                        Some(remainder) => remainder.format_with_symbol(),
                        None => "—".to_owned(),
                    })
                    .strong()
                    .color(Palette::ACCENT),
                );
                ui.end_row();
            });

        if over {
            ui.add_space(8.0);
            ui.add(
                Label::new(
                    RichText::new(
                        "Dağıtılan toplam ödeme tutarını aşıyor. Kaydetmek için dağıtımları \
                         azaltın.",
                    )
                    .color(Palette::DANGER),
                )
                .wrap(),
            );
        }
    });
}

// ---------------------------------------------------------------------------
// Hareket ayrıntısı
// ---------------------------------------------------------------------------

/// Tahsilat/ödeme ayrıntısı.
fn movement_detail(ui: &mut Ui, app: &mut OnMuhasebeApp) {
    let mut action: Option<Action> = None;

    back_header(ui, "← Hareketler", "Kayıt ayrıntısı", &mut action);

    ui.add_space(metrics::SECTION_GAP);
    if messages(ui, app, &mut action) {
        ui.add_space(metrics::SECTION_GAP);
    }

    let payment = app.cash_state().detail.clone();
    let allocations = app.cash_state().detail_allocations.clone();
    let cancelling = app.cash_state().cancelling;

    let Some(payment) = payment else {
        widgets::notice_card(ui, "Kayıt bulunamadı", "Kayıt okunamadı; listeye dönün.");
        if let Some(action) = action {
            action.apply(app);
        }
        return;
    };

    payment_header(ui, &payment, &mut action);
    ui.add_space(metrics::SECTION_GAP);
    allocations_card(ui, &allocations, &mut action);
    ui.add_space(metrics::SECTION_GAP);
    payment_cancel_card(ui, app, &payment, cancelling, &mut action);

    if let Some(action) = action {
        action.apply(app);
    }
}

/// Ayrıntı üst bilgi kartı.
fn payment_header(ui: &mut Ui, payment: &Payment, action: &mut Option<Action>) {
    widgets::card(ui, "Kayıt bilgileri", |ui| {
        Sides::new().height(24.0).shrink_left().show(
            ui,
            |ui| {
                ui.label(
                    RichText::new(payment.payment_type.label())
                        .size(15.0)
                        .strong()
                        .color(match payment.payment_type {
                            PaymentType::Tahsilat => Palette::SUCCESS,
                            PaymentType::Odeme => Palette::DANGER,
                        }),
                );
                ui.label(
                    RichText::new(format_iso_date_short(&payment.payment_date))
                        .size(12.5)
                        .color(Palette::MUTED),
                );
            },
            |ui| payment_status_badge(ui, payment.status),
        );

        ui.add_space(8.0);

        Grid::new("kasa_detay_bilgi")
            .num_columns(2)
            .min_col_width(150.0)
            .spacing([16.0, 6.0])
            .show(ui, |ui| {
                widgets::row_label(ui, "Cari hesap");
                if widgets::link(
                    ui,
                    RichText::new(format!(
                        "{} — {}",
                        payment.customer_code, payment.customer_name
                    )),
                )
                .clicked()
                {
                    *action = Some(Action::OpenCustomer(payment.customer_id));
                }
                ui.end_row();

                widgets::row_label(ui, "Kasa/banka");
                if widgets::link(ui, RichText::new(&payment.cash_account_name)).clicked() {
                    *action = Some(Action::HighlightAccount(payment.cash_account_id));
                }
                ui.end_row();

                widgets::row_label(ui, "Tutar");
                ui.label(
                    RichText::new(payment.amount.format_with_symbol())
                        .size(15.0)
                        .strong()
                        .color(Palette::TEXT),
                );
                ui.end_row();

                widgets::row_label(ui, "Dağıtılan");
                ui.label(payment.allocated_total.format_with_symbol());
                ui.end_row();

                if payment.remainder() > Money::ZERO {
                    widgets::row_label(ui, "Avans");
                    ui.label(
                        RichText::new(payment.remainder().format_with_symbol())
                            .strong()
                            .color(Palette::ACCENT),
                    );
                    ui.end_row();
                }

                if let Some(description) = &payment.description {
                    widgets::row_label(ui, "Açıklama");
                    ui.add(Label::new(description).wrap());
                    ui.end_row();
                }

                widgets::row_label(ui, "Kayıt zamanı");
                ui.label(format!("{} (UTC)", payment.created_at));
                ui.end_row();
            });
    });
}

/// Dağıtım listesi kartı.
fn allocations_card(ui: &mut Ui, allocations: &[PaymentAllocation], action: &mut Option<Action>) {
    widgets::card(ui, "Fatura dağıtımları", |ui| {
        if allocations.is_empty() {
            widgets::bullet(ui, "Bu kayıt hiçbir faturaya dağıtılmamış (tam avans).");
            return;
        }

        Grid::new("kasa_detay_dagitim")
            .num_columns(4)
            .min_col_width(110.0)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label(
                    RichText::new("Numara")
                        .size(12.0)
                        .strong()
                        .color(Palette::MUTED),
                );
                ui.label(
                    RichText::new("Tarih")
                        .size(12.0)
                        .strong()
                        .color(Palette::MUTED),
                );
                ui.label(
                    RichText::new("Genel")
                        .size(12.0)
                        .strong()
                        .color(Palette::MUTED),
                );
                ui.label(
                    RichText::new("Bu kayıttan")
                        .size(12.0)
                        .strong()
                        .color(Palette::MUTED),
                );
                ui.end_row();

                for allocation in allocations {
                    if widgets::link(
                        ui,
                        RichText::new(&allocation.invoice_number)
                            .monospace()
                            .size(12.0),
                    )
                    .clicked()
                    {
                        *action = Some(Action::OpenInvoice(allocation.invoice_id));
                    }
                    ui.label(
                        RichText::new(format_iso_date_short(&allocation.invoice_date))
                            .size(12.0)
                            .color(Palette::MUTED),
                    );
                    ui.label(
                        RichText::new(allocation.invoice_grand_total.format_with_symbol())
                            .size(12.0),
                    );
                    ui.label(
                        RichText::new(allocation.amount.format_with_symbol())
                            .size(12.0)
                            .strong(),
                    );
                    ui.end_row();
                }
            });
    });
}

/// İptal bölümü: iptal edilmişse bilgi, değilse iptal düğmesi/onayı.
fn payment_cancel_card(
    ui: &mut Ui,
    app: &mut OnMuhasebeApp,
    payment: &Payment,
    cancelling: bool,
    action: &mut Option<Action>,
) {
    if payment.is_cancelled() {
        widgets::card(ui, "İptal bilgisi", |ui| {
            Grid::new("kasa_iptal_bilgi")
                .num_columns(2)
                .min_col_width(150.0)
                .spacing([16.0, 6.0])
                .show(ui, |ui| {
                    widgets::row_label(ui, "İptal zamanı");
                    ui.label(format!(
                        "{} (UTC)",
                        payment.cancelled_at.as_deref().unwrap_or("—")
                    ));
                    ui.end_row();

                    widgets::row_label(ui, "İptal nedeni");
                    ui.add(Label::new(payment.cancel_reason.as_deref().unwrap_or("—")).wrap());
                    ui.end_row();
                });
        });
        return;
    }

    if !app.is_manager() {
        return;
    }

    if !cancelling {
        ui.horizontal(|ui| {
            if ui
                .add(Button::new("İptal et").min_size(vec2(150.0, 30.0)))
                .on_hover_text(
                    "Kayıt silinmez; iptal edilir. Kasa bakiyesi ve fatura ödemeleri geri alınır.",
                )
                .clicked()
            {
                *action = Some(Action::StartPaymentCancel);
            }
            if ui
                .add(Button::new("Listeye dön").min_size(vec2(150.0, 30.0)))
                .clicked()
            {
                *action = Some(Action::BackToMovementList);
            }
        });
        return;
    }

    widgets::card(ui, "Kaydı iptal et", |ui| {
        ui.add(
            Label::new(
                RichText::new(
                    "Bu tahsilat/ödeme iptal edilecek; kasa bakiyesi ve fatura ödemeleri \
                     geri alınacak. İptal nedeni zorunludur.",
                )
                .size(13.5)
                .color(Palette::TEXT),
            )
            .wrap(),
        );
        ui.add_space(6.0);

        let state = app.cash_state_mut();
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
                *action = Some(Action::SubmitPaymentCancel);
            }
            if ui
                .add(Button::new("Vazgeç").min_size(vec2(120.0, 30.0)))
                .clicked()
            {
                *action = Some(Action::AbortPaymentCancel);
            }
        });
    });
}

/// Fatura ayrıntısında kullanılan ortak dağıtım listesi (paylaşılan çizim).
pub(crate) fn invoice_payment_links(ui: &mut Ui, links: &[InvoicePaymentLink]) {
    if links.is_empty() {
        widgets::bullet(ui, "Bu faturaya henüz ödeme kaydı yok.");
        return;
    }

    for link in links {
        Sides::new().height(22.0).shrink_left().show(
            ui,
            |ui| {
                ui.label(
                    RichText::new(format_iso_date_short(&link.payment_date))
                        .monospace()
                        .size(12.5)
                        .color(Palette::MUTED),
                );
                ui.label(
                    RichText::new(link.payment_type.label())
                        .size(12.5)
                        .strong()
                        .color(match link.payment_type {
                            PaymentType::Tahsilat => Palette::SUCCESS,
                            PaymentType::Odeme => Palette::DANGER,
                        }),
                );
                ui.add(
                    Label::new(
                        RichText::new(&link.cash_account_name)
                            .size(12.5)
                            .color(Palette::MUTED),
                    )
                    .truncate(),
                );
            },
            |ui| {
                ui.label(
                    RichText::new(link.amount.format_with_symbol())
                        .size(12.5)
                        .strong()
                        .color(Palette::TEXT),
                );
                ui.add_space(8.0);
                payment_status_badge(ui, link.status);
            },
        );
    }
}
