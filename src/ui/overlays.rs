// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Modal katmanları: kurulum sihirbazı, global arama, kısayol yardımı,
//! iptal/çıkış onayları ve hata ayrıntıları.

use egui::{
    Align, Context, Frame, Id, Key, Label, Layout, Modifiers, RichText, ScrollArea, Stroke,
    TextEdit,
};

use crate::app::{FocusTarget, OnMuhasebeApp, SHORTCUTS, ShortcutAction};
use crate::cash::{AccountType, Field as CashField};
use crate::income_expense::EntryType;
use crate::onboarding::{CATEGORY_PACKAGES, OnboardingStep};
use crate::ui::theme::Palette;

/// Tüm modal katmanlarını çizer. Çağrı, panellerden **sonra** yapılmalıdır.
pub fn show(context: &Context, app: &mut OnMuhasebeApp) {
    onboarding(context, app);
    onboarding_skip_confirm(context, app);
    search_palette(context, app);
    shortcuts_help(context, app);
    cancel_confirm(context, app);
    logout_confirm(context, app);
    error_details(context, app);
}

/// Yarıda kalan geri yükleme için üç seçenekli karar penceresi.
///
/// Bu pencere oturum açılmadan önce de gösterilebilmelidir; bu nedenle
/// [`show`] içinde değil, doğrudan uygulama döngüsünden çağrılır.
pub fn show_restore_recovery(context: &Context, app: &mut OnMuhasebeApp) {
    let Some(journal) = app.restore_recovery().cloned() else {
        return;
    };

    let mut use_safety = false;
    let mut continue_current = false;
    let mut close_app = false;

    egui::Modal::new(Id::unique("restore_recovery_modal"))
        .frame(modal_frame())
        .show(context, |ui| {
            ui.set_width(580.0);
            ui.set_max_width(580.0);
            ui.label(
                RichText::new("Yarıda kalan geri yükleme")
                    .size(20.0)
                    .strong()
                    .color(Palette::TEXT),
            );
            ui.add_space(6.0);
            ui.label(
                "Son geri yükleme işlemi yarıda kaldı. Ne yapmak istersiniz? \
                 Hiçbir seçenek sizin onayınız olmadan uygulanmaz.",
            );
            ui.add_space(8.0);

            egui::Grid::new("restore_recovery_grid")
                .num_columns(2)
                .spacing([12.0, 4.0])
                .show(ui, |ui| {
                    for (label, value) in [
                        ("Başlangıç", journal.start.clone()),
                        ("Kalan adım", journal.step.as_str().to_owned()),
                        ("Kaynak yedek", journal.source_backup.display().to_string()),
                        (
                            "Güvenlik yedeği",
                            journal.pre_restore_backup.display().to_string(),
                        ),
                        ("Hedef veritabanı", journal.target_db.display().to_string()),
                    ] {
                        ui.label(RichText::new(label).size(12.0).color(Palette::MUTED));
                        ui.add(Label::new(RichText::new(value).monospace().size(11.0)).wrap());
                        ui.end_row();
                    }
                });

            if let Some(error) = app.restore_recovery_error() {
                ui.add_space(8.0);
                ui.label(RichText::new(error).color(Palette::DANGER).size(12.5));
            }

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Güvenlik yedeğine dön").clicked() {
                    use_safety = true;
                }
                if ui.button("Mevcut veritabanıyla devam et").clicked() {
                    continue_current = true;
                }
                if ui.button("Uygulamayı kapat").clicked() {
                    close_app = true;
                }
            });
        });

    if close_app {
        // Günlük korunur; uygulama yeniden açıldığında soru tekrar sorulur.
        context.send_viewport_cmd(egui::ViewportCommand::Close);
        return;
    }
    if use_safety {
        let _ = app.restore_recovery_with_safety_backup();
    }
    if continue_current {
        let _ = app.restore_recovery_continue();
    }
}

fn modal_frame() -> Frame {
    Frame::new()
        .fill(Palette::SURFACE)
        .inner_margin(18)
        .corner_radius(10)
        .stroke(Stroke::new(1.0, Palette::BORDER))
}

