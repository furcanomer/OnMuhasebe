// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Pencerenin genel yerleşimi.
//!
//! Yerleşim sırası önemlidir: paneller (`Panel`, `CentralPanel`) merkez
//! içerikten önce eklenir.
//!
//! ```text
//! +------------------------------------------------------------------+
//! | üst menü çubuğu: Dosya | Modüller | Yardım                        |
//! +------------------------------------------------------------------+
//! | üst bilgi: uygulama adı | aktif sayfa başlığı          tarih      |
//! +----------------+-------------------------------------------------+
//! | sol menü       | içerik (aktif görünüm)                          |
//! | (bölümler)     |                                                 |
//! +----------------+-------------------------------------------------+
//! | durum çubuğu: veritabanı durumu, şema sürümü, program sürümü      |
//! +------------------------------------------------------------------+
//! ```
//!
//! Satır içi hizalamalar (sol/sağ) [`egui::Sides`] ile yapılır; bu bileşen
//! satır yüksekliğini sabitlediği için dar pencerelerde taşma oluşmaz.

use egui::{Frame, Label, Margin, Panel, Rect, RichText, Sides, Stroke};

use crate::APP_NAME;
use crate::APP_VERSION;
use crate::app::{OnMuhasebeApp, View};
use crate::ui::format::format_date_short;
use crate::ui::theme::{Palette, metrics};
use crate::ui::{
    cash, customers, income_expense, invoices, navigation, overview, products, reports, settings,
    transfer,
};

/// Yerleşimi oluşturan panellerin dış dikdörtgenleri.
///
/// Yalnızca düzenin doğrulanması (testler) ve hata ayıklama için kullanılır;
/// arayüz mantığı bu değerlere bağlı değildir.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Panels {
    /// Menü çubuğu.
    pub menu: Rect,
    /// Uygulama adı ve aktif sayfa başlığı.
    pub header: Rect,
    /// Sol gezinme paneli.
    pub navigation: Rect,
    /// Merkez içerik alanı.
    pub content: Rect,
    /// Alt durum çubuğu.
    pub status: Rect,
}

/// Programın tüm arayüzünü çizer ve panellerin ölçülerini döndürür.
pub fn draw(ui: &mut egui::Ui, app: &mut OnMuhasebeApp) -> Panels {
    // Yerel tarih gece yarısını geçtiğinde gösterim kendiliğinden güncellenir.
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_secs(60));

    app.poll_transfer();
    if app.transfer_state().busy_message.is_some() {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(100));
    }
    app.tick_notices();
    app.tick_search();
    if app.search_state().changed_at.is_some() || app.search_state().searching {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(30));
    }

    let menu = menu_bar(ui, app).response.rect;
    let header = header_bar(ui, app).response.rect;
    let status = status_bar(ui, app).response.rect;
    let navigation = navigation::show(ui, app).response.rect;
    let content = content(ui, app).response.rect;

    about_window(ui, app);
    transfer::show(ui.ctx(), app);
    crate::ui::overlays::show(ui.ctx(), app);

    Panels {
        menu,
        header,
        navigation,
        content,
        status,
    }
}

/// Üst menü çubuğu.
fn menu_bar(ui: &mut egui::Ui, app: &mut OnMuhasebeApp) -> egui::InnerResponse<()> {
    Panel::top("ana_menu").frame(chrome_frame()).show(ui, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("Dosya", |ui| {
                if ui.button("Tüm veriyi dışa aktar...").clicked() {
                    ui.close();
                    if let Some(path) =
                        transfer::save_backup_dialog(&OnMuhasebeApp::default_backup_name())
                    {
                        let path = OnMuhasebeApp::backup_path(path);
                        let _ = app.start_backup_export(path);
                    }
                }
                if app.is_manager() && ui.button("Yedeği geri yükle...").clicked() {
                    ui.close();
                    if let Some(path) = transfer::open_backup_dialog() {
                        let _ = app.prepare_restore(path);
                    }
                }
                ui.separator();
                if ui.button("Oturumu kapat").clicked() {
                    ui.close();
                    app.logout_confirm = true;
                }
                if ui.button("Çıkış").clicked() {
                    ui.close();
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });

            ui.menu_button("Modüller", |ui| {
                for view in View::ALL {
                    if ui.button(view.title()).clicked() {
                        app.set_view(view);
                        ui.close();
                    }
                }
            });

            ui.menu_button("Yardım", |ui| {
                if ui.button("Klavye kısayolları").clicked() {
                    app.open_shortcuts_help();
                    ui.close();
                }
                if ui.button("Hata ayrıntıları").clicked() {
                    app.error_details_open = true;
                    ui.close();
                }
                if ui.button("Log klasörünü aç").clicked() {
                    app.open_logs_folder();
                    ui.close();
                }
                if ui.button("Hakkında").clicked() {
                    app.set_about_open(true);
                    ui.close();
                }
            });
        });
    })
}

/// Üst bilgi çubuğu: uygulama adı, aktif sayfa başlığı ve yerel tarih.
fn header_bar(ui: &mut egui::Ui, app: &OnMuhasebeApp) -> egui::InnerResponse<()> {
    let today = format_date_short(chrono::Local::now().date_naive());

    Panel::top("ust_bilgi")
        .frame(
            Frame::new()
                .fill(Palette::SURFACE)
                .inner_margin(Margin::symmetric(
                    metrics::PANEL_PADDING_X,
                    metrics::PANEL_PADDING_Y + 2,
                ))
                .stroke(Stroke::new(1.0, Palette::BORDER)),
        )
        .show(ui, |ui| {
            Sides::new().height(24.0).show(
                ui,
                |ui| {
                    ui.label(
                        RichText::new(APP_NAME)
                            .size(16.0)
                            .strong()
                            .color(Palette::TEXT),
                    );
                    ui.separator();
                    ui.add(
                        Label::new(
                            RichText::new(app.view().title())
                                .size(13.5)
                                .color(Palette::MUTED),
                        )
                        .truncate(),
                    );
                },
                |ui| {
                    if let Some(actor) = app.current_actor() {
                        ui.label(
                            RichText::new(format!("{} · {}", actor.label(), actor.role.label()))
                                .size(12.5)
                                .color(Palette::MUTED),
                        );
                        ui.separator();
                    }
                    ui.label(RichText::new(today).size(13.0).color(Palette::MUTED));
                },
            );
        })
}

