// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Uygulama teması: renk paleti ve egui stil ayarları.
//!
//! Tema tek bir yerden uygulanır ([`apply`]), böylece tüm ekranlar aynı
//! kurumsal görünümü paylaşır ve yeni modüller eklendiğinde ek ayar gerekmez.

use egui::{Color32, Context, CornerRadius, Stroke, Theme, ThemePreference, Visuals};

/// Uygulamanın renk paleti.
///
/// Arayüz kodunda sabit `Color32::from_rgb(...)` çağrıları yerine bu sabitler
/// kullanılır.
pub struct Palette;

impl Palette {
    /// Ana pencere arka planı.
    pub const BACKGROUND: Color32 = Color32::from_rgb(0xF4, 0xF6, 0xF8);
    /// Kart ve panel yüzeyleri.
    pub const SURFACE: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
    /// Gezinme çubuğu ve durum çubuğu.
    pub const CHROME: Color32 = Color32::from_rgb(0xEB, 0xEE, 0xF2);
    /// Ayırıcı çizgiler ve kenarlıklar.
    pub const BORDER: Color32 = Color32::from_rgb(0xD3, 0xD9, 0xE0);
    /// Vurgu rengi (seçili öğeler, bağlantılar).
    pub const ACCENT: Color32 = Color32::from_rgb(0x1B, 0x5E, 0xA8);
    /// Bağlantıyla gelinen kaydın yumuşak vurgu arka planı.
    pub const ACCENT_SOFT: Color32 = Color32::from_rgb(0xE8, 0xF1, 0xFB);
    /// Ana metin rengi.
    pub const TEXT: Color32 = Color32::from_rgb(0x1E, 0x24, 0x2B);
    /// İkincil / açıklama metni rengi.
    pub const MUTED: Color32 = Color32::from_rgb(0x5F, 0x6A, 0x76);
    /// Olumlu durum (program çalışıyor).
    pub const SUCCESS: Color32 = Color32::from_rgb(0x1B, 0x7A, 0x3B);
    /// Uyarı durumu.
    pub const WARNING: Color32 = Color32::from_rgb(0xA9, 0x6A, 0x00);
    /// Hata durumu.
    pub const DANGER: Color32 = Color32::from_rgb(0xB0, 0x2A, 0x2A);
}

/// Arayüz genelinde kullanılan ortak ölçüler (piksel).
///
/// Sabit değerlerin tek yerde toplanması, ekranlar arasında tutarlı boşluk ve
/// hizalama sağlar.
pub mod metrics {
    /// Kart ve çerçeve köşe yuvarlaklığı.
    pub const CORNER_RADIUS: u8 = 8;
    /// Kart iç dolgusu.
    pub const CARD_PADDING_X: i8 = 14;
    pub const CARD_PADDING_Y: i8 = 12;
    /// Menü, üst bilgi ve durum çubuğu kenar boşlukları.
    pub const PANEL_PADDING_X: i8 = 10;
    pub const PANEL_PADDING_Y: i8 = 5;
    /// İçerik alanının kenar boşluğu.
    pub const CONTENT_MARGIN: i8 = 18;
    /// Sol gezinme panelinin genişliği.
    pub const NAV_WIDTH: f32 = 216.0;
    /// Gezinme öğesinin yüksekliği.
    pub const NAV_ITEM_HEIGHT: f32 = 30.0;
    /// Bölümler arasındaki dikey boşluk.
    pub const SECTION_GAP: f32 = 12.0;
    /// Modül kısayol kartının boyutu.
    pub const SHORTCUT_WIDTH: f32 = 198.0;
    pub const SHORTCUT_HEIGHT: f32 = 50.0;
}

/// Tema ayarlarını egui bağlamına uygular.
///
/// Uygulama her zaman açık temayı kullanır; işletim sisteminin koyu tema
/// tercihinden etkilenmez.
pub fn apply(context: &Context) {
    context.set_theme(ThemePreference::Light);

    context.style_mut_of(Theme::Light, |style| {
        style.visuals = build_visuals();
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        style.spacing.menu_margin = egui::Margin::same(6);

        // Kaydırma çubukları içeriğin üzerine binmesin; yer ayırsın. Aksi
        // hâlde uzun satırların son harfleri çubuğun altında kalır.
        style.spacing.scroll = egui::style::ScrollStyle::solid();
        style.spacing.scroll.bar_width = 8.0;
    });
}