fn onboarding(context: &Context, app: &mut OnMuhasebeApp) {
    if !app.onboarding_state().open {
        return;
    }

    let mut close_skip = false;
    let mut next = false;
    let mut back = false;
    let mut finish = false;

    egui::Modal::new(Id::unique("onboarding_modal"))
        .frame(modal_frame())
        .show(context, |ui| {
            ui.set_width(560.0);
            ui.set_max_width(560.0);
            ui.label(
                RichText::new("Kurulum sihirbazı")
                    .size(20.0)
                    .strong()
                    .color(Palette::TEXT),
            );
            ui.label(
                RichText::new(format!(
                    "Adım {} / 4 · {}",
                    app.onboarding_state().step.number(),
                    app.onboarding_state().step.title()
                ))
                .size(12.5)
                .color(Palette::MUTED),
            );
            ui.add_space(8.0);
            step_indicator(ui, app.onboarding_state().step);
            ui.add_space(10.0);

            ScrollArea::vertical().max_height(330.0).show(ui, |ui| {
                match app.onboarding_state().step {
                    OnboardingStep::Company => company_step(ui, app),
                    OnboardingStep::Cash => cash_step(ui, app),
                    OnboardingStep::Categories => categories_step(ui, app),
                    OnboardingStep::Summary => summary_step(ui, app),
                }
            });

            if let Some(error) = app.onboarding_state().error.clone() {
                ui.add_space(8.0);
                ui.label(RichText::new(error).color(Palette::DANGER).size(12.5));
            }

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Daha sonra").clicked() {
                    close_skip = true;
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let last = app.onboarding_state().step == OnboardingStep::Summary;
                    if last {
                        if ui.button("Tamamla").clicked() {
                            finish = true;
                        }
                    } else if ui.button("İleri →").clicked() {
                        next = true;
                    }
                    if app.onboarding_state().step != OnboardingStep::Company
                        && ui.button("← Geri").clicked()
                    {
                        back = true;
                    }
                });
            });
        });

    if context.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape))
        && !app.onboarding_state().skip_confirm
    {
        close_skip = true;
    }
    if close_skip {
        app.request_onboarding_skip();
    }
    if back {
        app.onboarding_previous();
    }
    if next {
        app.onboarding_next();
    }
    if finish {
        app.onboarding_finish();
    }
}

fn step_indicator(ui: &mut egui::Ui, step: OnboardingStep) {
    ui.horizontal(|ui| {
        for candidate in OnboardingStep::ALL {
            let active = candidate.number() <= step.number();
            let color = if active {
                Palette::ACCENT
            } else {
                Palette::BORDER
            };
            let (rect, _) = ui.allocate_exact_size(egui::vec2(56.0, 5.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 3.0, color);
        }
    });
}

fn company_step(ui: &mut egui::Ui, app: &mut OnMuhasebeApp) {
    ui.label(
        RichText::new("Bu bilgiler isteğe bağlıdır; daha sonra Ayarlar'dan değiştirebilirsiniz.")
            .size(12.0)
            .color(Palette::MUTED),
    );
    ui.add_space(8.0);

    let focus = app.take_focus(FocusTarget::OnboardingCompanyName);
    let response = {
        let state = app.onboarding_state_mut();
        field(
            ui,
            "Ticari unvan",
            &mut state.company.name,
            "Örn. Örnek Ticaret Ltd. Şti.",
        )
    };
    if focus {
        response.request_focus();
    }
    {
        let state = app.onboarding_state_mut();
        field(
            ui,
            "Yetkili kişi",
            &mut state.company.contact_person,
            "Ad Soyad",
        );
        field(ui, "Telefon", &mut state.company.phone, "0212 000 00 00");
        field(ui, "E-posta", &mut state.company.email, "info@ornek.com");
        field(ui, "Adres", &mut state.company.address, "Açık adres");
        field(
            ui,
            "Vergi dairesi",
            &mut state.company.tax_office,
            "Örn. Beşiktaş",
        );
        field(
            ui,
            "Vergi numarası",
            &mut state.company.tax_number,
            "1234567890",
        );
    }
}

fn cash_step(ui: &mut egui::Ui, app: &mut OnMuhasebeApp) {
    ui.label(
        RichText::new("İlk kasa veya banka hesabınızı ekleyin. Boş bırakırsanız bu adım atlanır.")
            .size(12.0)
            .color(Palette::MUTED),
    );
    ui.add_space(8.0);

    let focus = app.take_focus(FocusTarget::OnboardingCashName);
    let response = {
        let state = app.onboarding_state_mut();
        field(
            ui,
            "Hesap adı",
            &mut state.cash_draft.name,
            "Örn. Merkez Kasa",
        )
    };
    if focus {
        response.request_focus();
    }

    let mut account_type = app.onboarding_state().cash_draft.account_type;
    egui::ComboBox::from_id_salt("onboarding_account_type")
        .selected_text(account_type.label())
        .show_ui(ui, |ui| {
            for candidate in AccountType::ALL {
                ui.selectable_value(&mut account_type, candidate, candidate.label());
            }
        });
    app.onboarding_state_mut().cash_draft.account_type = account_type;

    if account_type == AccountType::Banka {
        let state = app.onboarding_state_mut();
        field(
            ui,
            "Banka adı",
            &mut state.cash_draft.bank_name,
            "Örn. Örnek Bank",
        );
        field(
            ui,
            "IBAN",
            &mut state.cash_draft.iban,
            "TR00 0000 0000 0000 0000 0000 00",
        );
    }

    {
        let state = app.onboarding_state_mut();
        field(
            ui,
            "Açılış bakiyesi",
            &mut state.cash_draft.opening_balance,
            "0,00",
        );
        field(ui, "Notlar", &mut state.cash_draft.notes, "İsteğe bağlı");
    }

    if let Some(message) = app
        .onboarding_state()
        .cash_errors
        .get(CashField::Name)
        .or_else(|| app.onboarding_state().cash_errors.get(CashField::Iban))
    {
        ui.label(RichText::new(message).size(11.5).color(Palette::DANGER));
    }
}

fn categories_step(ui: &mut egui::Ui, app: &mut OnMuhasebeApp) {
    ui.label(
        RichText::new(
            "Sık kullanılan gelir/gider kategorilerini seçin. Sonradan ekleyebilirsiniz.",
        )
        .size(12.0)
        .color(Palette::MUTED),
    );
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui.button("Tümünü seç").clicked() {
            app.onboarding_set_all_categories(true);
        }
        if ui.button("Seçimi kaldır").clicked() {
            app.onboarding_set_all_categories(false);
        }
    });
    ui.add_space(4.0);

    let mut toggles = Vec::new();
    {
        let state = app.onboarding_state_mut();
        for (index, package) in CATEGORY_PACKAGES.iter().enumerate() {
            let mut selected = state
                .selected_categories
                .get(index)
                .copied()
                .unwrap_or(false);
            let label = format!("{} · {}", package.name, package.category_type.label());
            if ui.checkbox(&mut selected, label).changed() {
                toggles.push(index);
            }
        }
    }
    for index in toggles {
        app.onboarding_toggle_category(index);
    }
}

