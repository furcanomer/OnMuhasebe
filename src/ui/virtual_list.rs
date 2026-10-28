// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Sanal liste yardımcıları.
//!
//! `egui::ScrollArea::show_rows` yalnızca görünür aralığı çizer. Bu modül
//! aralık hesabını ayrıca saf fonksiyon olarak sunar; böylece büyük listelerde
//! tüm satırların çizilmediği testlerde doğrulanabilir.

use std::ops::Range;

/// Görünür satır aralığını hesaplar.
///
/// `overscan`, görünür alanın üstünde/altında fazladan çizilecek satır sayısıdır
/// (kaydırma sırasında boşluk görünmemesi için).
pub fn visible_range(
    total_rows: usize,
    row_height: f32,
    scroll_offset: f32,
    viewport_height: f32,
    overscan: usize,
) -> Range<usize> {
    if total_rows == 0 || row_height <= 0.0 || viewport_height <= 0.0 {
        return 0..0;
    }
    let row_height = f64::from(row_height);
    let offset = f64::from(scroll_offset.max(0.0));
    let viewport = f64::from(viewport_height.max(0.0));
    let first = ((offset / row_height).floor() as usize).saturating_sub(overscan);
    let last_visible = ((offset + viewport) / row_height).ceil() as usize;
    let last = last_visible.saturating_add(overscan).min(total_rows);
    first.min(last)..last
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn milyon_satirda_yalnizca_gorunen_aralik_hesaplanir() {
        let range = visible_range(1_000_000, 30.0, 0.0, 3000.0, 5);
        assert_eq!(range.start, 0);
        assert_eq!(range.end, 105, "100 görünür + 5 overscan");
        assert!(range.len() < 200);
    }

    #[test]
    fn kaydirinca_aralik_kayar_ve_sinirlanir() {
        let range = visible_range(1_000_000, 30.0, 300.0, 300.0, 2);
        assert_eq!(range, 8..22);
        let end = visible_range(100, 30.0, 2700.0, 300.0, 5);
        assert_eq!(end, 85..100, "son satırdan öteye taşmamalı");
    }

    #[test]
    fn bos_liste_ve_gecersiz_olcu_bos_aralik() {
        assert_eq!(visible_range(0, 30.0, 0.0, 300.0, 5), 0..0);
        assert_eq!(visible_range(10, 0.0, 0.0, 300.0, 5), 0..0);
        assert_eq!(visible_range(10, 30.0, 0.0, 0.0, 5), 0..0);
    }
}
