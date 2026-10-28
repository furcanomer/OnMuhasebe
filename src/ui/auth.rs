// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! İlk yönetici, giriş ve kilit ekranları.
//!
//! Bu ekranlar ana yerleşimin (menü/gezinme) dışında tam pencereyi kullanır.
//! 900×600 en küçük pencere boyutunda taşmamak için formlar kaydırılabilir.

use egui::{CentralPanel, Frame, Margin, RichText, ScrollArea, TextEdit, Ui};

use crate::APP_NAME;
use crate::app::{AuthScreen, OnMuhasebeApp};
use crate::ui::theme::Palette;

/// Kimlik doğrulama ekranını çizer.
pub fn show(ui: &mut Ui, app: &mut OnMuhasebeApp) {
    CentralPanel::default()
        .frame(
            Frame::new()
                .fill(Palette::BACKGROUND)
                .inner_margin(Margin::same(24)),
        )
        .show(ui, |ui| {
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.vertical_centered(|ui| {
                        ui.add_space(18.0);
                        ui.label(
                            RichText::new(APP_NAME)
                                .size(28.0)
                                .strong()
                                .color(Palette::TEXT),
                        );
                        ui.label(
                            RichText::new("Yerel ön muhasebe uygulaması")
                                .size(13.0)
                                .color(Palette::MUTED),
                        );
                        ui.add_space(16.0);
                        form_card(ui, |ui| {
                            ui.set_max_width(420.0);
                            match app.auth_screen() {
                                AuthScreen::FirstAdmin => first_admin(ui, app),
                                AuthScreen::Login => login(ui, app),
                                AuthScreen::Locked => locked(ui, app),
                                AuthScreen::Authenticated => {}
                            }
                        });
                        ui.add_space(18.0);
                    });
                });
        });
}

fn form_card(ui: &mut Ui, contents: impl FnOnce(&mut Ui)) {
    Frame::new()
        .fill(Palette::SURFACE)
        .stroke(egui::Stroke::new(1.0, Palette::BORDER))
        .inner_margin(Margin::symmetric(24, 20))
        .show(ui, |ui| {
            ui.set_width(420.0_f32.min(ui.available_width()));
            contents(ui);
        });
}

fn status_messages(ui: &mut Ui, app: &OnMuhasebeApp) {
    if app.startup_backup_pending() {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(
                RichText::new("Otomatik yedek alınıyor...")
                    .color(Palette::MUTED)
                    .size(12.5),
            );
        });
    }
    if let Some(warning) = app.startup_backup_warning() {
        widgets_notice(ui, "Uyarı", warning, Palette::WARNING);
    }
    if let Some(error) = app.startup_error() {
        widgets_notice(ui, "Veritabanı açılamadı", error, Palette::DANGER);
    }
}

fn widgets_notice(ui: &mut Ui, title: &str, message: &str, color: egui::Color32) {
    Frame::new()
        .fill(color.gamma_multiply(0.08))
        .stroke(egui::Stroke::new(1.0, color))
        .inner_margin(Margin::same(10))
        .show(ui, |ui| {
            ui.label(RichText::new(title).strong().color(color).size(13.0));
            ui.label(RichText::new(message).size(12.5).color(Palette::TEXT));
        });
}

fn form_error(ui: &mut Ui, message: Option<&str>) {
    if let Some(message) = message {
        ui.add_space(6.0);
        ui.label(RichText::new(message).color(Palette::DANGER).size(12.5));
    }
}

fn form_notice(ui: &mut Ui, message: Option<&str>) {
    if let Some(message) = message {
        ui.add_space(6.0);
        ui.label(RichText::new(message).color(Palette::SUCCESS).size(12.5));
    }
}

fn first_admin(ui: &mut Ui, app: &mut OnMuhasebeApp) {
    ui.label(RichText::new("İlk yönetici oluştur").size(18.0).strong());
    ui.label(
        RichText::new(
            "Programı ilk kez kullanıyorsunuz. Devam etmek için bir yönetici hesabı oluşturun.",
        )
        .size(12.5)
        .color(Palette::MUTED),
    );
    ui.add_space(10.0);
    status_messages(ui, app);
    if app.startup_error().is_some() || app.database().is_none() {
        return;
    }

    let state = app.auth_state_mut();
    field(ui, "Kullanıcı adı", &mut state.username, false);
    field(ui, "Görünen ad (opsiyonel)", &mut state.display_name, false);
    field(ui, "Şifre", &mut state.password, true);
    field(ui, "Şifre tekrarı", &mut state.password_confirm, true);
    form_error(ui, state.error.as_deref());
    ui.label(
        RichText::new("Şifre en az 8 karakter, en az bir harf ve bir rakam içermelidir.")
            .size(11.5)
            .color(Palette::MUTED),
    );
    ui.add_space(10.0);
    if ui.button("Yöneticiyi oluştur").clicked() {
        let _ = app.create_initial_admin();
    }
}

fn login(ui: &mut Ui, app: &mut OnMuhasebeApp) {
    ui.label(RichText::new("Giriş yap").size(18.0).strong());
    ui.label(
        RichText::new("Kullanıcı hesabınızla oturum açın.")
            .size(12.5)
            .color(Palette::MUTED),
    );
    ui.add_space(10.0);
    status_messages(ui, app);
    if let Some(notice) = app.auth_state().notice.as_deref() {
        form_notice(ui, Some(notice));
    }

    if app.startup_error().is_some() || app.database().is_none() {
        return;
    }

    let (enter_pressed, clicked) = {
        let state = app.auth_state_mut();
        field(ui, "Kullanıcı adı", &mut state.username, false);
        field(ui, "Şifre", &mut state.password, true);
        form_error(ui, state.error.as_deref());
        ui.add_space(10.0);
        let clicked = ui.button("Giriş yap").clicked();
        let enter = ui.ctx().input(|input| input.key_pressed(egui::Key::Enter));
        (enter, clicked)
    };
    if enter_pressed || clicked {
        let _ = app.attempt_login();
    }
    ui.add_space(8.0);
    ui.label(
        RichText::new("Şifrenizi unuttuysanız kurtarma adımları README dosyasındadır.")
            .size(11.5)
            .color(Palette::MUTED),
    );
}

fn locked(ui: &mut Ui, app: &mut OnMuhasebeApp) {
    ui.label(RichText::new("Oturum kilitlendi").size(18.0).strong());
    let name = app
        .current_actor()
        .map(|actor| actor.label().to_owned())
        .unwrap_or_default();
    ui.label(
        RichText::new(format!(
            "{name} için hareketsizlik nedeniyle oturum kilitlendi. Devam etmek için şifrenizi girin."
        ))
        .size(12.5)
        .color(Palette::MUTED),
    );
    ui.add_space(10.0);
    let state = app.auth_state_mut();
    field(ui, "Şifre", &mut state.password, true);
    form_error(ui, state.error.as_deref());
    ui.add_space(10.0);
    if ui.button("Kilidi aç").clicked() {
        let _ = app.unlock();
    }
    if ui.button("Oturumu kapat").clicked() {
        app.logout();
    }
}

fn field(ui: &mut Ui, label: &str, value: &mut String, password: bool) {
    ui.add_space(6.0);
    ui.label(
        RichText::new(label)
            .size(12.0)
            .strong()
            .color(Palette::TEXT),
    );
    let edit = if password {
        TextEdit::singleline(value).password(true)
    } else {
        TextEdit::singleline(value)
    };
    ui.add_sized([ui.available_width(), 28.0], edit);
}