fn summary_step(ui: &mut egui::Ui, app: &OnMuhasebeApp) {
    let state = app.onboarding_state();
    ui.label(RichText::new("Şirket").strong());
    ui.label(state.company.display_name());
    if !state.company.tax_number.trim().is_empty() {
        ui.label(
            RichText::new(format!(
                "Vergi no: {} · Vergi dairesi: {}",
                state.company.tax_number.trim(),
                state.company.tax_office.trim()
            ))
            .size(12.0)
            .color(Palette::MUTED),
        );
    }
    ui.add_space(8.0);

    ui.label(RichText::new("Kasa / Banka").strong());
    if state.cash_draft.name.trim().is_empty() {
        ui.label(RichText::new("Hesap eklenmeyecek.").color(Palette::MUTED));
    } else {
        ui.label(format!(
            "{} · {}",
            state.cash_draft.name.trim(),
            state.cash_draft.account_type.label()
        ));
    }
    ui.add_space(8.0);

    ui.label(RichText::new("Kategoriler").strong());
    ui.label(format!(
        "{} gelir, {} gider kategorisi eklenecek.",
        app.onboarding_selected_category_count(EntryType::Gelir),
        app.onboarding_selected_category_count(EntryType::Gider)
    ));
    ui.add_space(8.0);

    ui.label(
        RichText::new("Tamamla düğmesine bastığınızda tüm kayıtlar tek işlemde yazılır.")
            .size(12.0)
            .color(Palette::MUTED),
    );
}

fn onboarding_skip_confirm(context: &Context, app: &mut OnMuhasebeApp) {
    if !app.onboarding_state().open || !app.onboarding_state().skip_confirm {
        return;
    }

    let mut cancel = false;
    let mut confirm = false;
    let response = egui::Modal::new(Id::unique("onboarding_skip_modal"))
        .frame(modal_frame())
        .show(context, |ui| {
            ui.set_width(380.0);
            ui.label(RichText::new("Kurulumu atla?").size(17.0).strong());
            ui.add_space(6.0);
            ui.label(
                "Şirket bilgileri, kasa hesabı ve kategoriler eklenmeyecek. Sihirbazı daha sonra \
                 Ayarlar > Genel bölümünden açabilirsiniz.",
            );
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui.button("Vazgeç").clicked() {
                    cancel = true;
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Kurulumu atla").clicked() {
                        confirm = true;
                    }
                });
            });
        });

    if response.should_close() {
        cancel = true;
    }
    if cancel {
        app.cancel_onboarding_skip();
    }
    if confirm {
        app.confirm_onboarding_skip();
    }
}