/// Uygulamanın açık tema görsel ayarlarını üretir.
fn build_visuals() -> Visuals {
    let mut visuals = Visuals::light();

    visuals.override_text_color = Some(Palette::TEXT);
    visuals.panel_fill = Palette::BACKGROUND;
    visuals.window_fill = Palette::SURFACE;
    visuals.extreme_bg_color = Palette::SURFACE;
    visuals.faint_bg_color = Palette::CHROME;
    visuals.window_corner_radius = CornerRadius::same(8);
    visuals.window_stroke = Stroke::new(1.0, Palette::BORDER);
    visuals.hyperlink_color = Palette::ACCENT;

    visuals.selection.bg_fill = Palette::ACCENT.gamma_multiply(0.30);
    visuals.selection.stroke = Stroke::new(1.0, Palette::ACCENT);

    visuals.widgets.noninteractive.bg_fill = Palette::SURFACE;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Palette::BORDER);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(6);

    // Butonlar `weak_bg_fill` kullanır; bu değer verilmezse egui'nin varsayılan
    // gri tonu kullanılır ve paletle uyumsuz görünür.
    visuals.widgets.inactive.bg_fill = Palette::SURFACE;
    visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(0xE6, 0xEA, 0xEF);
    // Metin alanları beyaz kart üzerinde arka planla birleşmemeli; odak
    // almamış alanların da ince bir kenarlığı görünür olmalıdır.
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, Palette::BORDER);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(6);

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(0xDA, 0xE0, 0xE8);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(0xD2, 0xDA, 0xE4);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, Palette::ACCENT.gamma_multiply(0.7));
    visuals.widgets.hovered.corner_radius = CornerRadius::same(6);

    visuals.widgets.active.bg_fill = Palette::ACCENT.gamma_multiply(0.35);
    visuals.widgets.active.weak_bg_fill = Color32::from_rgb(0xC3, 0xCD, 0xDA);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, Palette::ACCENT);
    visuals.widgets.active.corner_radius = CornerRadius::same(6);

    visuals.widgets.open.weak_bg_fill = Color32::from_rgb(0xD2, 0xDA, 0xE4);
    visuals.widgets.open.corner_radius = CornerRadius::same(6);

    visuals
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{FontFamily, FontId, Pos2, RawInput, Rect, vec2};

    /// Arayüzde geçen tüm Türkçe harfleri içeren örnek metin.
    const TURKCE_HARFLER: &str = "çğıöşüÇĞİÖŞÜ";

    /// Arayüzde kullanılan yazı tiplerinin Türkçe karakterleri gerçekten
    /// çizebildiğini doğrular.
    ///
    /// egui'nin gömülü yazı tipleri Latin Extended-A kapsamını içerir; bu test
    /// kapsamın korunmasını güvence altına alır (eksik karakter "tofu" kutusu
    /// olarak çizilirdi).
    #[test]
    fn turkce_karakterler_yazitiplerinde_bulunur() {
        let context = Context::default();
        apply(&context);

        let screen = Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0));
        let mut kontrol_edildi = false;

        context
            .run_ui(
                RawInput {
                    screen_rect: Some(screen),
                    ..Default::default()
                },
                |_ui| {
                    for family in [FontFamily::Proportional, FontFamily::Monospace] {
                        let font_id = FontId::new(14.0, family);
                        for character in TURKCE_HARFLER.chars() {
                            let text = character.to_string();
                            assert!(
                                context.fonts_mut(|fonts| fonts.has_glyphs(&font_id, &text)),
                                "{font_id:?} yazı tipinde {character:?} karakteri bulunamadı"
                            );
                        }
                    }
                    kontrol_edildi = true;
                },
            )
            .drop_without_applying_deltas();

        assert!(kontrol_edildi, "yazı tipleri kontrol edilmeliydi");
    }
}
