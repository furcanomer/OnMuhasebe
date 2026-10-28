// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Arayüz genelinde paylaşılan küçük bileşenler.
//!
//! Tekrar eden kart, bilgi kutusu ve madde işareti çizimleri tek yerde
//! toplanmıştır; ekranlar yalnızca bu bileşenleri çağırır. Böylece görünüm
//! (boşluk, renk, köşe yuvarlaklığı) tüm ekranlarda aynı kalır.

use egui::{
    Color32, CornerRadius, CursorIcon, Frame, Label, Margin, Response, RichText, Sense, Stroke,
    TextEdit, Ui,
};

use crate::ui::theme::{Palette, metrics};

/// Tıklanabilir, satır kaydırabilen bağlantı metni.
///
/// `egui::Link` uzun metinlerde genişliği zorlar; bu yardımcı, `Label::wrap`
/// kullanarak dar pencerelerde taşmayı önler.
pub fn link(ui: &mut Ui, text: RichText) -> Response {
    ui.add(
        Label::new(text.color(Palette::ACCENT).underline())
            .wrap()
            .sense(Sense::click()),
    )
    .on_hover_cursor(CursorIcon::PointingHand)
}

/// Başlıklı yüzey kartı.
pub fn card(ui: &mut Ui, title: &str, contents: impl FnOnce(&mut Ui)) {
    frame(ui, Palette::SURFACE, Palette::BORDER, |ui| {
        heading(ui, title);
        ui.add_space(8.0);
        contents(ui);
    });
}

/// Kart başlığı.
pub fn heading(ui: &mut Ui, title: &str) {
    ui.label(
        RichText::new(title)
            .size(15.0)
            .strong()
            .color(Palette::TEXT),
    );
}

/// Kart içi bölüm alt başlığı.
pub fn subheading(ui: &mut Ui, title: &str) {
    ui.label(
        RichText::new(title)
            .size(13.0)
            .strong()
            .color(Palette::TEXT),
    );
}

/// Hata bilgisini gösteren kart.
pub fn error_card(ui: &mut Ui, title: &str, message: &str) {
    frame(
        ui,
        Palette::DANGER.gamma_multiply(0.07),
        Palette::DANGER,
        |ui| {
            ui.label(
                RichText::new(title)
                    .size(15.0)
                    .strong()
                    .color(Palette::DANGER),
            );
            ui.add_space(4.0);
            ui.add(Label::new(message).wrap());
        },
    );
}

/// Sarı uyarı kartı (yedek boyutu gibi dikkat gerektiren durumlar).
pub fn warning_card(ui: &mut Ui, title: &str, message: &str) {
    frame(
        ui,
        Palette::WARNING.gamma_multiply(0.09),
        Palette::WARNING,
        |ui| {
            ui.label(
                RichText::new(title)
                    .size(14.0)
                    .strong()
                    .color(Palette::WARNING),
            );
            ui.add_space(4.0);
            ui.add(Label::new(RichText::new(message).color(Palette::TEXT)).wrap());
        },
    );
}

/// Dikkat çeken ama hata olmayan bilgilendirme kartı.
pub fn notice_card(ui: &mut Ui, title: &str, message: &str) {
    frame(
        ui,
        Palette::ACCENT.gamma_multiply(0.06),
        Palette::BORDER,
        |ui| {
            ui.label(
                RichText::new(title)
                    .size(14.0)
                    .strong()
                    .color(Palette::ACCENT),
            );
            ui.add_space(4.0);
            ui.add(Label::new(RichText::new(message).color(Palette::TEXT)).wrap());
        },
    );
}

/// Kapatılabilir başarı bilgilendirmesi; "Kapat" düğmesine basılırsa `true` döner.
pub fn success_card(ui: &mut Ui, title: &str, message: &str) -> bool {
    let mut dismissed = false;

    frame(
        ui,
        Palette::SUCCESS.gamma_multiply(0.09),
        Palette::SUCCESS,
        |ui| {
            ui.label(
                RichText::new(title)
                    .size(14.0)
                    .strong()
                    .color(Palette::SUCCESS),
            );
            ui.add_space(4.0);
            ui.add(Label::new(RichText::new(message).color(Palette::TEXT)).wrap());
            ui.add_space(6.0);
            dismissed = ui.small_button("Kapat").clicked();
        },
    );

    dismissed
}