fn search_palette(context: &Context, app: &mut OnMuhasebeApp) {
    if !app.search_state().open {
        return;
    }

    let has_results = !app.search_state().results.is_empty();
    let down = has_results
        && context.input_mut(|input| input.consume_key(Modifiers::NONE, Key::ArrowDown));
    let up =
        has_results && context.input_mut(|input| input.consume_key(Modifiers::NONE, Key::ArrowUp));
    let enter =
        has_results && context.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Enter));
    let escape = context.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape));

    let mut changed = false;
    let mut clicked: Option<usize> = None;

    egui::Modal::new(Id::unique("search_palette_modal"))
        .frame(modal_frame())
        .show(context, |ui| {
            ui.set_width(600.0);
            ui.set_max_width(600.0);
            ui.label(RichText::new("Global arama").size(17.0).strong());
            ui.add_space(6.0);

            let focus = app.take_focus(FocusTarget::SearchQuery);
            let response = {
                let state = app.search_state_mut();
                ui.add(
                    TextEdit::singleline(&mut state.query)
                        .hint_text("Cari, ürün, fatura numarası veya hesap ara...")
                        .desired_width(f32::INFINITY),
                )
            };
            if focus {
                response.request_focus();
            }
            changed = response.changed();

            ui.add_space(4.0);
            if app.search_state().changed_at.is_some() || app.search_state().searching {
                ui.label(
                    RichText::new("Aranıyor...")
                        .size(11.5)
                        .color(Palette::MUTED),
                );
            }

            let normalized_len = crate::customer::normalize_for_search(&app.search_state().query)
                .chars()
                .count();
            if normalized_len < 2 {
                ui.add_space(6.0);
                ui.label(
                    RichText::new("Aramak için en az 2 karakter yazın.")
                        .size(12.0)
                        .color(Palette::MUTED),
                );
            }

            if let Some(error) = app.search_state().error.clone() {
                ui.add_space(6.0);
                ui.label(RichText::new(error).size(12.0).color(Palette::DANGER));
            }

            let grouped = app.search_grouped_results();
            if !grouped.is_empty() {
                ui.add_space(6.0);
                ScrollArea::vertical().max_height(330.0).show(ui, |ui| {
                    let mut index = 0;
                    for (group, hits) in grouped {
                        ui.label(
                            RichText::new(group.label())
                                .size(11.0)
                                .strong()
                                .color(Palette::MUTED),
                        );
                        for hit in hits {
                            let selected = index == app.search_state().selected;
                            let text = format!("{}  ·  {}", hit.title, hit.subtitle);
                            let response = ui.selectable_label(
                                selected,
                                RichText::new(text).size(13.0).color(Palette::TEXT),
                            );
                            if response.clicked() {
                                clicked = Some(index);
                            }
                            index += 1;
                        }
                        ui.add_space(4.0);
                    }
                });
            } else if normalized_len >= 2
                && app.search_state().changed_at.is_none()
                && !app.search_state().last_query.is_empty()
                && app.search_state().error.is_none()
            {
                ui.add_space(6.0);
                ui.label(
                    RichText::new("Sonuç bulunamadı.")
                        .size(12.0)
                        .color(Palette::MUTED),
                );
            }

            ui.add_space(8.0);
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("↑↓ seç · Enter aç · Esc kapat")
                        .size(11.0)
                        .color(Palette::MUTED),
                );
            });
        });

    if changed {
        app.search_query_changed();
    }
    if down {
        app.search_move(1);
    }
    if up {
        app.search_move(-1);
    }
    if escape {
        app.close_search_palette();
        return;
    }
    if enter {
        app.search_activate_selected();
        return;
    }
    if let Some(index) = clicked {
        app.search_state_mut().selected = index;
        app.search_activate_selected();
    }
}