/// Alt durum çubuğu: veritabanı durumu, şema sürümü ve program sürümü.
fn status_bar(ui: &mut egui::Ui, app: &mut OnMuhasebeApp) -> egui::InnerResponse<()> {
    Panel::bottom("durum_cubugu")
        .frame(chrome_frame())
        .show(ui, |ui| {
            let healthy = app.is_healthy();
            let (color, label) = if healthy {
                (Palette::SUCCESS, "Program çalışıyor")
            } else {
                (Palette::DANGER, "Veritabanı bağlantısı kurulamadı")
            };
            let schema_version = app.status().map(|status| status.schema_version);
            let has_error = app.last_error_summary().is_some();
            let mut refresh = false;
            let mut show_error_details = false;

            Sides::new().height(22.0).show(
                ui,
                |ui| {
                    ui.label(RichText::new("●").color(color));
                    ui.label(RichText::new(label).size(12.5).color(Palette::MUTED));

                    if let Some(schema_version) = schema_version {
                        ui.label(
                            RichText::new(format!("Şema sürümü: {schema_version}"))
                                .size(12.5)
                                .color(Palette::MUTED),
                        );
                    }
                },
                |ui| {
                    if has_error {
                        show_error_details = ui.button("Hata ayrıntıları").clicked();
                        ui.separator();
                    }
                    ui.label(
                        RichText::new(format!("Sürüm {APP_VERSION}"))
                            .size(12.5)
                            .color(Palette::MUTED),
                    );
                    ui.separator();
                    refresh = ui.button("Durumu yenile").clicked();
                },
            );

            if refresh {
                app.refresh_status();
            }
            if show_error_details {
                app.error_details_open = true;
            }
        })
}

/// Merkez içerik alanı: aktif görünümü çizer.
fn content(ui: &mut egui::Ui, app: &mut OnMuhasebeApp) -> egui::InnerResponse<()> {
    egui::CentralPanel::default()
        .frame(
            Frame::new()
                .fill(Palette::BACKGROUND)
                .inner_margin(Margin::same(metrics::CONTENT_MARGIN)),
        )
        .show(ui, |ui| {
            let mut scroll = egui::ScrollArea::vertical().auto_shrink([false, false]);

            // Ekran değiştiğinde içerik en baştan gösterilir; aksi hâlde önceki
            // kaydırma konumu korunur ve kullanıcı yeni ekranın ortasında kalır.
            if app.take_scroll_reset() {
                scroll = scroll.vertical_scroll_offset(0.0);
            }

            scroll.show(ui, |ui| {
                ui.set_width(ui.available_width());

                match app.view() {
                    View::Overview => overview::show(ui, app),
                    View::Customers => customers::show(ui, app),
                    View::Inventory => products::show(ui, app),
                    View::PurchaseInvoices | View::SalesInvoices => invoices::show(ui, app),
                    View::CashAndBank => cash::show(ui, app),
                    View::IncomeExpense => income_expense::show(ui, app),
                    View::Reports => reports::show(ui, app),
                    View::Settings => settings::show(ui, app),
                }
            });
        })
}

/// "Hakkında" penceresi.
fn about_window(ui: &mut egui::Ui, app: &mut OnMuhasebeApp) {
    let mut open = app.about_open();
    if !open {
        return;
    }
    let mut close = false;

    let database_path = app
        .database()
        .and_then(|database| database.path())
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "Bellek içi / açılamadı".to_owned());
    let log = app
        .latest_log_file()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "Henüz log dosyası yok".to_owned());
    let system = format!(
        "{} · {} · {}-bit · {}",
        std::env::consts::OS,
        std::env::consts::ARCH,
        usize::BITS,
        std::env::var("PROCESSOR_ARCHITECTURE").unwrap_or_else(|_| "bilinmiyor".to_owned())
    );
    let info = format!(
        "{APP_NAME}\nSürüm: {APP_VERSION}\nGit: {}\nVeri dizini: {}\nVeritabanı: {}\nYedekler: {}\nLoglar: {}\nSon log: {}\nSQLite: {}\nSistem: {}\n",
        crate::GIT_HASH,
        app.config().data_dir().display(),
        database_path,
        app.config().backups_dir().display(),
        app.logs_dir().display(),
        log,
        rusqlite::version(),
        system
    );

    egui::Window::new("Hakkında")
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .default_width(520.0)
        .max_height(500.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ui.ctx(), |ui| {
            ui.set_min_width(460.0);
            ui.set_max_width(560.0);

            ui.label(RichText::new(APP_NAME).size(20.0).strong());
            ui.label(
                RichText::new(format!("Sürüm {APP_VERSION} · Git {}", crate::GIT_HASH))
                    .color(Palette::MUTED),
            );
            ui.add_space(6.0);
            ui.label("Windows için yerel (native) masaüstü ön muhasebe uygulaması.");
            ui.label(RichText::new("Rust + egui/eframe + SQLite").color(Palette::MUTED));
            ui.add_space(4.0);
            ui.label(RichText::new("BSD 3-Clause Lisansı ile açık kaynak").color(Palette::MUTED));
            ui.label("Geliştirici: @furcanomer");
            ui.label(
                RichText::new("GitHub: https://github.com/furcanomer/OnMuhasebe")
                    .color(Palette::MUTED),
            );
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(6.0);

            egui::Grid::new("about_grid")
                .num_columns(2)
                .spacing([12.0, 4.0])
                .show(ui, |ui| {
                    for (label, value) in [
                        ("Veri dizini", app.config().data_dir().display().to_string()),
                        ("Veritabanı", database_path.clone()),
                        ("Yedekler", app.config().backups_dir().display().to_string()),
                        ("Loglar", app.logs_dir().display().to_string()),
                        ("Son log", log.clone()),
                        ("SQLite", rusqlite::version().to_owned()),
                        ("Sistem", system.clone()),
                    ] {
                        ui.label(RichText::new(label).size(12.0).color(Palette::MUTED));
                        ui.add(Label::new(RichText::new(value).monospace().size(11.5)).wrap());
                        ui.end_row();
                    }
                });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Bilgileri kopyala").clicked() {
                    ui.ctx().copy_text(info.clone());
                }
                if ui.button("Log klasörünü aç").clicked() {
                    app.open_logs_folder();
                }
                if ui.button("Kapat").clicked() {
                    close = true;
                }
            });
        });

    if close {
        open = false;
    }
    app.set_about_open(open);
}

