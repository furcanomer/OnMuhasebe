// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Dosya aktarımı pencereleri ve yerel aç/kaydet diyalogları.
//!
//! Bu modül yalnızca arayüzü çizer ve kullanıcı niyetini toplar. CSV yazma,
//! içe aktarma ve yedekleme işleri [`crate::app`] katmanındadır. Yerel dosya
//! diyalogları `rfd` ile açılır; testler bu fonksiyonları çağırmaz, doğrudan
//! yol tabanlı uygulama metotlarını kullanır.

use std::path::PathBuf;

use egui::{Align2, Grid, Label, RichText, ScrollArea, Window};

use crate::app::{
    ImportPreview, ImportResultView, NoticeKind, OnMuhasebeApp, PendingRestore, TransferNotice,
};
use crate::ui::theme::Palette;

/// Arayüzde kullanıcının başlattığı aktarım eylemi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    /// İçe aktarma önizlemesini kapatır.
    CancelPreview,
    /// Önizlemedeki hazır satırları aktarır.
    ConfirmImport,
    /// İçe aktarma sonucunu kapatır.
    DismissResult,
    /// Bilgilendirmeyi kapatır.
    DismissNotice,
    /// Bilgilendirmedeki dosyanın klasörünü açar.
    OpenFolder,
    /// Başarılı geri yükleme sonrası uygulamayı kapatır.
    CloseApp,
    /// Geri yükleme onayını iptal eder.
    CancelRestore,
    /// Geri yüklemeyi başlatır.
    ConfirmRestore,
}

/// CSV kaydetme diyalogu.
pub fn save_csv_dialog(default_name: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("CSV dosyası", &["csv"])
        .set_file_name(default_name)
        .save_file()
}

/// CSV açma diyalogu.
pub fn open_csv_dialog() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("CSV dosyası", &["csv"])
        .pick_file()
}

/// Yedek kaydetme diyalogu.
pub fn save_backup_dialog(default_name: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("OnMuhasebe yedeği", &["onm-backup"])
        .set_file_name(default_name)
        .save_file()
}

/// Yedek açma diyalogu.
pub fn open_backup_dialog() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("OnMuhasebe yedeği", &["onm-backup"])
        .pick_file()
}

/// Tüm aktarım pencerelerini çizer.
pub fn show(ctx: &egui::Context, app: &mut OnMuhasebeApp) {
    let mut action = None;

    if let Some(preview) = app.transfer_state().import_preview.as_ref() {
        import_preview_window(
            ctx,
            preview,
            app.transfer_state().import_error.as_deref(),
            &mut action,
        );
    }
    if let Some(result) = app.transfer_state().import_result.as_ref() {
        import_result_window(ctx, result, &mut action);
    }
    if let Some(pending) = app.transfer_state().pending_restore.as_ref() {
        restore_window(ctx, pending, &mut action);
    }
    if let Some(notice) = app.transfer_state().notice.as_ref() {
        notice_window(ctx, notice, &mut action);
    }
    if let Some(message) = app.transfer_state().busy_message.as_deref() {
        busy_window(ctx, message);
    }

    match action {
        Some(Action::CancelPreview) => app.cancel_import_preview(),
        Some(Action::ConfirmImport) => {
            let _ = app.confirm_import();
        }
        Some(Action::DismissResult) => app.dismiss_import_result(),
        Some(Action::DismissNotice) => app.dismiss_transfer_notice(),
        Some(Action::OpenFolder) => app.open_transfer_folder(),
        Some(Action::CloseApp) => {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        Some(Action::CancelRestore) => app.cancel_restore(),
        Some(Action::ConfirmRestore) => {
            let _ = app.confirm_restore();
        }
        None => {}
    }
}

fn import_preview_window(
    ctx: &egui::Context,
    preview: &ImportPreview,
    error: Option<&str>,
    action: &mut Option<Action>,
) {
    let title = format!("{} — Önizleme", preview.kind.title());
    Window::new(title)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(false)
        .default_width(780.0)
        .max_width(860.0)
        .show(ctx, |ui| {
            ui.set_max_width(820.0);
            ui.add(
                Label::new(
                    RichText::new(format!(
                        "{} · {} hazır, {} hatalı",
                        preview.file_name,
                        preview.ready_count(),
                        preview.error_count()
                    ))
                    .size(12.5)
                    .color(Palette::MUTED),
                )
                .wrap(),
            );
            ui.add_space(6.0);

            ScrollArea::vertical()
                .max_height(320.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    Grid::new("csv_ice_aktarma_onizleme")
                        .num_columns(3)
                        .striped(true)
                        .min_col_width(52.0)
                        .spacing([12.0, 5.0])
                        .show(ui, |ui| {
                            ui.label(header_text("Satır"));
                            ui.label(header_text("Kod"));
                            ui.label(header_text("Durum"));
                            ui.end_row();

                            for row in &preview.rows {
                                ui.label(
                                    RichText::new(row.line.to_string())
                                        .monospace()
                                        .size(12.0)
                                        .color(Palette::MUTED),
                                );
                                ui.add(Label::new(RichText::new(&row.code).size(12.5)).truncate());
                                match &row.status {
                                    crate::app::ImportRowStatus::Ready => {
                                        ui.label(
                                            RichText::new("Hazır")
                                                .size(12.5)
                                                .strong()
                                                .color(Palette::SUCCESS),
                                        );
                                    }
                                    crate::app::ImportRowStatus::Error(reason) => {
                                        ui.add(
                                            Label::new(
                                                RichText::new(format!("Hata: {reason}"))
                                                    .size(12.5)
                                                    .color(Palette::DANGER),
                                            )
                                            .truncate(),
                                        );
                                    }
                                }
                                ui.end_row();
                            }
                        });
                });

            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    "Yalnızca hazır satırlar aktarılacak. Hatalı satırlar atlanır; \
                     aktarım tek işlemde yapılır.",
                )
                .size(12.5)
                .color(Palette::MUTED),
            );

            if let Some(error) = error {
                ui.add_space(4.0);
                ui.add(
                    Label::new(
                        RichText::new(error)
                            .size(12.5)
                            .strong()
                            .color(Palette::DANGER),
                    )
                    .wrap(),
                );
            }

            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                if ui.button("Vazgeç").clicked() {
                    *action = Some(Action::CancelPreview);
                }
                let label = format!("{} hazır satırı aktar", preview.ready_count());
                if ui
                    .add_enabled(preview.ready_count() > 0, egui::Button::new(label))
                    .clicked()
                {
                    *action = Some(Action::ConfirmImport);
                }
            });
        });
}

