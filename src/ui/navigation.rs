// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Sol gezinme menüsü.
//!
//! Menü, [`View::ALL`] listesinden üretilir; böylece yeni bir bölüm
//! eklendiğinde burada değişiklik yapılması gerekmez. Seçili bölüm vurgulanır
//! ve tıklama doğrudan uygulama durumuna yazılır.

use egui::{Button, Frame, Margin, Panel, RichText, ScrollArea, Stroke, Ui, vec2};

use crate::app::{OnMuhasebeApp, View};
use crate::ui::theme::{Palette, metrics};

/// Gezinme panelini çizer ve panelin yanıtını döndürür.
///
/// Tıklanan bölüm doğrudan uygulama durumuna yazılır; dönen yanıt yalnızca
/// yerleşimi doğrulamak (panel ölçüsü) için kullanılır.
pub fn show(ui: &mut Ui, app: &mut OnMuhasebeApp) -> egui::InnerResponse<()> {
    Panel::left("gezinme")
        .resizable(false)
        .exact_size(metrics::NAV_WIDTH)
        .frame(
            Frame::new()
                .fill(Palette::CHROME)
                .inner_margin(Margin::symmetric(
                    metrics::PANEL_PADDING_X,
                    metrics::PANEL_PADDING_Y + 4,
                ))
                .stroke(Stroke::new(1.0, Palette::BORDER)),
        )
        .show(ui, |ui| {
            // Pencere kısaldığında bölümler taşmasın, gerektiğinde kaydırılsın.
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("BÖLÜMLER").small().color(Palette::MUTED));
                    ui.add_space(8.0);

                    let active = app.view();
                    let mut selected = None;

                    for view in View::ALL {
                        if item(ui, view, view == active) {
                            selected = Some(view);
                        }
                        ui.add_space(2.0);
                    }

                    if let Some(view) = selected {
                        app.set_view(view);
                    }
                });
        })
}

/// Tek bir gezinme öğesi çizer; tıklandıysa `true` döner.
fn item(ui: &mut Ui, view: View, active: bool) -> bool {
    let button = Button::new(RichText::new(view.title()).size(14.0))
        .selected(active)
        .min_size(vec2(ui.available_width(), metrics::NAV_ITEM_HEIGHT));

    let response = ui.add(button);

    if view.is_ready() {
        response.clicked()
    } else {
        response
            .on_hover_text("Bu bölüm henüz geliştirilmedi.")
            .clicked()
    }
}
