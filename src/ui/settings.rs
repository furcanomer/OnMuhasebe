// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Ayarlar bölümünün dört sekmesi.
//!
//! Genel ve Yedekleme yalnızca yöneticiye, Kullanıcılar yalnızca yöneticiye
//! açıktır. Operatör kendi şifresini Güvenlik sekmesinden değiştirebilir.

use egui::{ComboBox, RichText, ScrollArea, TextEdit, Ui};

use crate::app::{OnMuhasebeApp, SettingsTab};
use crate::auth::Role;
use crate::backup::BackupKind;
use crate::ui::format::format_bytes;
use crate::ui::theme::Palette;
use crate::ui::widgets;

const USER_ROW_HEIGHT: f32 = 44.0;

/// Ayarlar ekranını çizer.
pub fn show(ui: &mut Ui, app: &mut OnMuhasebeApp) {
    widgets::heading(ui, "Ayarlar");
    ui.label(
        RichText::new("Uygulama, güvenlik, yedekleme ve kullanıcı ayarları.")
            .size(12.5)
            .color(Palette::MUTED),
    );
    ui.add_space(10.0);

    let manager = app.is_manager();
    let current = app.settings_state().tab;
    let mut selected = current;
    ui.horizontal_wrapped(|ui| {
        for tab in [
            SettingsTab::General,
            SettingsTab::Security,
            SettingsTab::Backup,
            SettingsTab::Users,
        ] {
            if !manager && tab != SettingsTab::Security {
                continue;
            }
            if ui.selectable_label(current == tab, tab.label()).clicked() {
                selected = tab;
            }
        }
    });
    if selected != current {
        app.settings_state_mut().tab = selected;
    }
    ui.separator();
    ui.add_space(8.0);

    let tab = if manager {
        app.settings_state().tab
    } else {
        SettingsTab::Security
    };
    match tab {
        SettingsTab::General => general(ui, app),
        SettingsTab::Security => security(ui, app),
        SettingsTab::Backup => backup(ui, app),
        SettingsTab::Users => users(ui, app),
    }
}

fn general(ui: &mut Ui, app: &mut OnMuhasebeApp) {
    widgets::card(ui, "Genel", |ui| {
        let current = app.settings().clone();
        let mut enabled = current.auto_lock_enabled;
        let mut minutes = current.auto_lock_minutes;
        let mut info = current.log_info;

        ui.horizontal(|ui| {
            ui.label("Dil:");
            ui.label(RichText::new("Türkçe").strong());
            ui.label(
                RichText::new("(şimdilik tek dil)")
                    .size(11.5)
                    .color(Palette::MUTED),
            );
        });
        ui.add_space(8.0);
        if ui
            .checkbox(&mut enabled, "Hareketsizlik sonrası otomatik kilit")
            .changed()
        {
            app.settings_mut().auto_lock_enabled = enabled;
        }
        ui.add_enabled_ui(enabled, |ui| {
            ui.horizontal(|ui| {
                ui.label("Kilit süresi:");
                ComboBox::from_id_salt("auto_lock_minutes")
                    .selected_text(format!("{minutes} dakika"))
                    .show_ui(ui, |ui| {
                        for option in [5, 15, 30, 60] {
                            ui.selectable_value(&mut minutes, option, format!("{option} dakika"));
                        }
                    });
            });
        });
        if minutes != current.auto_lock_minutes {
            app.settings_mut().auto_lock_minutes = minutes;
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label("Log seviyesi:");
            ComboBox::from_id_salt("log_level")
                .selected_text(if info {
                    "Info (ayrıntılı)"
                } else {
                    "Warn (varsayılan)"
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut info, false, "Warn (varsayılan)");
                    ui.selectable_value(&mut info, true, "Info (ayrıntılı)");
                });
        });
        if info != current.log_info {
            app.settings_mut().log_info = info;
        }
        ui.add_space(12.0);
        if ui.button("Ayarları kaydet").clicked() {
            {
                let settings = app.settings_mut();
                settings.auto_lock_enabled = enabled;
                settings.auto_lock_minutes = minutes;
                settings.log_info = info;
            }
            let _ = app.save_general_settings();
        }
        if let Some(error) = app.settings_state().settings_error.clone() {
            ui.add_space(6.0);
            ui.label(RichText::new(error).color(Palette::DANGER).size(12.5));
        }
        if let Some(notice) = app.settings_state().settings_notice.clone() {
            ui.add_space(6.0);
            ui.label(RichText::new(notice).color(Palette::SUCCESS).size(12.5));
        }
    });

    let company = app
        .database()
        .and_then(|database| crate::onboarding::CompanyInfo::load(database).ok())
        .unwrap_or_default();
    widgets::card(ui, "Şirket ve kurulum", |ui| {
        ui.label(
            RichText::new(company.display_name())
                .size(14.0)
                .strong()
                .color(Palette::TEXT),
        );
        if !company.tax_number.trim().is_empty() || !company.tax_office.trim().is_empty() {
            ui.label(
                RichText::new(format!(
                    "Vergi dairesi: {} · Vergi no: {}",
                    company.tax_office.trim(),
                    company.tax_number.trim()
                ))
                .size(12.0)
                .color(Palette::MUTED),
            );
        }
        if !company.phone.trim().is_empty() || !company.email.trim().is_empty() {
            ui.label(
                RichText::new(format!("{} {}", company.phone.trim(), company.email.trim()))
                    .size(12.0)
                    .color(Palette::MUTED),
            );
        }
        ui.add_space(8.0);
        if app.is_manager() && ui.button("Kurulum sihirbazını yeniden aç").clicked() {
            app.reopen_onboarding();
        }
        ui.label(
            RichText::new("Şirket bilgileri, ilk kasa hesabı ve hazır kategoriler.")
                .size(11.5)
                .color(Palette::MUTED),
        );
    });
}