fn shortcuts_help(context: &Context, app: &mut OnMuhasebeApp) {
    if !app.shortcuts_open {
        return;
    }

    let response = egui::Modal::new(Id::unique("shortcuts_modal"))
        .frame(modal_frame())
        .show(context, |ui| {
            ui.set_width(520.0);
            ui.label(RichText::new("Klavye kısayolları").size(19.0).strong());
            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "Metin alanı odaklıyken yalnızca global kısayollar (Ctrl+K, Ctrl+F, F5, Ctrl+Q) çalışır.",
                )
                .size(12.0)
                .color(Palette::MUTED),
            );
            ui.add_space(8.0);
            ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
                let mut current_group = "";
                for shortcut in SHORTCUTS {
                    if shortcut.group != current_group {
                        current_group = shortcut.group;
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(current_group)
                                .size(11.0)
                                .strong()
                                .color(Palette::MUTED),
                        );
                    }
                    ui.horizontal(|ui| {
                        ui.add_sized(
                            [90.0, 20.0],
                            Label::new(
                                RichText::new(shortcut.keys_label())
                                    .monospace()
                                    .size(12.5)
                                    .color(Palette::TEXT),
                            ),
                        );
                        ui.label(RichText::new(shortcut.label).size(12.5));
                        if shortcut.action == ShortcutAction::Search && shortcut.key == Key::F {
                            ui.label(
                                RichText::new("(Ctrl+K ile aynı)")
                                    .size(11.0)
                                    .color(Palette::MUTED),
                            );
                        }
                    });
                }
            });
            ui.add_space(8.0);
            if ui.button("Kapat").clicked() {
                app.shortcuts_open = false;
            }
        });

    if response.should_close() {
        app.shortcuts_open = false;
    }
}

fn cancel_confirm(context: &Context, app: &mut OnMuhasebeApp) {
    let Some(target) = app.cancel_confirm else {
        return;
    };

    let mut dismiss = false;
    let mut confirm = false;
    let response = egui::Modal::new(Id::unique("cancel_confirm_modal"))
        .frame(modal_frame())
        .show(context, |ui| {
            ui.set_width(400.0);
            ui.label(
                RichText::new("Kaydedilmemiş değişiklikler")
                    .size(17.0)
                    .strong(),
            );
            ui.add_space(6.0);
            ui.label(format!(
                "{} formunda kaydedilmemiş değişiklikler var. Çıkmak istiyor musunuz?",
                target.label()
            ));
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui.button("Formda kal").clicked() {
                    dismiss = true;
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Değişiklikleri at").clicked() {
                        confirm = true;
                    }
                });
            });
        });

    if response.should_close() {
        dismiss = true;
    }
    if dismiss {
        app.dismiss_cancel_confirmation();
    }
    if confirm {
        app.confirm_cancel_current_form();
    }
}

fn logout_confirm(context: &Context, app: &mut OnMuhasebeApp) {
    if !app.logout_confirm {
        return;
    }

    let mut dismiss = false;
    let mut confirm = false;
    let response = egui::Modal::new(Id::unique("logout_confirm_modal"))
        .frame(modal_frame())
        .show(context, |ui| {
            ui.set_width(380.0);
            ui.label(RichText::new("Oturumu kapat").size(17.0).strong());
            ui.add_space(6.0);
            ui.label("Kaydedilmemiş formlar varsa kaybolur. Oturumu kapatmak istiyor musunuz?");
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui.button("Vazgeç").clicked() {
                    dismiss = true;
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Oturumu kapat").clicked() {
                        confirm = true;
                    }
                });
            });
        });

    if response.should_close() {
        dismiss = true;
    }
    if dismiss {
        app.logout_confirm = false;
    }
    if confirm {
        app.logout_confirm = false;
        app.logout();
    }
}

fn error_details(context: &Context, app: &mut OnMuhasebeApp) {
    if !app.error_details_open {
        return;
    }

    let mut close = false;
    let response = egui::Modal::new(Id::unique("error_details_modal"))
        .frame(modal_frame())
        .show(context, |ui| {
            ui.set_width(520.0);
            ui.label(RichText::new("Hata ayrıntıları").size(18.0).strong());
            ui.add_space(6.0);
            let message = app
                .last_error_summary()
                .unwrap_or_else(|| "Etkin bir hata kaydı yok.".to_owned());
            ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                ui.add(Label::new(RichText::new(message).monospace().size(12.0)).wrap());
            });
            ui.add_space(8.0);
            ui.label(
                RichText::new("Log klasörü")
                    .size(12.0)
                    .color(Palette::MUTED),
            );
            ui.add(
                Label::new(
                    RichText::new(app.logs_dir().display().to_string())
                        .monospace()
                        .size(11.5),
                )
                .wrap(),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Log klasörünü aç").clicked() {
                    app.open_logs_folder();
                }
                if ui.button("Kapat").clicked() {
                    close = true;
                }
            });
        });

    if response.should_close() {
        close = true;
    }
    if close {
        app.error_details_open = false;
    }
}

fn field(ui: &mut egui::Ui, label: &str, value: &mut String, hint: &str) -> egui::Response {
    ui.add_space(4.0);
    ui.label(RichText::new(label).size(12.0).color(Palette::MUTED));
    ui.add(
        TextEdit::singleline(value)
            .hint_text(hint)
            .desired_width(f32::INFINITY),
    )
}