/// Menü ve durum çubuğu için ortak çerçeve.
fn chrome_frame() -> Frame {
    Frame::new()
        .fill(Palette::CHROME)
        .inner_margin(Margin::symmetric(
            metrics::PANEL_PADDING_X,
            metrics::PANEL_PADDING_Y,
        ))
        .stroke(Stroke::new(1.0, Palette::BORDER))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::product::Unit;
    use egui::{Pos2, vec2};
    use tempfile::TempDir;

    /// Verilen bölüm ve pencere boyutu için yerleşimi iki kare çalıştırır.
    ///
    /// Gerçek bir çizim yapılmaz; egui'nin yerleşim hesabı çalıştırılır ve
    /// panellerin ölçüleri okunur. Bu, arayüzün görsel olarak incelenemediği
    /// ortamlarda taşma/bozulma hatalarını yakalar.
    fn run_layout(view: View, width: f32, height: f32) -> (Panels, TempDir) {
        run_layout_with(view, width, height, |_| {})
    }

    /// [`run_layout`] ile aynıdır; ek olarak çizimden önce uygulama durumunu
    /// hazırlamak için bir geri çağırma alır (örneğin cari formunu açmak).
    fn run_layout_with(
        view: View,
        width: f32,
        height: f32,
        setup: impl FnOnce(&mut OnMuhasebeApp),
    ) -> (Panels, TempDir) {
        let directory = tempfile::tempdir().expect("geçici dizin oluşturulmalı");
        let config = AppConfig::with_data_dir(directory.path().join("veri"));

        let context = egui::Context::default();
        let mut app = OnMuhasebeApp::with_config(&context, config);
        app.set_view(view);
        setup(&mut app);

        let screen = Rect::from_min_size(Pos2::ZERO, vec2(width, height));

        // egui yerleşimi ilk karede tam oturmadığı için iki kare çalıştırılır.
        let _first_frame = frame(&context, &mut app, screen);
        let panels = frame(&context, &mut app, screen);

        (panels, directory)
    }

    /// Tek bir kareyi çalıştırır ve yerleşim ölçülerini döndürür.
    ///
    /// Testlerde gerçek bir çizim (grafik bağlamı) olmadığı için egui'nin
    /// ürettiği doku değişiklikleri açıkça temizlenir.
    fn frame(context: &egui::Context, app: &mut OnMuhasebeApp, screen: Rect) -> Panels {
        let input = egui::RawInput {
            screen_rect: Some(screen),
            ..Default::default()
        };

        let mut panels = None;
        context
            .run_ui(input, |ui| {
                panels = Some(draw(ui, app));
            })
            .drop_without_applying_deltas();

        panels.expect("yerleşim hesaplanmalı")
    }

    #[test]
    fn genel_bakis_yerlesimi_beklenen_olculerdedir() {
        let (panels, _directory) = run_layout(View::Overview, 1180.0, 760.0);

        assert!(
            panels.menu.height() <= 40.0,
            "menü çubuğu çok yüksek: {:?}",
            panels.menu
        );
        assert!(
            panels.header.height() <= 60.0,
            "üst bilgi çok yüksek: {:?}",
            panels.header
        );
        assert!(
            panels.status.height() <= 40.0,
            "durum çubuğu çok yüksek: {:?}",
            panels.status
        );
        assert!(
            (panels.navigation.width() - metrics::NAV_WIDTH).abs() < 1.0,
            "gezinme genişliği beklenen değerde değil: {:?}",
            panels.navigation
        );
        assert!(
            panels.content.height() > 300.0,
            "içerik alanı çok küçük: {:?}",
            panels.content
        );
    }

    #[test]
    fn ust_ve_alt_paneller_pencere_genisligini_kullanir() {
        let (panels, _directory) = run_layout(View::Overview, 1000.0, 700.0);

        for (name, rect) in [("menü", panels.menu), ("üst bilgi", panels.header)] {
            assert!(
                (rect.width() - 1000.0).abs() < 1.0,
                "{name} tam genişlikte olmalı: {rect:?}"
            );
            assert!((rect.left() - 0.0).abs() < 1.0, "{name} soldan başlamalı");
        }
    }

    #[test]
    fn dar_pencerede_paneller_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;
        let (panels, _directory) = run_layout(View::Overview, GENISLIK, YUKSEKLIK);

        for (name, rect) in [
            ("menü", panels.menu),
            ("üst bilgi", panels.header),
            ("gezinme", panels.navigation),
            ("içerik", panels.content),
            ("durum çubuğu", panels.status),
        ] {
            assert!(
                rect.right() <= GENISLIK + 1.0,
                "{name} sağdan taşıyor: {rect:?}"
            );
            assert!(
                rect.bottom() <= YUKSEKLIK + 1.0,
                "{name} aşağıdan taşıyor: {rect:?}"
            );
            assert!(rect.width() > 0.0, "{name} görünmez: {rect:?}");
            assert!(rect.height() > 0.0, "{name} görünmez: {rect:?}");
        }
    }

    #[test]
    fn tum_bolumler_hatasiz_cizilir() {
        for view in View::ALL {
            let (panels, _directory) = run_layout(view, 1180.0, 760.0);
            assert!(
                panels.content.height() > 300.0,
                "{view:?} bölümünde içerik alanı çok küçük: {:?}",
                panels.content
            );
        }
    }

    #[test]
    fn hakkinda_penceresi_acikken_de_yerlesim_bozulmaz() {
        let directory = tempfile::tempdir().expect("geçici dizin oluşturulmalı");
        let config = AppConfig::with_data_dir(directory.path().join("veri"));
        let context = egui::Context::default();
        let mut app = OnMuhasebeApp::with_config(&context, config);
        app.set_about_open(true);

        let screen = Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 700.0));
        frame(&context, &mut app, screen);
        frame(&context, &mut app, screen);

        assert!(app.about_open(), "pencere açık kalmalı");
    }

    /// Örnek bir cari hesap ekler ve kaydın kimliğini döndürür.
    fn ornek_cari_ekle(app: &mut OnMuhasebeApp, ad: &str) -> i64 {
        app.open_customer_new_form();
        {
            let state = app.customer_state_mut();
            state.draft.name = ad.to_owned();
            state.draft.phone = "0212 555 12 34".to_owned();
            state.draft.tax_office = "Beşiktaş".to_owned();
            state.draft.email = "info@ornek.com".to_owned();
        }
        app.submit_customer_form();

        app.customer_state()
            .detail_id
            .expect("cari hesap kaydedilmeli ve ayrıntısı açılmalı")
    }

    /// Panellerin verilen pencereye sığdığını doğrular.
    fn paneller_sigar(panels: &Panels, genislik: f32, yukseklik: f32, baglam: &str) {
        for (name, rect) in [
            ("menü", panels.menu),
            ("üst bilgi", panels.header),
            ("gezinme", panels.navigation),
            ("içerik", panels.content),
            ("durum çubuğu", panels.status),
        ] {
            assert!(
                rect.right() <= genislik + 1.0,
                "{baglam}: {name} sağdan taşıyor: {rect:?}"
            );
            assert!(
                rect.bottom() <= yukseklik + 1.0,
                "{baglam}: {name} aşağıdan taşıyor: {rect:?}"
            );
            assert!(rect.width() > 0.0, "{baglam}: {name} görünmez: {rect:?}");
            assert!(rect.height() > 0.0, "{baglam}: {name} görünmez: {rect:?}");
        }
    }

    #[test]
    fn cari_hesap_listesi_dar_pencerede_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;

        let (panels, _directory) = run_layout_with(View::Customers, GENISLIK, YUKSEKLIK, |app| {
            // Uzun unvan ve dolu alanlar en zorlayıcı durumdur.
            let id = ornek_cari_ekle(
                app,
                "Çok Uzun Bir Firma Unvanı İle Deneme Anonim Şirketi ve Ortakları",
            );
            app.open_customer_detail(id);
            app.open_customer_list();
        });

        paneller_sigar(&panels, GENISLIK, YUKSEKLIK, "cari listesi");
    }

    #[test]
    fn cari_hesap_formu_dar_pencerede_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;

        let (panels, _directory) = run_layout_with(View::Customers, GENISLIK, YUKSEKLIK, |app| {
            app.open_customer_new_form();
            app.customer_state_mut().draft.name = String::new();
            // Doğrulama hatalı form da çizilebilmelidir.
            app.submit_customer_form();
        });

        paneller_sigar(&panels, GENISLIK, YUKSEKLIK, "cari formu");
    }

    #[test]
    fn cari_hesap_detayi_dar_pencerede_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;

        let (panels, _directory) = run_layout_with(View::Customers, GENISLIK, YUKSEKLIK, |app| {
            ornek_cari_ekle(app, "Öztürk İnşaat Ltd. Şti.");
        });

        paneller_sigar(&panels, GENISLIK, YUKSEKLIK, "cari ayrıntısı");
    }

    /// Örnek bir ürün ekler (başlangıç stoğu ile) ve kaydın kimliğini döndürür.
    fn ornek_urun_ekle(app: &mut OnMuhasebeApp, ad: &str) -> i64 {
        app.open_product_new_form();
        {
            let state = app.product_state_mut();
            state.draft.name = ad.to_owned();
            state.draft.barcode = "8691234567890".to_owned();
            state.draft.unit = Unit::Kilogram;
            state.draft.category = "Bağlantı elemanları ve hırdavat".to_owned();
            state.draft.purchase_price = "1.234,56".to_owned();
            state.draft.sale_price = "1.999,00".to_owned();
            state.draft.min_stock = "10".to_owned();
            state.draft.starting_stock = "5".to_owned();
        }
        app.submit_product_form();

        app.product_state()
            .detail_id
            .expect("ürün kaydedilmeli ve ayrıntısı açılmalı")
    }

    #[test]
    fn urun_listesi_dar_pencerede_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;

        let (panels, _directory) = run_layout_with(View::Inventory, GENISLIK, YUKSEKLIK, |app| {
            let id = ornek_urun_ekle(
                app,
                "Çok Uzun Bir Ürün Adı İle Deneme Malzemesi ve Yedek Parça Çeşidi",
            );
            app.open_product_detail(id);
            app.open_product_list();
        });

        paneller_sigar(&panels, GENISLIK, YUKSEKLIK, "ürün listesi");
    }

    #[test]
    fn urun_formu_dar_pencerede_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;

        let (panels, _directory) = run_layout_with(View::Inventory, GENISLIK, YUKSEKLIK, |app| {
            app.open_product_new_form();
            app.product_state_mut().draft.name = String::new();
            // Doğrulama hatalı form da çizilebilmelidir.
            app.submit_product_form();
        });

        paneller_sigar(&panels, GENISLIK, YUKSEKLIK, "ürün formu");
    }

    #[test]
    fn urun_detayi_dar_pencerede_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;

        let (panels, _directory) = run_layout_with(View::Inventory, GENISLIK, YUKSEKLIK, |app| {
            ornek_urun_ekle(app, "Çelik Vida 4x40");
        });

        paneller_sigar(&panels, GENISLIK, YUKSEKLIK, "ürün ayrıntısı");
    }

    #[test]
    fn stok_hareketi_formu_dar_pencerede_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;

        let (panels, _directory) = run_layout_with(View::Inventory, GENISLIK, YUKSEKLIK, |app| {
            let id = ornek_urun_ekle(app, "Çelik Vida 4x40");
            app.open_movement_form(id);
            app.product_state_mut().movement_draft.quantity = "8".to_owned();
            // Negatif stok onayı kartı da çizilebilmelidir.
            app.product_state_mut().movement_draft.movement_type =
                crate::product::MovementType::Out;
            app.submit_movement_form();
        });

        paneller_sigar(&panels, GENISLIK, YUKSEKLIK, "stok hareketi formu");
    }

    /// Cari ve ürün ekleyip bir satış faturası kesinleştirir; kimliğini döner.
    fn ornek_fatura_ekle(app: &mut OnMuhasebeApp) -> i64 {
        app.set_view(View::Customers);
        app.open_customer_new_form();
        {
            let state = app.customer_state_mut();
            state.draft.name =
                "Çok Uzun Bir Firma Unvanı İle Deneme Anonim Şirketi ve Ortakları".to_owned();
        }
        app.submit_customer_form();
        let customer_id = app
            .customer_state()
            .detail_id
            .expect("cari hesap kaydedilmeli");

        let product_id = ornek_urun_ekle(
            app,
            "Çok Uzun Bir Ürün Adı İle Deneme Malzemesi ve Yedek Parça",
        );

        app.set_view(View::SalesInvoices);
        app.open_invoice_new_form();
        {
            let state = app.invoice_state_mut();
            state.draft.customer_id = Some(customer_id);
            state.draft.notes =
                "Uzun bir açıklama satırı: sevkiyat sonrası fatura düzenlenmiştir.".to_owned();
            state.draft.lines[0].product_id = Some(product_id);
            state.draft.lines[0].quantity = "2".to_owned();
            state.draft.lines[0].unit_price = "1.250,50".to_owned();
            state.draft.lines[0].discount_rate = "10".to_owned();
            state.draft.lines[0].vat_rate = "20".to_owned();
        }
        app.submit_invoice_form();
        assert!(
            app.invoice_state().pending.is_some(),
            "kaydetme onayı beklenmeli"
        );
        app.confirm_invoice_save();
        app.invoice_state().detail_id.expect("fatura kaydedilmeli")
    }

    #[test]
    fn fatura_listesi_dar_pencerede_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;

        let (panels, _directory) =
            run_layout_with(View::SalesInvoices, GENISLIK, YUKSEKLIK, |app| {
                let id = ornek_fatura_ekle(app);
                app.open_invoice_detail(id);
                app.open_invoice_list();
            });

        paneller_sigar(&panels, GENISLIK, YUKSEKLIK, "fatura listesi");
    }

    #[test]
    fn fatura_formu_dar_pencerede_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;

        let (panels, _directory) =
            run_layout_with(View::SalesInvoices, GENISLIK, YUKSEKLIK, |app| {
                let product_id = ornek_urun_ekle(app, "Çelik Vida 4x40");
                app.set_view(View::SalesInvoices);
                app.open_invoice_new_form();
                app.add_invoice_line();
                {
                    let state = app.invoice_state_mut();
                    // Uzun metin ve hatalı değerler en zorlayıcı durumdur.
                    state.draft.notes = "Uzun not: ".repeat(8);
                    state.draft.due_date = "31.12.2026".to_owned();
                    state.draft.lines[0].product_id = Some(product_id);
                    state.draft.lines[0].description = "Uzun açıklama ".repeat(6);
                    state.draft.lines[0].quantity = "0".to_owned();
                    state.draft.lines[0].unit_price = "abc".to_owned();
                }
                app.submit_invoice_form();
            });

        paneller_sigar(&panels, GENISLIK, YUKSEKLIK, "fatura formu");
    }

    #[test]
    fn fatura_detayi_ve_iptal_onayi_dar_pencerede_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;

        let (panels, _directory) =
            run_layout_with(View::SalesInvoices, GENISLIK, YUKSEKLIK, |app| {
                ornek_fatura_ekle(app);
                // İptal onay kartı (boş nedenle hata) de çizilebilmelidir.
                app.start_invoice_cancel();
                app.submit_invoice_cancel();
            });

        paneller_sigar(
            &panels,
            GENISLIK,
            YUKSEKLIK,
            "fatura ayrıntısı ve iptal onayı",
        );
    }

    #[test]
    fn fatura_kaydetme_onayi_dar_pencerede_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;

        let (panels, _directory) =
            run_layout_with(View::SalesInvoices, GENISLIK, YUKSEKLIK, |app| {
                app.set_view(View::Customers);
                app.open_customer_new_form();
                app.customer_state_mut().draft.name = "Öztürk İnşaat".to_owned();
                app.submit_customer_form();
                let customer_id = app
                    .customer_state()
                    .detail_id
                    .expect("cari hesap kaydedilmeli");
                let product_id = ornek_urun_ekle(app, "Çelik Vida 4x40");

                // Yeni fatura ile stoğu eksiye düşürecek onay kartı çizilir.
                app.set_view(View::SalesInvoices);
                app.open_invoice_new_form();
                {
                    let state = app.invoice_state_mut();
                    state.draft.customer_id = Some(customer_id);
                    state.draft.lines[0].product_id = Some(product_id);
                    state.draft.lines[0].quantity = "9999".to_owned();
                    state.draft.lines[0].unit_price = "1".to_owned();
                }
                app.submit_invoice_form();
            });

        paneller_sigar(&panels, GENISLIK, YUKSEKLIK, "fatura kaydetme onayı");
    }

    // -----------------------------------------------------------------------
    // Kasa/banka ve tahsilat/ödeme ekranları
    // -----------------------------------------------------------------------

    /// Örnek bir kasa hesabı ekler.
    fn ornek_kasa_ekle(app: &mut OnMuhasebeApp, ad: &str, acilis: &str) -> i64 {
        app.set_view(View::CashAndBank);
        app.open_cash_account_new_form();
        {
            let state = app.cash_state_mut();
            state.account_draft.name = ad.to_owned();
            state.account_draft.opening_balance = acilis.to_owned();
        }
        app.submit_cash_account_form();
        app.cash_state()
            .account_list
            .iter()
            .find(|account| account.name == ad)
            .map(|account| account.id)
            .expect("kasa hesabı kaydedilmeli")
    }

    /// Örnek bir banka hesabı ekler (uzun IBAN ile).
    fn ornek_banka_ekle(app: &mut OnMuhasebeApp, ad: &str, banka: &str, iban: &str) -> i64 {
        app.set_view(View::CashAndBank);
        app.open_cash_account_new_form();
        {
            let state = app.cash_state_mut();
            state.account_draft.name = ad.to_owned();
            state.account_draft.account_type = crate::cash::AccountType::Banka;
            state.account_draft.bank_name = banka.to_owned();
            state.account_draft.iban = iban.to_owned();
        }
        app.submit_cash_account_form();
        app.cash_state()
            .account_list
            .iter()
            .find(|account| account.name == ad)
            .map(|account| account.id)
            .expect("banka hesabı kaydedilmeli")
    }

    #[test]
    fn kasa_hesap_listesi_dar_pencerede_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;

        let (panels, _directory) = run_layout_with(View::CashAndBank, GENISLIK, YUKSEKLIK, |app| {
            ornek_kasa_ekle(app, "Merkez Kasa", "1.000,00");
            let pasif = ornek_kasa_ekle(app, "Şube Kasası", "250,00");
            ornek_banka_ekle(
                app,
                "Çok Uzun Banka Hesabı Adı İle Deneme",
                "Türkiye Cumhuriyeti Ziraat Bankası A.Ş.",
                "TR330006100519786457841326",
            );
            app.toggle_cash_account_active(pasif);
            app.cash_state_mut().account_filter.search = "kasa".to_owned();
            app.refresh_cash_account_list();
        });

        paneller_sigar(&panels, GENISLIK, YUKSEKLIK, "kasa hesap listesi");
    }

    #[test]
    fn kasa_hesap_formu_dar_pencerede_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;

        let (panels, _directory) = run_layout_with(View::CashAndBank, GENISLIK, YUKSEKLIK, |app| {
            ornek_kasa_ekle(app, "Merkez Kasa", "10,00");
            app.open_cash_account_edit_form(
                app.cash_state()
                    .account_list
                    .first()
                    .map(|account| account.id)
                    .expect("hesap bulunmalı"),
            );
            // Hatalı girişlerle doğrulama mesajları da çizilmelidir.
            {
                let state = app.cash_state_mut();
                state.account_draft.name =
                    "Çok Uzun Bir Kasa Hesabı Adı İle Deneme Şubesi".to_owned();
                state.account_draft.account_type = crate::cash::AccountType::Banka;
                state.account_draft.iban = "TR12".to_owned();
                state.account_draft.opening_balance = "abc".to_owned();
            }
            app.submit_cash_account_form();
        });

        paneller_sigar(&panels, GENISLIK, YUKSEKLIK, "kasa hesap formu");
    }

    #[test]
    fn tahsilat_formu_dar_pencerede_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;

        let (panels, _directory) = run_layout_with(View::CashAndBank, GENISLIK, YUKSEKLIK, |app| {
            let invoice_id = ornek_fatura_ekle(app);
            ornek_kasa_ekle(app, "Merkez Kasa", "0,00");
            app.open_payment_form_for_invoice(invoice_id);
            // Aşırı dağıtım uyarısı ve uzun açıklama en zorlayıcı durumdur.
            {
                let state = app.cash_state_mut();
                state.payment_draft.amount = "1,00".to_owned();
                state.payment_draft.description = "Uzun açıklama metni ".repeat(5);
            }
            app.submit_payment_form();
        });

        paneller_sigar(&panels, GENISLIK, YUKSEKLIK, "tahsilat formu");
    }

    #[test]
    fn hareket_listesi_dar_pencerede_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;

        let (panels, _directory) = run_layout_with(View::CashAndBank, GENISLIK, YUKSEKLIK, |app| {
            let invoice_id = ornek_fatura_ekle(app);
            ornek_kasa_ekle(app, "Merkez Kasa", "0,00");
            app.open_payment_form_for_invoice(invoice_id);
            app.submit_payment_form();
            app.confirm_payment_save();

            // İptal edilmiş bir kayıt da listede görünmelidir.
            let payment_id = app
                .cash_state()
                .movement_list
                .first()
                .map(|payment| payment.id)
                .expect("kayıt olmalı");
            app.open_payment_detail(payment_id);
            app.start_payment_cancel();
            app.cash_state_mut().cancel_reason = "Sehven girildi".to_owned();
            app.submit_payment_cancel();

            app.open_payment_list();
            app.cash_state_mut().movement_filter.search = "Uzun".to_owned();
            app.refresh_payment_list();
        });

        paneller_sigar(&panels, GENISLIK, YUKSEKLIK, "hareket listesi");
    }

    #[test]
    fn hareket_detayi_ve_iptal_onayi_dar_pencerede_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;

        let (panels, _directory) = run_layout_with(View::CashAndBank, GENISLIK, YUKSEKLIK, |app| {
            let invoice_id = ornek_fatura_ekle(app);
            ornek_kasa_ekle(app, "Merkez Kasa", "0,00");
            app.open_payment_form_for_invoice(invoice_id);
            app.submit_payment_form();
            app.confirm_payment_save();

            let payment_id = app
                .cash_state()
                .movement_list
                .first()
                .map(|payment| payment.id)
                .expect("kayıt olmalı");
            app.open_payment_detail(payment_id);
            // İptal onay kartı (boş nedenle hata) da çizilebilmelidir.
            app.start_payment_cancel();
            app.submit_payment_cancel();
        });

        paneller_sigar(
            &panels,
            GENISLIK,
            YUKSEKLIK,
            "hareket ayrıntısı ve iptal onayı",
        );
    }

    // -----------------------------------------------------------------------
    // Gelir/gider ekranları
    // -----------------------------------------------------------------------

    /// Örnek bir gelir/gider kategorisi ekler.
    fn ornek_gelir_gider_kategorisi(
        app: &mut OnMuhasebeApp,
        ad: &str,
        tur: crate::income_expense::EntryType,
    ) -> i64 {
        app.set_view(View::IncomeExpense);
        app.open_income_expense_category_new_form();
        {
            let state = app.income_expense_state_mut();
            state.category_draft.name = ad.to_owned();
            state.category_draft.category_type = tur;
            state.category_draft.description = "Uzun açıklama metni ".repeat(6);
        }
        app.submit_income_expense_category_form();
        app.income_expense_state()
            .categories
            .iter()
            .find(|category| category.name == ad)
            .map(|category| category.id)
            .expect("kategori kaydedilmeli")
    }

    /// Örnek bir gelir/gider hareketi ekler.
    fn ornek_gelir_gider_ekle(
        app: &mut OnMuhasebeApp,
        tur: crate::income_expense::EntryType,
        kategori: i64,
        hesap: i64,
        tutar: &str,
    ) -> i64 {
        app.open_income_expense_entry_form(tur);
        app.set_income_expense_category(Some(kategori));
        app.set_income_expense_cash_account(Some(hesap));
        {
            let state = app.income_expense_state_mut();
            state.entry_draft.amount = tutar.to_owned();
            state.entry_draft.description = "Uzun açıklama metni ".repeat(6);
            state.entry_draft.document_ref = "FIS-2026-0001".to_owned();
        }
        app.submit_income_expense_entry_form();
        app.confirm_income_expense_entry_save();
        app.income_expense_state()
            .entry_detail_id
            .expect("gelir/gider kaydedilmeli")
    }

    #[test]
    fn gelir_gider_kategori_listesi_dar_pencerede_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;

        let (panels, _directory) =
            run_layout_with(View::IncomeExpense, GENISLIK, YUKSEKLIK, |app| {
                let pasif = ornek_gelir_gider_kategorisi(
                    app,
                    "Çok Uzun Bir Kategori Adı İle Deneme Kırtasiye Gideri",
                    crate::income_expense::EntryType::Gider,
                );
                ornek_gelir_gider_kategorisi(
                    app,
                    "Kira Geliri",
                    crate::income_expense::EntryType::Gelir,
                );
                app.toggle_income_expense_category_active(pasif);
                app.set_income_expense_tab(crate::app::IncomeExpenseTab::Categories);
                app.income_expense_state_mut().category_filter.search = "kategori".to_owned();
                app.refresh_income_expense_category_list();
            });

        paneller_sigar(&panels, GENISLIK, YUKSEKLIK, "gelir/gider kategori listesi");
    }

    #[test]
    fn gelir_gider_kategori_formu_dar_pencerede_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;

        let (panels, _directory) =
            run_layout_with(View::IncomeExpense, GENISLIK, YUKSEKLIK, |app| {
                app.open_income_expense_category_new_form();
                {
                    let state = app.income_expense_state_mut();
                    state.category_draft.name = "a".repeat(120);
                    state.category_draft.description = "Uzun açıklama ".repeat(12);
                }
                app.submit_income_expense_category_form();
            });

        paneller_sigar(&panels, GENISLIK, YUKSEKLIK, "gelir/gider kategori formu");
    }

    #[test]
    fn gelir_gider_hareket_listesi_dar_pencerede_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;

        let (panels, _directory) =
            run_layout_with(View::IncomeExpense, GENISLIK, YUKSEKLIK, |app| {
                let hesap = ornek_kasa_ekle(app, "Merkez Kasa", "1.000,00");
                let kategori = ornek_gelir_gider_kategorisi(
                    app,
                    "Çok Uzun Bir Kategori Adı İle Deneme Kira Gideri",
                    crate::income_expense::EntryType::Gider,
                );
                let id = ornek_gelir_gider_ekle(
                    app,
                    crate::income_expense::EntryType::Gider,
                    kategori,
                    hesap,
                    "250,00",
                );
                app.open_income_expense_detail(id);
                app.start_income_expense_cancel();
                app.income_expense_state_mut().cancel_reason = "Sehven girildi".to_owned();
                app.submit_income_expense_cancel();
                app.open_income_expense_list();
                app.income_expense_state_mut().entry_filter.search = "uzun".to_owned();
                app.refresh_income_expense_list();
            });

        paneller_sigar(&panels, GENISLIK, YUKSEKLIK, "gelir/gider hareket listesi");
    }

    #[test]
    fn gelir_gider_hareket_formu_dar_pencerede_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;

        let (panels, _directory) =
            run_layout_with(View::IncomeExpense, GENISLIK, YUKSEKLIK, |app| {
                let hesap = ornek_kasa_ekle(app, "Merkez Kasa", "100,00");
                let kategori = ornek_gelir_gider_kategorisi(
                    app,
                    "Kira",
                    crate::income_expense::EntryType::Gider,
                );
                app.open_income_expense_entry_form(crate::income_expense::EntryType::Gider);
                app.set_income_expense_category(Some(kategori));
                app.set_income_expense_cash_account(Some(hesap));
                {
                    let state = app.income_expense_state_mut();
                    state.entry_draft.amount = "abc".to_owned();
                    state.entry_draft.description = "Uzun açıklama ".repeat(12);
                    state.entry_draft.document_ref = "F".repeat(120);
                }
                app.submit_income_expense_entry_form();
            });

        paneller_sigar(&panels, GENISLIK, YUKSEKLIK, "gelir/gider hareket formu");
    }

    #[test]
    fn gelir_gider_ozeti_dar_pencerede_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;

        let (panels, _directory) =
            run_layout_with(View::IncomeExpense, GENISLIK, YUKSEKLIK, |app| {
                let hesap = ornek_kasa_ekle(app, "Merkez Kasa", "1.000,00");
                let gelir = ornek_gelir_gider_kategorisi(
                    app,
                    "Çok Uzun Bir Kira Geliri Kategorisi Adı",
                    crate::income_expense::EntryType::Gelir,
                );
                let gider = ornek_gelir_gider_kategorisi(
                    app,
                    "Çok Uzun Bir Kira Gideri Kategorisi Adı",
                    crate::income_expense::EntryType::Gider,
                );
                ornek_gelir_gider_ekle(
                    app,
                    crate::income_expense::EntryType::Gelir,
                    gelir,
                    hesap,
                    "5.000,00",
                );
                ornek_gelir_gider_ekle(
                    app,
                    crate::income_expense::EntryType::Gider,
                    gider,
                    hesap,
                    "2.500,00",
                );
                app.set_income_expense_tab(crate::app::IncomeExpenseTab::Summary);
                app.set_income_expense_summary_this_month();
            });

        paneller_sigar(&panels, GENISLIK, YUKSEKLIK, "gelir/gider özeti");
    }

    #[test]
    fn kasa_hareket_listesi_gelir_giderle_birlikte_dar_pencerede_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;

        let (panels, _directory) = run_layout_with(View::CashAndBank, GENISLIK, YUKSEKLIK, |app| {
            let invoice_id = ornek_fatura_ekle(app);
            let hesap = ornek_kasa_ekle(app, "Merkez Kasa", "0,00");
            app.open_payment_form_for_invoice(invoice_id);
            app.submit_payment_form();
            app.confirm_payment_save();

            let gelir = ornek_gelir_gider_kategorisi(
                app,
                "Kira Geliri",
                crate::income_expense::EntryType::Gelir,
            );
            let gider =
                ornek_gelir_gider_kategorisi(app, "Kira", crate::income_expense::EntryType::Gider);
            ornek_gelir_gider_ekle(
                app,
                crate::income_expense::EntryType::Gelir,
                gelir,
                hesap,
                "5.000,00",
            );
            ornek_gelir_gider_ekle(
                app,
                crate::income_expense::EntryType::Gider,
                gider,
                hesap,
                "2.500,00",
            );

            app.set_view(View::CashAndBank);
            app.set_cash_tab(crate::app::CashTab::Movements);
            app.set_movement_tab(crate::app::MovementTab::All);
            app.cash_state_mut().movement_filter.search = "Uzun".to_owned();
            app.refresh_payment_list();
        });

        paneller_sigar(
            &panels,
            GENISLIK,
            YUKSEKLIK,
            "birleşik kasa hareket listesi",
        );
    }

    // -----------------------------------------------------------------------
    // Raporlar
    // -----------------------------------------------------------------------

    /// Rapor bölümüne geçer ve istenen sekmeyi açar.
    fn rapor_sekmesi(app: &mut OnMuhasebeApp, tab: crate::report::ReportKind) {
        app.set_view(View::Reports);
        app.load_report_options();
        app.set_report_tab(tab);
    }

    #[test]
    fn rapor_cari_ekstre_dar_pencerede_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;

        let (panels, _directory) = run_layout_with(View::Reports, GENISLIK, YUKSEKLIK, |app| {
            ornek_fatura_ekle(app);
            rapor_sekmesi(app, crate::report::ReportKind::CustomerStatement);
            let customer_id = app
                .reports_state()
                .customers
                .first()
                .map(|customer| customer.id)
                .expect("cari bulunmalı");
            app.set_report_customer(Some(customer_id));
        });

        paneller_sigar(&panels, GENISLIK, YUKSEKLIK, "cari ekstre raporu");
    }

    #[test]
    fn rapor_cari_bakiye_ozeti_dar_pencerede_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;

        let (panels, _directory) = run_layout_with(View::Reports, GENISLIK, YUKSEKLIK, |app| {
            ornek_fatura_ekle(app);
            rapor_sekmesi(app, crate::report::ReportKind::CustomerBalances);
        });

        paneller_sigar(&panels, GENISLIK, YUKSEKLIK, "cari bakiye özeti raporu");
    }

    #[test]
    fn rapor_stok_durumu_dar_pencerede_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;

        let (panels, _directory) = run_layout_with(View::Reports, GENISLIK, YUKSEKLIK, |app| {
            ornek_urun_ekle(app, "Çok Uzun Bir Ürün Adı İle Deneme Malzemesi");
            rapor_sekmesi(app, crate::report::ReportKind::StockStatus);
        });

        paneller_sigar(&panels, GENISLIK, YUKSEKLIK, "stok durumu raporu");
    }

    #[test]
    fn rapor_satis_alis_ozeti_dar_pencerede_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;

        let (panels, _directory) = run_layout_with(View::Reports, GENISLIK, YUKSEKLIK, |app| {
            ornek_fatura_ekle(app);
            rapor_sekmesi(app, crate::report::ReportKind::SalesPurchase);
        });

        paneller_sigar(&panels, GENISLIK, YUKSEKLIK, "satış/alış özeti raporu");
    }

    #[test]
    fn rapor_kasa_banka_hareket_ozeti_dar_pencerede_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;

        let (panels, _directory) = run_layout_with(View::Reports, GENISLIK, YUKSEKLIK, |app| {
            let invoice_id = ornek_fatura_ekle(app);
            let hesap = ornek_kasa_ekle(app, "Merkez Kasa", "1.000,00");
            app.open_payment_form_for_invoice(invoice_id);
            app.submit_payment_form();
            app.confirm_payment_save();
            let gelir = ornek_gelir_gider_kategorisi(
                app,
                "Kira Geliri",
                crate::income_expense::EntryType::Gelir,
            );
            ornek_gelir_gider_ekle(
                app,
                crate::income_expense::EntryType::Gelir,
                gelir,
                hesap,
                "5.000,00",
            );
            rapor_sekmesi(app, crate::report::ReportKind::CashSummary);
        });

        paneller_sigar(
            &panels,
            GENISLIK,
            YUKSEKLIK,
            "kasa/banka hareket özeti raporu",
        );
    }

    #[test]
    fn rapor_gelir_gider_ozeti_dar_pencerede_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;

        let (panels, _directory) = run_layout_with(View::Reports, GENISLIK, YUKSEKLIK, |app| {
            let hesap = ornek_kasa_ekle(app, "Merkez Kasa", "0,00");
            let gelir = ornek_gelir_gider_kategorisi(
                app,
                "Çok Uzun Bir Kira Geliri Kategorisi Adı",
                crate::income_expense::EntryType::Gelir,
            );
            let gider = ornek_gelir_gider_kategorisi(
                app,
                "Çok Uzun Bir Kira Gideri Kategorisi Adı",
                crate::income_expense::EntryType::Gider,
            );
            ornek_gelir_gider_ekle(
                app,
                crate::income_expense::EntryType::Gelir,
                gelir,
                hesap,
                "5.000,00",
            );
            ornek_gelir_gider_ekle(
                app,
                crate::income_expense::EntryType::Gider,
                gider,
                hesap,
                "2.500,00",
            );
            rapor_sekmesi(app, crate::report::ReportKind::IncomeExpense);
        });

        paneller_sigar(&panels, GENISLIK, YUKSEKLIK, "gelir/gider özeti raporu");
    }

    // -----------------------------------------------------------------------
    // CSV ve yedekleme pencereleri
    // -----------------------------------------------------------------------

    fn ornek_cari_ice_aktarma_dosyasi(satir_sayisi: usize) -> (TempDir, std::path::PathBuf) {
        let directory = tempfile::tempdir().expect("geçici dizin");
        let path = directory.path().join("cari_ice.csv");
        let mut contents = String::from(
            "\u{feff}code;name;kind;phone;email;address;tax_office;tax_number;\
             national_id;contact_person;notes;active\r\n",
        );
        for index in 0..satir_sayisi {
            contents.push_str(&format!(
                "CAR-{index};Çok Uzun Bir Cari Unvanı {index};Musteri;0555 111 22 33;\
                 cari{index}@ornek.com;Uzun adres metni;Beşiktaş;1234567890;9876543210;\
                 Yetkili Kişi;{};Evet\r\n",
                "Uzun not metni ".repeat(6)
            ));
        }
        std::fs::write(&path, contents).expect("CSV yazılmalı");
        (directory, path)
    }

    #[test]
    fn csv_ice_aktarma_onizlemesi_dar_pencerede_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;

        let (panels, _directory) = run_layout_with(View::Customers, GENISLIK, YUKSEKLIK, |app| {
            let (_csv_directory, path) = ornek_cari_ice_aktarma_dosyasi(24);
            app.preview_customer_import(&path)
                .expect("önizleme hazırlanmalı");
        });

        paneller_sigar(&panels, GENISLIK, YUKSEKLIK, "CSV içe aktarma önizlemesi");
    }

    #[test]
    fn csv_ice_aktarma_sonucu_dar_pencerede_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;

        let (panels, _directory) = run_layout_with(View::Customers, GENISLIK, YUKSEKLIK, |app| {
            let (csv_directory, path) = ornek_cari_ice_aktarma_dosyasi(3);
            let mut contents = String::from(
                "\u{feff}code;name;kind;phone;email;address;tax_office;tax_number;\
                 national_id;contact_person;notes;active\r\n\
                 CAR-1;Hazır Cari;Musteri;;;;;;;;;;Evet\r\n\
                 ;Kodsuz Cari;Musteri;;;;;;;;;;Evet\r\n",
            );
            contents.push_str("CAR-2;İkinci Cari;Musteri;;;;;;;;;;Evet\r\n");
            std::fs::write(&path, contents).expect("CSV yazılmalı");
            app.preview_customer_import(&path).unwrap();
            app.confirm_import().unwrap();
            drop(csv_directory);
        });

        paneller_sigar(&panels, GENISLIK, YUKSEKLIK, "CSV içe aktarma sonucu");
    }

    #[test]
    fn yedek_basarili_bildirimi_dar_pencerede_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;

        let (panels, _directory) = run_layout_with(View::Overview, GENISLIK, YUKSEKLIK, |app| {
            let backup_directory = tempfile::tempdir().unwrap();
            app.export_backup_now(backup_directory.path().join("yedek.onm-backup"))
                .unwrap();
        });

        paneller_sigar(&panels, GENISLIK, YUKSEKLIK, "yedek başarı bildirimi");
    }

    #[test]
    fn yedek_geri_yukleme_onayi_dar_pencerede_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;

        let (panels, _directory) = run_layout_with(View::Overview, GENISLIK, YUKSEKLIK, |app| {
            let backup_directory = tempfile::tempdir().unwrap();
            let path = backup_directory.path().join("yedek.onm-backup");
            app.export_backup_now(&path).unwrap();
            app.prepare_restore(&path).unwrap();
        });

        paneller_sigar(&panels, GENISLIK, YUKSEKLIK, "yedek geri yükleme onayı");
    }

    #[test]
    fn gecersiz_yedek_bildirimi_dar_pencerede_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;

        let (panels, _directory) = run_layout_with(View::Overview, GENISLIK, YUKSEKLIK, |app| {
            let backup_directory = tempfile::tempdir().unwrap();
            let path = backup_directory.path().join("gecersiz.onm-backup");
            std::fs::write(&path, b"bu bir yedek degil").unwrap();
            let _ = app.prepare_restore(&path);
        });

        paneller_sigar(&panels, GENISLIK, YUKSEKLIK, "geçersiz yedek bildirimi");
    }

    #[test]
    fn yedekleme_bekleme_penceresi_dar_pencerede_tasmaz() {
        const GENISLIK: f32 = 900.0;
        const YUKSEKLIK: f32 = 600.0;

        let (panels, _directory) = run_layout_with(View::Overview, GENISLIK, YUKSEKLIK, |app| {
            app.transfer_state_mut().busy_message = Some("Tüm veri dışa aktarılıyor...".to_owned());
        });

        paneller_sigar(&panels, GENISLIK, YUKSEKLIK, "yedekleme bekleme penceresi");
    }
}