fn security(ui: &mut Ui, app: &mut OnMuhasebeApp) {
    let manager = app.is_manager();
    let mut submit = false;
    widgets::card(ui, "Şifre değiştir", |ui| {
        ui.label(
            RichText::new("Kendi şifrenizi değiştirmek için mevcut şifrenizi girin.")
                .size(12.0)
                .color(Palette::MUTED),
        );
        ui.add_space(8.0);
        let state = app.settings_state_mut();
        let form = &mut state.password_form;
        ui.label("Mevcut şifre:");
        ui.add_sized(
            [ui.available_width().min(360.0), 28.0],
            TextEdit::singleline(&mut form.current).password(true),
        );
        ui.label("Yeni şifre:");
        ui.add_sized(
            [ui.available_width().min(360.0), 28.0],
            TextEdit::singleline(&mut form.new_password).password(true),
        );
        ui.label("Yeni şifre tekrarı:");
        ui.add_sized(
            [ui.available_width().min(360.0), 28.0],
            TextEdit::singleline(&mut form.confirm).password(true),
        );
        if let Some(error) = state.password_error.as_deref() {
            ui.add_space(5.0);
            ui.label(RichText::new(error).color(Palette::DANGER).size(12.5));
        }
        if let Some(notice) = state.password_notice.as_deref() {
            ui.add_space(5.0);
            ui.label(RichText::new(notice).color(Palette::SUCCESS).size(12.5));
        }
        ui.add_space(10.0);
        submit = ui.button("Şifreyi değiştir").clicked();
    });
    if submit {
        let _ = app.submit_password_change();
    }

    ui.add_space(8.0);
    widgets::card(ui, "Otomatik kilit", |ui| {
        let settings = app.settings();
        ui.label(format!(
            "Durum: {} · Süre: {} dakika",
            if settings.auto_lock_enabled {
                "Açık"
            } else {
                "Kapalı"
            },
            settings.auto_lock_minutes
        ));
        if manager {
            ui.label(
                RichText::new("Süreyi Genel sekmesinden değiştirebilirsiniz.")
                    .size(12.0)
                    .color(Palette::MUTED),
            );
        } else {
            ui.label(
                RichText::new("Bu ayarı yalnızca yönetici değiştirebilir.")
                    .size(12.0)
                    .color(Palette::MUTED),
            );
        }
    });
}

