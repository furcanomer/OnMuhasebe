// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Pencere simgesi: `assets/icon.ico` dosyasını harici bir görüntü kütüphanesi
//! olmadan çözer.
//!
//! ICO içindeki 32 bit BMP görüntüsü BGRA ve alt-satır-önce biçimindedir;
//! [`egui::IconData`] ise RGBA ve üst-satır-önce bekler. Bu modül aradaki
//! dönüşümü yapar.

use egui::IconData;

/// Derlemeye gömülen simge dosyası.
const ICON_BYTES: &[u8] = include_bytes!("../assets/icon.ico");

/// Simgeyi yükler. Dosya geçersizse `None` döner; uygulama simgesiz açılır.
pub fn load() -> Option<IconData> {
    parse_ico(ICON_BYTES)
}

fn parse_ico(data: &[u8]) -> Option<IconData> {
    if data.len() < 6 || read_u16(data, 0)? != 0 || read_u16(data, 2)? != 1 {
        return None;
    }
    let count = read_u16(data, 4)? as usize;
    if count == 0 {
        return None;
    }

    for index in 0..count {
        let entry = 6 + index * 16;
        if entry + 16 > data.len() {
            return None;
        }
        let width = data[entry] as u32;
        let height = data[entry + 1] as u32;
        let bit_count = read_u16(data, entry + 6)?;
        let size = read_u32(data, entry + 8)? as usize;
        let offset = read_u32(data, entry + 12)? as usize;
        if bit_count != 32 || width == 0 || height == 0 {
            continue;
        }
        if offset.checked_add(size)? > data.len() {
            continue;
        }
        if let Some(icon) = parse_dib(&data[offset..offset + size], width, height) {
            return Some(icon);
        }
    }

    None
}

fn parse_dib(data: &[u8], width: u32, height: u32) -> Option<IconData> {
    if data.len() < 40 {
        return None;
    }
    let header_size = read_u32(data, 0)? as usize;
    let compression = read_u32(data, 16)?;
    let declared_height = read_u32(data, 8)?;
    if header_size < 40 || compression != 0 || declared_height < height * 2 {
        return None;
    }

    let pixel_bytes = (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(4)?;
    let start = header_size;
    if start.checked_add(pixel_bytes)? > data.len() {
        return None;
    }

    let mut rgba = vec![0_u8; pixel_bytes];
    let row_bytes = width as usize * 4;
    for row in 0..height as usize {
        let source_row = start + (height as usize - 1 - row) * row_bytes;
        let target_row = row * row_bytes;
        for column in 0..width as usize {
            let source = source_row + column * 4;
            let target = target_row + column * 4;
            rgba[target] = data[source + 2];
            rgba[target + 1] = data[source + 1];
            rgba[target + 2] = data[source];
            rgba[target + 3] = data[source + 3];
        }
    }

    Some(IconData {
        rgba,
        width,
        height,
    })
}

fn read_u16(data: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        data.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        data.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gomulu_simge_cozulur() {
        let icon = load().expect("assets/icon.ico çözülebilmeli");
        assert_eq!(icon.width, 32);
        assert_eq!(icon.height, 32);
        assert_eq!(icon.rgba.len(), 32 * 32 * 4);
        assert!(
            icon.rgba
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[3] == 255)
        );
    }

    #[test]
    fn gecersiz_veri_none_doner() {
        assert!(parse_ico(&[]).is_none());
        assert!(parse_ico(b"not an icon").is_none());
    }
}