fn import_result_window(
    ctx: &egui::Context,
    result: &ImportResultView,
    action: &mut Option<Action>,
) {
    Window::new("İçe aktarma sonucu")
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(false)
        .default_width(640.0)
        .max_width(820.0)
        .show(ctx, |ui| {
            ui.set_max_width(780.0);
            ui.label(
                RichText::new(format!(
                    "{} kayıt eklendi, {} satır atlandı.",
                    result.inserted,
                    result.skipped.len()
                ))
                .size(15.0)
                .strong()
                .color(Palette::TEXT),
            );
            ui.add_space(6.0);

            if result.skipped.is_empty() {
                ui.label(
                    RichText::new("Atlanan satır yok.")
                        .size(12.5)
                        .color(Palette::MUTED),
                );
            } else {
                ScrollArea::vertical()
                    .max_height(300.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        Grid::new("csv_ice_aktarma_atlanan")
                            .num_columns(2)
                            .striped(true)
                            .min_col_width(140.0)
                            .spacing([14.0, 5.0])
                            .show(ui, |ui| {
                                ui.label(header_text("Kod"));
                                ui.label(header_text("Atlama sebebi"));
                                ui.end_row();
                                for skipped in &result.skipped {
                                    ui.add(
                                        Label::new(
                                            RichText::new(&skipped.code).size(12.5).monospace(),
                                        )
                                        .truncate(),
                                    );
                                    ui.add(
                                        Label::new(
                                            RichText::new(&skipped.reason)
                                                .size(12.5)
                                                .color(Palette::DANGER),
                                        )
                                        .truncate(),
                                    );
                                    ui.end_row();
                                }
                            });
                    });
            }

            ui.add_space(8.0);
            if ui.button("Tamam").clicked() {
                *action = Some(Action::DismissResult);
            }
        });
}

fn restore_window(ctx: &egui::Context, pending: &PendingRestore, action: &mut Option<Action>) {
    Window::new("Yedeği geri yükle")
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(false)
        .default_width(610.0)
        .max_width(820.0)
        .show(ctx, |ui| {
            ui.set_max_width(760.0);
            ui.add(
                Label::new(
                    RichText::new(pending.path.display().to_string())
                        .monospace()
                        .size(12.0)
                        .color(Palette::MUTED),
                )
                .wrap(),
            );
            ui.add_space(6.0);
            ui.label(
                RichText::new(format!(
                    "Şema sürümü: {} · Yedek tarihi: {}",
                    pending.info.schema_version,
                    pending.info.created_at.as_deref().unwrap_or("bilinmiyor")
                ))
                .size(12.5)
                .color(Palette::MUTED),
            );
            ui.add_space(8.0);
            ui.add(
                Label::new(
                    RichText::new(
                        "Bu işlem mevcut tüm verilerinizi değiştirecek. Mevcut verileriniz \
                         otomatik olarak yedeklenecek. Devam edilsin mi?",
                    )
                    .size(13.0)
                    .color(Palette::TEXT),
                )
                .wrap(),
            );
            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                if ui.button("Vazgeç").clicked() {
                    *action = Some(Action::CancelRestore);
                }
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new("Yedeği geri yükle").color(Palette::SURFACE),
                        )
                        .fill(Palette::DANGER),
                    )
                    .clicked()
                {
                    *action = Some(Action::ConfirmRestore);
                }
            });
        });
}

fn notice_window(ctx: &egui::Context, notice: &TransferNotice, action: &mut Option<Action>) {
    let color = match notice.kind {
        NoticeKind::Success => Palette::SUCCESS,
        NoticeKind::Error => Palette::DANGER,
    };
    Window::new(&notice.title)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(false)
        .default_width(590.0)
        .max_width(820.0)
        .show(ctx, |ui| {
            ui.set_max_width(740.0);
            ui.label(RichText::new(&notice.message).size(13.0).color(color));
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                if notice.restore_success {
                    if ui.add(egui::Button::new("Uygulamayı kapat")).clicked() {
                        *action = Some(Action::CloseApp);
                    }
                } else {
                    if notice.can_open_folder() && ui.button("Klasörü aç").clicked() {
                        *action = Some(Action::OpenFolder);
                    }
                    if ui.button("Tamam").clicked() {
                        *action = Some(Action::DismissNotice);
                    }
                }
            });
        });
}

fn busy_window(ctx: &egui::Context, message: &str) {
    Window::new("Lütfen bekleyin")
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new(message).size(13.0).color(Palette::TEXT));
            });
        });
}

fn header_text(text: &str) -> RichText {
    RichText::new(text)
        .size(12.0)
        .strong()
        .color(Palette::MUTED)
}