fn backup(ui: &mut Ui, app: &mut OnMuhasebeApp) {
    let settings = app.settings().clone();
    let total = app.settings_state().backup_total_bytes;
    let warning = app.backup_size_warning();
    let mut manual = false;
    let mut save = false;
    let mut restore: Option<std::path::PathBuf> = None;
    let entries = app.settings_state().backups.clone();
    let manager = app.is_manager();

    if warning {
        widgets::warning_card(
            ui,
            "Yedek boyutu uyarısı",
            &format!(
                "Toplam yedek boyutu {} ({} MB) sınırını aştı. Eski manuel yedekleri silebilirsiniz.",
                format_bytes(total),
                settings.backup_warn_mb
            ),
        );
        ui.add_space(8.0);
    }

    widgets::card(ui, "Yedekleme", |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label("Son otomatik yedek:");
            let last = settings
                .last_auto_backup_at
                .map(|value| value.format("%d.%m.%Y %H:%M").to_string())
                .unwrap_or_else(|| "Henüz yok".to_owned());
            ui.label(RichText::new(last).strong());
            ui.separator();
            ui.label("Toplam boyut:");
            ui.label(RichText::new(format_bytes(total)).strong());
        });
        ui.add_space(8.0);
        manual = ui.button("Şimdi yedek al").clicked();
        ui.add_space(10.0);
        ui.separator();
        widgets::subheading(ui, "Rotasyon (otomatik yedekler)");
        ui.horizontal(|ui| {
            ui.label("Günlük tut:");
            let mut daily = settings.rotation.daily_keep;
            if ui
                .add(egui::DragValue::new(&mut daily).range(1..=3650))
                .changed()
            {
                app.settings_mut().rotation.daily_keep = daily;
            }
            ui.label("gün");
        });
        ui.horizontal(|ui| {
            ui.label("Aylık tut:");
            let mut monthly = settings.rotation.monthly_keep;
            if ui
                .add(egui::DragValue::new(&mut monthly).range(1..=120))
                .changed()
            {
                app.settings_mut().rotation.monthly_keep = monthly;
            }
            ui.label("ay");
        });
        ui.horizontal(|ui| {
            ui.label("Yıllık tut:");
            let mut yearly = settings.rotation.yearly_keep;
            if ui
                .add(egui::DragValue::new(&mut yearly).range(1..=50))
                .changed()
            {
                app.settings_mut().rotation.yearly_keep = yearly;
            }
            ui.label("yıl");
        });
        ui.add_space(8.0);
        save = ui.button("Rotasyon ayarlarını kaydet").clicked();
        ui.label(
            RichText::new("Manuel yedekler rotasyona dahil değildir.")
                .size(11.5)
                .color(Palette::MUTED),
        );
        if let Some(error) = app.settings_state().settings_error.clone() {
            ui.label(RichText::new(error).color(Palette::DANGER).size(12.5));
        }
        if let Some(notice) = app.settings_state().settings_notice.clone() {
            ui.label(RichText::new(notice).color(Palette::SUCCESS).size(12.5));
        }
        if let Some(error) = app.settings_state().backup_error.clone() {
            ui.add_space(4.0);
            ui.label(RichText::new(error).color(Palette::DANGER).size(12.5));
        }
        if let Some(notice) = app.settings_state().backup_notice.clone() {
            ui.add_space(4.0);
            ui.label(RichText::new(notice).color(Palette::SUCCESS).size(12.5));
        }
    });

    ui.add_space(8.0);
    widgets::card(ui, "Son yedekler", |ui| {
        if entries.is_empty() {
            ui.label(RichText::new("Henüz yedek yok.").color(Palette::MUTED));
            return;
        }
        ScrollArea::vertical()
            .id_salt("yedek_listesi")
            .max_height(260.0)
            .show(ui, |ui| {
                for entry in &entries {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(entry.kind.label()).size(11.5).color(
                            if entry.kind == BackupKind::Auto {
                                Palette::ACCENT
                            } else {
                                Palette::MUTED
                            },
                        ));
                        ui.add_sized(
                            [260.0_f32.min(ui.available_width() * 0.45), 18.0],
                            egui::Label::new(entry.file_name.clone()).truncate(),
                        );
                        ui.label(
                            RichText::new(format_bytes(entry.size_bytes))
                                .size(11.5)
                                .color(Palette::MUTED),
                        );
                        if manager
                            && ui
                                .small_button("Bu yedeğe dön")
                                .on_hover_text("Yedekten geri yükleme onayı açılır")
                                .clicked()
                        {
                            restore = Some(entry.path.clone());
                        }
                    });
                }
            });
        ui.label(
            RichText::new("Geri yükleme yalnızca yönetici tarafından yapılabilir.")
                .size(11.5)
                .color(Palette::MUTED),
        );
    });

    if manual {
        let _ = app.create_manual_backup();
    }
    if save {
        let _ = app.save_backup_settings();
    }
    if let Some(path) = restore {
        let _ = app.prepare_restore(path);
    }
}