/// Başlıksız çerçeveli yüzey (liste satırları gibi tekrar eden bloklar için).
pub fn surface(ui: &mut Ui, contents: impl FnOnce(&mut Ui)) {
    frame(ui, Palette::SURFACE, Palette::BORDER, contents);
}

/// Renklendirilmiş yüzey (düşük stok satırları gibi vurgulu bloklar için).
pub fn surface_tinted(ui: &mut Ui, fill: Color32, border: Color32, contents: impl FnOnce(&mut Ui)) {
    frame(ui, fill, border, contents);
}

/// Solunda madde işareti bulunan metin satırı.
pub fn bullet(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("•").color(Palette::MUTED));
        ui.add(Label::new(text).wrap());
    });
}

/// Durum tablolarında sol kolon etiketi.
pub fn row_label(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).color(Palette::MUTED));
}

/// Uzun metinlerin (örneğin dosya yollarının) satırı taşırmasını engelleyen
/// monospace etiket.
pub fn path_label(ui: &mut Ui, text: &str) {
    ui.add(Label::new(RichText::new(text).monospace().size(12.5)).wrap());
}

/// Aranabilir açılır seçim kutusu.
///
/// Seçenekler `(kimlik, etiket)` çiftleridir. Kullanıcı yazdıkça liste
/// süzülür (Türkçe karakterlerden etkilenmez). Seçim yapılırsa ilgili kimlik
/// döner; pencere kapalıysa `None` döner.
///
/// Arama metni, aynı `id_salt` için ekranlar arası korunur (`ui.data`).
pub fn searchable_select(
    ui: &mut Ui,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    selected_text: &str,
    options: &[(i64, String)],
    width: f32,
) -> Option<i64> {
    let filter_id = egui::Id::unique(&id_salt).with("arama_filtresi");
    let mut filter = ui
        .data(|data| data.get_temp::<String>(filter_id))
        .unwrap_or_default();

    let mut selected = None;

    // Uzun etiketlerin kutuyu genişletip satırı taşırmasını önlemek için seçili
    // metin, verilen genişliğe sığacak biçimde kısaltılır (yaklaşık 6,5 piksel
    // / karakter; kutu iç boşluğu ve ok simgesi için pay bırakılır).
    let max_chars = (((width - 26.0) / 6.5).floor() as usize).max(4);
    let shown_text = truncate_chars(selected_text, max_chars);

    egui::ComboBox::from_id_salt(id_salt)
        .selected_text(shown_text)
        .width(width)
        .show_ui(ui, |ui| {
            ui.add(
                TextEdit::singleline(&mut filter)
                    .desired_width(width)
                    .hint_text("Ara..."),
            );
            ui.separator();

            let needle = crate::customer::normalize_for_search(&filter);
            let mut shown = 0_usize;

            egui::ScrollArea::vertical()
                .max_height(220.0)
                .show(ui, |ui| {
                    for (id, label) in options {
                        if !needle.is_empty()
                            && !crate::customer::normalize_for_search(label).contains(&needle)
                        {
                            continue;
                        }
                        shown += 1;
                        if ui.selectable_label(false, label).clicked() {
                            selected = Some(*id);
                        }
                    }
                });

            if shown == 0 {
                ui.label(
                    RichText::new("Eşleşen kayıt yok.")
                        .size(12.5)
                        .color(Palette::MUTED),
                );
            }
        });

    if selected.is_some() {
        filter.clear();
    }
    ui.data_mut(|data| data.insert_temp(filter_id, filter));

    selected
}

/// Metni en fazla `max_chars` karaktere indirir; kırpılırsa sonuna `…` ekler.
///
/// Türkçe karakterler çok baytlı olduğu için karakter sınırları korunur.
fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }

    let keep = max_chars.saturating_sub(1);
    let mut result: String = text.chars().take(keep).collect();
    result.push('…');
    result
}

/// Ortak kart çerçevesi.
fn frame(ui: &mut Ui, fill: Color32, border: Color32, contents: impl FnOnce(&mut Ui)) {
    Frame::group(ui.style())
        .fill(fill)
        .stroke(Stroke::new(1.0, border))
        .corner_radius(CornerRadius::same(metrics::CORNER_RADIUS))
        .inner_margin(Margin::symmetric(
            metrics::CARD_PADDING_X,
            metrics::CARD_PADDING_Y,
        ))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            contents(ui);
        });
}
