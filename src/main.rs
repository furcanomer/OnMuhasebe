// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Çalıştırılabilir dosyanın giriş noktası.
//!
//! Görevi yalnızca katmanları birbirine bağlamaktır: yapılandırmayı çözer,
//! [`egui`]/[`eframe`] penceresini oluşturur ve uygulama durumunu pencereye verir.
//! İş mantığı bu dosyada bulunmaz.
//!
//! ## Yeniden boyutlandırma ve pencere
//!
//! Pencere başlığı "Ön Muhasebe" olarak ayarlanır, başlangıç boyutu 1180×760,
//! en küçük boyutu 900×600'dür ve pencere serbestçe yeniden boyutlandırılabilir.

// Dağıtım (release) derlemesinde arka planda siyah konsol penceresi açılmasın.
// Hata durumları yine de arayüzde gösterilir (bkz. `OnMuhasebeApp::new`).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use on_muhasebe::app::OnMuhasebeApp;
use on_muhasebe::config::AppConfig;
use on_muhasebe::{APP_NAME, APP_VERSION, icon, logging};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::load()?;

    // Veritabanı dosyası ilk açılışta oluşturulur. Hata oluşursa uygulama
    // yine başlatılır ve sorun karşılama ekranında gösterilir.
    config.ensure_data_dir()?;

    // Log altyapısı uygulamadan önce kurulur; panik hook'u da burada takılır.
    let _logging_guard = match logging::init(&config, false) {
        Ok(guard) => Some(guard),
        Err(error) => {
            eprintln!("Log altyapısı başlatılamadı: {error}");
            None
        }
    };
    logging::install_panic_hook();
    tracing::info!("uygulama başlatılıyor");

    let mut viewport = egui::ViewportBuilder::default()
        .with_title(format!("{APP_NAME} — Giriş — v{APP_VERSION}"))
        .with_inner_size([1180.0, 760.0])
        .with_min_inner_size([900.0, 600.0])
        .with_resizable(true);
    if let Some(icon) = icon::load() {
        viewport = viewport.with_icon(icon);
    }

    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        APP_NAME,
        native_options,
        Box::new(move |creation_context| {
            Ok(Box::new(OnMuhasebeApp::new(creation_context, config)))
        }),
    )?;
    tracing::info!("uygulama kapandı");

    Ok(())
}