#[derive(Debug, Clone, Copy)]
enum UserAction {
    Edit(i64),
    Toggle(i64),
    Reset(i64),
    Delete(i64),
}

fn users(ui: &mut Ui, app: &mut OnMuhasebeApp) {
    if !app.is_manager() {
        ui.label(
            RichText::new("Kullanıcı yönetimi için yönetici yetkisi gerekir.")
                .color(Palette::DANGER),
        );
        return;
    }

    let current_id = app.current_actor().map(|actor| actor.user_id);
    let mut action = None;
    let mut new_user = false;
    let mut save = false;
    let mut cancel = false;
    let form_open = app.settings_state().user_form_open;
    let editing_self = app
        .settings_state()
        .user_form
        .editing_id
        .is_some_and(|id| Some(id) == current_id);

    if form_open {
        widgets::card(
            ui,
            if app.settings_state().user_form.editing_id.is_some() {
                "Kullanıcıyı düzenle"
            } else {
                "Yeni kullanıcı"
            },
            |ui| {
                let state = app.settings_state_mut();
                let form = &mut state.user_form;
                ui.horizontal(|ui| {
                    ui.label("Kullanıcı adı:");
                    ui.add_enabled(
                        form.editing_id.is_none(),
                        TextEdit::singleline(&mut form.username).desired_width(220.0),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Görünen ad:");
                    ui.add(TextEdit::singleline(&mut form.display_name).desired_width(220.0));
                });
                ui.horizontal(|ui| {
                    ui.label("Rol:");
                    ui.add_enabled_ui(!editing_self, |ui| {
                        ComboBox::from_id_salt("user_role")
                            .selected_text(form.role.label())
                            .show_ui(ui, |ui| {
                                for role in Role::ALL {
                                    ui.selectable_value(&mut form.role, role, role.label());
                                }
                            });
                    });
                });
                ui.add_enabled(
                    !editing_self,
                    egui::Checkbox::new(&mut form.active, "Aktif"),
                );
                if form.editing_id.is_none() {
                    ui.horizontal(|ui| {
                        ui.label("Şifre:");
                        ui.add(
                            TextEdit::singleline(&mut form.password)
                                .password(true)
                                .desired_width(220.0),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Şifre tekrarı:");
                        ui.add(
                            TextEdit::singleline(&mut form.password_confirm)
                                .password(true)
                                .desired_width(220.0),
                        );
                    });
                }
                if let Some(error) = state.user_form_error.as_deref() {
                    ui.label(RichText::new(error).color(Palette::DANGER).size(12.5));
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    save = ui.button("Kaydet").clicked();
                    cancel = ui.button("Vazgeç").clicked();
                });
            },
        );
        ui.add_space(8.0);
    }

    let mut reset_submit = false;
    let mut reset_cancel = false;
    if app.settings_state().reset_user_id.is_some() {
        widgets::card(ui, "Şifre sıfırla", |ui| {
            let state = app.settings_state_mut();
            ui.label(
                RichText::new("Yeni şifre kullanıcıya bir sonraki girişte bildirilecek.")
                    .size(12.0)
                    .color(Palette::MUTED),
            );
            ui.horizontal(|ui| {
                ui.label("Yeni şifre:");
                ui.add(
                    TextEdit::singleline(&mut state.reset_password)
                        .password(true)
                        .desired_width(220.0),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Tekrar:");
                ui.add(
                    TextEdit::singleline(&mut state.reset_password_confirm)
                        .password(true)
                        .desired_width(220.0),
                );
            });
            ui.horizontal(|ui| {
                reset_submit = ui.button("Şifreyi sıfırla").clicked();
                reset_cancel = ui.button("Vazgeç").clicked();
            });
            if let Some(error) = state.users_error.as_deref() {
                ui.label(RichText::new(error).color(Palette::DANGER).size(12.5));
            }
        });
        ui.add_space(8.0);
    }

    if let Some(id) = app.settings_state().pending_delete_user {
        widgets::warning_card(
            ui,
            "Kullanıcıyı sil",
            "Bu işlem kullanıcı hesabını kalıcı olarak siler. Emin misiniz?",
        );
        ui.horizontal(|ui| {
            if ui.button("Sil").clicked() {
                let _ = app.delete_user(id);
                app.settings_state_mut().pending_delete_user = None;
            }
            if ui.button("Vazgeç").clicked() {
                app.settings_state_mut().pending_delete_user = None;
            }
        });
        ui.add_space(8.0);
    }

    ui.horizontal(|ui| {
        new_user = ui.button("Yeni kullanıcı").clicked();
    });
    if let Some(notice) = app.settings_state().users_notice.clone() {
        ui.label(RichText::new(notice).color(Palette::SUCCESS).size(12.5));
    }
    if let Some(error) = app.settings_state().users_error.clone() {
        ui.label(RichText::new(error).color(Palette::DANGER).size(12.5));
    }
    ui.add_space(6.0);

    let users = app.settings_state().users.clone();
    if users.is_empty() {
        ui.label(RichText::new("Kullanıcı bulunamadı.").color(Palette::MUTED));
    } else {
        // 900x600 pencerede tüm işlem düğmeleri tek satıra sığmaz. İşlemler
        // açılır menüde toplanır; sütunlar da dar ekrana göre ölçülür.
        ui.horizontal(|ui| {
            let header = |ui: &mut Ui, width: f32, text: &str| {
                ui.add_sized(
                    [width, 18.0],
                    egui::Label::new(
                        RichText::new(text)
                            .size(11.5)
                            .strong()
                            .color(Palette::MUTED),
                    )
                    .truncate(),
                );
            };
            header(ui, 115.0, "Kullanıcı adı");
            header(ui, 105.0, "Görünen ad");
            header(ui, 60.0, "Rol");
            header(ui, 50.0, "Durum");
            header(ui, 110.0, "Son giriş");
            header(ui, 70.0, "İşlemler");
        });
        ui.separator();
        let list_height = ui.available_height().clamp(120.0, 360.0);
        ScrollArea::vertical()
            .id_salt("kullanici_listesi")
            .max_height(list_height)
            .show_rows(ui, USER_ROW_HEIGHT, users.len(), |ui, range| {
                for index in range {
                    let user = &users[index];
                    ui.horizontal(|ui| {
                        ui.add_sized(
                            [115.0, 24.0],
                            egui::Label::new(RichText::new(&user.username).strong()).truncate(),
                        );
                        ui.add_sized([105.0, 24.0], egui::Label::new(user.label()).truncate());
                        ui.add_sized([60.0, 24.0], egui::Label::new(user.role.label()).truncate());
                        ui.add_sized(
                            [50.0, 24.0],
                            egui::Label::new(if user.active { "Aktif" } else { "Pasif" })
                                .truncate(),
                        );
                        let last_login: String = user
                            .last_login_at
                            .as_deref()
                            .unwrap_or("Hiç giriş yapmadı")
                            .chars()
                            .take(16)
                            .collect();
                        ui.add_sized([110.0, 24.0], egui::Label::new(last_login).truncate());
                        ui.menu_button("İşlemler", |ui| {
                            if ui.button("Düzenle").clicked() {
                                action = Some(UserAction::Edit(user.id));
                                ui.close();
                            }
                            if Some(user.id) != current_id {
                                let toggle = if user.active {
                                    "Pasife al"
                                } else {
                                    "Aktifleştir"
                                };
                                if ui.button(toggle).clicked() {
                                    action = Some(UserAction::Toggle(user.id));
                                    ui.close();
                                }
                                if ui.button("Şifre sıfırla").clicked() {
                                    action = Some(UserAction::Reset(user.id));
                                    ui.close();
                                }
                                if ui.button("Sil").clicked() {
                                    action = Some(UserAction::Delete(user.id));
                                    ui.close();
                                }
                            }
                        });
                    });
                    ui.separator();
                }
            });
    }

    if new_user {
        app.open_new_user_form();
    }
    if save {
        let _ = app.submit_user_form();
    }
    if cancel {
        app.cancel_user_form();
    }
    if reset_submit {
        let _ = app.submit_reset_user_form();
    }
    if reset_cancel {
        let state = app.settings_state_mut();
        state.reset_user_id = None;
        state.reset_password.clear();
        state.reset_password_confirm.clear();
    }
    match action {
        Some(UserAction::Edit(id)) => app.open_edit_user_form(id),
        Some(UserAction::Toggle(id)) => {
            let _ = app.toggle_user_active(id);
        }
        Some(UserAction::Reset(id)) => app.open_reset_user_form(id),
        Some(UserAction::Delete(id)) => {
            app.settings_state_mut().pending_delete_user = Some(id);
        }
        None => {}
    }
}
