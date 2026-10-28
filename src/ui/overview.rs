// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! "Genel Bakış" ekranı.
//!
//! Bu ekran uygulamanın çalıştığını, veritabanının hazır olduğunu ve hangi
//! bölümlerin henüz geliştirilmediğini gösterir. Gösterilen tüm tutarlar
//! **gerçek kayıtlardan** hesaplanır (kesinleşmiş faturaların genel toplamı);
//! örnek/sahte iş verisi üretilmez. Tahsilat/ödeme modülü bulunmadığı için
//! kalan bakiye veya nakit durumu gösterilmez.

use egui::{Button, Grid, Label, RichText, Sides};

use crate::APP_NAME;
use crate::APP_VERSION;
use crate::app::{OnMuhasebeApp, OverviewMetrics, View};
use crate::payment::{PaymentStatus, PaymentType};
use crate::ui::format::{format_bytes, format_date, format_date_short, format_iso_date_short};
use crate::ui::theme::{Palette, metrics};
use crate::ui::widgets;

/// Genel bakış ekranını çizer.
pub fn show(ui: &mut egui::Ui, app: &mut OnMuhasebeApp) {
    welcome(ui, app);
    ui.add_space(metrics::SECTION_GAP);

    if let Some(error) = app.startup_error() {
        widgets::error_card(ui, "Veritabanı açılamadı", error);
        ui.add_space(metrics::SECTION_GAP);
    }

    if let Some(error) = app.status_error() {
        widgets::error_card(ui, "Veritabanı durumu okunamadı", error);
        ui.add_space(metrics::SECTION_GAP);
    }

    let overview = app.overview_metrics();
    widgets::card(ui, "Özet", |ui| summary(ui, &overview, app));
    ui.add_space(metrics::SECTION_GAP);

    widgets::card(ui, "Raporlar", |ui| reports_shortcut(ui, app));
    ui.add_space(metrics::SECTION_GAP);

    widgets::card(ui, "Son faturalar", |ui| recent_invoices(ui, &overview));
    ui.add_space(metrics::SECTION_GAP);

    widgets::card(ui, "Son tahsilat/ödemeler", |ui| recent_payments(ui, app));
    ui.add_space(metrics::SECTION_GAP);

    widgets::card(ui, "Son stok hareketleri", |ui| {
        recent_movements(ui, &overview)
    });
    ui.add_space(metrics::SECTION_GAP);

    no_data_notice(ui);
    ui.add_space(metrics::SECTION_GAP);

    widgets::card(ui, "Modüller", |ui| shortcuts(ui, app));
    ui.add_space(metrics::SECTION_GAP);

    widgets::card(ui, "Veritabanı bağlantı durumu", |ui| {
        database_status(ui, app)
    });
    ui.add_space(metrics::SECTION_GAP);

    widgets::card(ui, "Program bilgisi", |ui| program_info(ui, app));
}

/// Gerçek kayıtlardan türetilen sayısal özet.
///
/// Burada **yalnızca gerçek kayıt sayıları** gösterilir; sahte finansal tutar
/// (satış, kâr, nakit, borç/alacak) üretilmez.
fn summary(ui: &mut egui::Ui, overview: &OverviewMetrics, app: &mut OnMuhasebeApp) {
    let totals = app.cash_totals();
    let account_count = app.cash_account_count();
    let (month_income_expense, has_income_expense) = app.month_income_expense();

    Grid::new("genel_bakis_ozet")
        .num_columns(2)
        .spacing([18.0, 8.0])
        .show(ui, |ui| {
            widgets::row_label(ui, "Aktif ürün");
            ui.label(format!("{} adet", overview.active_products));
            ui.end_row();

            widgets::row_label(ui, "Düşük stoklu ürün");
            ui.horizontal(|ui| {
                ui.label(format!("{} adet", overview.low_stock_products));
                if overview.low_stock_products > 0
                    && ui
                        .button("Listele")
                        .on_hover_text(
                            "Ürün ve Stok bölümünü \"yalnızca düşük stok\" filtresiyle açar.",
                        )
                        .clicked()
                {
                    app.show_low_stock_products();
                }
            });
            ui.end_row();

            widgets::row_label(ui, "Cari hesap");
            ui.label(format!("{} adet", overview.customers));
            ui.end_row();

            widgets::row_label(ui, "Kesinleşmiş satış");
            ui.label(
                RichText::new(overview.sales_total.format_with_symbol())
                    .strong()
                    .color(Palette::TEXT),
            );
            ui.end_row();

            widgets::row_label(ui, "Kesinleşmiş alış");
            ui.label(
                RichText::new(overview.purchase_total.format_with_symbol())
                    .strong()
                    .color(Palette::TEXT),
            );
            ui.end_row();

            if has_income_expense {
                widgets::row_label(ui, "Bu ay gelir");
                ui.label(
                    RichText::new(month_income_expense.income_total.format_with_symbol())
                        .strong()
                        .color(Palette::SUCCESS),
                );
                ui.end_row();

                widgets::row_label(ui, "Bu ay gider");
                ui.label(
                    RichText::new(month_income_expense.expense_total.format_with_symbol())
                        .strong()
                        .color(Palette::DANGER),
                );
                ui.end_row();

                widgets::row_label(ui, "Bu ay net");
                ui.label(
                    RichText::new(month_income_expense.net().format_with_symbol())
                        .size(15.0)
                        .strong()
                        .color(if month_income_expense.net().kurus() < 0 {
                            Palette::DANGER
                        } else {
                            Palette::TEXT
                        }),
                );
                ui.end_row();
            }

            if account_count > 0 {
                widgets::row_label(ui, "Toplam kasa");
                ui.label(
                    RichText::new(totals.cash.format_with_symbol())
                        .strong()
                        .color(Palette::TEXT),
                );
                ui.end_row();

                widgets::row_label(ui, "Toplam banka");
                ui.label(
                    RichText::new(totals.bank.format_with_symbol())
                        .strong()
                        .color(Palette::TEXT),
                );
                ui.end_row();

                widgets::row_label(ui, "Toplam nakit");
                ui.label(
                    RichText::new(totals.total().format_with_symbol())
                        .size(15.0)
                        .strong()
                        .color(Palette::TEXT),
                );
                ui.end_row();
            }
        });

    ui.add_space(8.0);
    ui.add(
        Label::new(
            RichText::new(
                "Satış, alış, tahsilat/ödeme ve gelir/gider tutarları gerçek \
                 kayıtlardan hesaplanır. Nakit toplamı = kasa + banka (aktif \
                 hesaplar). İptal edilen fatura, ödeme ve gelir/gider kayıtları \
                 katılmaz.",
            )
            .size(12.0)
            .color(Palette::MUTED),
        )
        .wrap(),
    );

    if !has_income_expense {
        ui.add_space(4.0);
        widgets::bullet(ui, "Bu ay henüz gelir/gider kaydı yok.");
    }

    if account_count == 0 {
        ui.add_space(4.0);
        widgets::bullet(ui, "Henüz kasa/banka hesabı yok.");
    }
}

/// Son faturaları (en yeni önce) listeler.
fn recent_invoices(ui: &mut egui::Ui, overview: &OverviewMetrics) {
    if overview.recent_invoices.is_empty() {
        widgets::bullet(ui, "Henüz fatura kaydı bulunmuyor.");
        return;
    }

    for invoice in &overview.recent_invoices {
        Sides::new().height(22.0).shrink_left().show(
            ui,
            |ui| {
                ui.label(
                    RichText::new(format_iso_date_short(&invoice.invoice_date))
                        .monospace()
                        .size(12.5)
                        .color(Palette::MUTED),
                );
                ui.label(
                    RichText::new(invoice.invoice_type.label())
                        .size(12.0)
                        .color(Palette::MUTED),
                );
                ui.add(
                    Label::new(
                        RichText::new(format!("{} — {}", invoice.number, invoice.customer_name))
                            .size(12.5)
                            .color(Palette::TEXT),
                    )
                    .truncate(),
                );
            },
            |ui| {
                let color = if invoice.is_cancelled() {
                    Palette::MUTED
                } else {
                    Palette::TEXT
                };
                ui.label(
                    RichText::new(invoice.totals.grand_total.format_with_symbol())
                        .size(12.5)
                        .strong()
                        .color(color),
                );
                if invoice.is_cancelled() {
                    ui.label(RichText::new("İptal").size(11.5).color(Palette::DANGER));
                }
            },
        );
    }
}

/// Son tahsilat/ödemeleri (en yeni önce) listeler.
fn recent_payments(ui: &mut egui::Ui, app: &OnMuhasebeApp) {
    let payments = app.recent_payments(5);
    if payments.is_empty() {
        widgets::bullet(ui, "Henüz tahsilat/ödeme kaydı yok.");
        return;
    }

    for payment in &payments {
        Sides::new().height(22.0).shrink_left().show(
            ui,
            |ui| {
                ui.label(
                    RichText::new(format_iso_date_short(&payment.payment_date))
                        .monospace()
                        .size(12.5)
                        .color(Palette::MUTED),
                );
                ui.label(
                    RichText::new(payment.payment_type.label())
                        .size(12.0)
                        .color(match payment.payment_type {
                            PaymentType::Tahsilat => Palette::SUCCESS,
                            PaymentType::Odeme => Palette::DANGER,
                        }),
                );
                ui.add(
                    Label::new(
                        RichText::new(format!(
                            "{} — {}",
                            payment.customer_code, payment.customer_name
                        ))
                        .size(12.5)
                        .color(Palette::TEXT),
                    )
                    .truncate(),
                );
            },
            |ui| {
                ui.label(
                    RichText::new(payment.amount.format_with_symbol())
                        .size(12.5)
                        .strong()
                        .color(if payment.status == PaymentStatus::Iptal {
                            Palette::MUTED
                        } else {
                            Palette::TEXT
                        }),
                );
                if payment.status == PaymentStatus::Iptal {
                    ui.label(RichText::new("İptal").size(11.5).color(Palette::DANGER));
                }
            },
        );
    }
}

/// Son stok hareketlerini (en yeni önce) listeler.
fn recent_movements(ui: &mut egui::Ui, overview: &OverviewMetrics) {
    if overview.recent_movements.is_empty() {
        widgets::bullet(ui, "Henüz stok hareketi kaydı bulunmuyor.");
        return;
    }

    for movement in &overview.recent_movements {
        Sides::new().height(22.0).shrink_left().show(
            ui,
            |ui| {
                ui.label(
                    RichText::new(movement_date(&movement.movement_date))
                        .monospace()
                        .size(12.5)
                        .color(Palette::MUTED),
                );
                ui.add(
                    Label::new(
                        RichText::new(&movement.product_name)
                            .size(12.5)
                            .color(Palette::TEXT),
                    )
                    .truncate(),
                );
                ui.label(
                    RichText::new(movement.movement_type.label())
                        .size(12.0)
                        .color(Palette::MUTED),
                );
            },
            |ui| {
                let positive = movement.quantity_delta.is_positive();
                ui.label(
                    RichText::new(movement.formatted_quantity())
                        .size(12.5)
                        .strong()
                        .color(if positive {
                            Palette::SUCCESS
                        } else {
                            Palette::DANGER
                        }),
                );
            },
        );
    }
}

/// `YYYY-MM-DD` tarihini kısa Türkçe biçimde gösterir.
fn movement_date(iso: &str) -> String {
    chrono::NaiveDate::parse_from_str(iso, "%Y-%m-%d")
        .map(format_date_short)
        .unwrap_or_else(|_| iso.to_owned())
}

/// Karşılama başlığı ve yerel sistem tarihi.
fn welcome(ui: &mut egui::Ui, app: &mut OnMuhasebeApp) {
    let today = format_date(chrono::Local::now().date_naive());
    let mut refresh = false;

    // `shrink_right`: sağdaki içerik kalan alanla sınırlanır, böylece dar
    // pencerelerde satır yana doğru taşmaz (gerekirse tarih kısaltılır).
    Sides::new().height(60.0).shrink_right().truncate().show(
        ui,
        |ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("Hoş geldiniz")
                        .size(23.0)
                        .strong()
                        .color(Palette::TEXT),
                );
                ui.label(
                    RichText::new("Windows için yerel masaüstü ön muhasebe uygulaması")
                        .size(13.0)
                        .color(Palette::MUTED),
                );
            });
        },
        |ui| {
            ui.label(
                RichText::new(format!("Bugün: {today}"))
                    .size(13.0)
                    .color(Palette::ACCENT),
            );
        },
    );

    ui.add_space(10.0);

    if ui.button("Durumu yenile").clicked() {
        refresh = true;
    }

    if refresh {
        app.refresh_status();
    }
}

/// Gerçek iş verisi bulunmadığını açıkça belirten bilgilendirme kutusu.
fn no_data_notice(ui: &mut egui::Ui) {
    widgets::notice_card(
        ui,
        "Kapsam",
        "Cari Hesap, Ürün ve Stok, Alış/Satış Faturası, Kasa/Banka \
         (tahsilat-ödeme), Gelir/Gider ve Raporlar modülleri kullanıma açıldı. \
         Dışa aktarma (CSV/Excel/PDF) sonraki aşamada geliştirilecektir. \
         Gösterilen tutarların tamamı gerçek kayıtlardan hesaplanır.",
    );
}

/// Raporlar bölümüne götüren kısayol kartı.
fn reports_shortcut(ui: &mut egui::Ui, app: &mut OnMuhasebeApp) {
    ui.add(
        Label::new(
            RichText::new(
                "Cari ekstre ve bakiye özeti, stok durumu, satış/alış, kasa/banka \
                 ve gelir/gider raporlarını tek ekranda görüntüleyin.",
            )
            .size(12.5)
            .color(Palette::MUTED),
        )
        .wrap(),
    );
    ui.add_space(8.0);
    if ui.button("Raporları aç").clicked() {
        app.set_view(View::Reports);
    }
}

/// Bölümlere götüren tıklanabilir kısayol kartları.
fn shortcuts(ui: &mut egui::Ui, app: &mut OnMuhasebeApp) {
    ui.add(
        Label::new(
            RichText::new("Bir bölüme gitmek için kartlardan birine tıklayın.")
                .small()
                .color(Palette::MUTED),
        )
        .wrap(),
    );
    ui.add_space(8.0);

    let mut selected = None;

    ui.horizontal_wrapped(|ui| {
        for view in View::ALL {
            if view == View::Overview {
                continue;
            }

            // Buton rengi temadan gelir (açık gri); üzerine gelindiğinde koyulaşır.
            // Böylece kısayollar beyaz kart üzerinde görünür kalır.
            let response = ui.add_sized(
                [metrics::SHORTCUT_WIDTH, metrics::SHORTCUT_HEIGHT],
                Button::new(RichText::new(view.title()).size(13.5).color(Palette::TEXT))
                    .stroke(egui::Stroke::new(1.0, Palette::BORDER))
                    .corner_radius(egui::CornerRadius::same(metrics::CORNER_RADIUS)),
            );

            if response.clicked() {
                selected = Some(view);
            }

            response.on_hover_text(view.description());
        }
    });

    if let Some(view) = selected {
        app.set_view(view);
    }
}

/// Veritabanı bağlantı durumunu satır satır gösterir.
fn database_status(ui: &mut egui::Ui, app: &OnMuhasebeApp) {
    let Some(status) = app.status() else {
        ui.colored_label(
            Palette::DANGER,
            "Veritabanı durumu okunamadı. Ayrıntı için yukarıdaki hata mesajına bakın.",
        );
        return;
    };

    let healthy = app.is_healthy();

    Grid::new("veritabani_durumu")
        .num_columns(2)
        .spacing([18.0, 8.0])
        .show(ui, |ui| {
            widgets::row_label(ui, "Bağlantı durumu");
            if healthy {
                ui.colored_label(Palette::SUCCESS, "● Bağlı ve çalışıyor");
            } else {
                ui.colored_label(Palette::WARNING, "● Sorunlu");
            }
            ui.end_row();

            widgets::row_label(ui, "Şema sürümü");
            ui.label(format!(
                "{} (desteklenen: {})",
                status.schema_version, status.supported_schema_version
            ));
            ui.end_row();

            widgets::row_label(ui, "Uygulanan migration");
            ui.label(format!("{} kayıt", status.applied_migrations.len()));
            ui.end_row();

            for migration in &status.applied_migrations {
                widgets::row_label(ui, "");
                ui.label(
                    RichText::new(format!(
                        "v{} – {} ({})",
                        migration.version, migration.name, migration.applied_at
                    ))
                    .small()
                    .color(Palette::MUTED),
                );
                ui.end_row();
            }

            widgets::row_label(ui, "Veritabanı tabloları");
            ui.label(format!("{} adet", status.user_table_count));
            ui.end_row();

            widgets::row_label(ui, "Dosya boyutu");
            match status.file_size_bytes {
                Some(bytes) => {
                    ui.label(format_bytes(bytes));
                }
                None => {
                    ui.label(RichText::new("bilinmiyor").color(Palette::MUTED));
                }
            }
            ui.end_row();

            widgets::row_label(ui, "İlk oluşturulma");
            match &status.created_at {
                Some(created_at) => {
                    ui.label(format!("{created_at} (UTC)"));
                }
                None => {
                    ui.label(RichText::new("bilinmiyor").color(Palette::MUTED));
                }
            }
            ui.end_row();
        });

    ui.add_space(8.0);
    widgets::subheading(ui, "Veritabanı dosyası");
    ui.add_space(2.0);
    match &status.path {
        Some(path) => widgets::path_label(ui, &path.display().to_string()),
        None => {
            ui.label(RichText::new("(bellek içi)").color(Palette::MUTED));
        }
    }

    ui.add_space(6.0);
    ui.add(
        Label::new(
            RichText::new(
                "Veritabanında uygulama meta verisi, cari hesaplar, ürünler, \
                 alış/satış faturaları, kasa/banka, tahsilat/ödeme ve \
                 gelir/gider tabloları bulunur. Raporlar bu tablolardan salt \
                 okunur biçimde türetilir.",
            )
            .small()
            .color(Palette::MUTED),
        )
        .wrap(),
    );
}

/// Program sürümü ve teknik bilgiler.
fn program_info(ui: &mut egui::Ui, app: &OnMuhasebeApp) {
    Grid::new("program_bilgisi")
        .num_columns(2)
        .spacing([18.0, 8.0])
        .show(ui, |ui| {
            widgets::row_label(ui, "Program");
            ui.label(APP_NAME);
            ui.end_row();

            widgets::row_label(ui, "Sürüm");
            ui.label(APP_VERSION);
            ui.end_row();

            widgets::row_label(ui, "Arayüz");
            ui.label("Rust + egui/eframe (yerel masaüstü)");
            ui.end_row();

            widgets::row_label(ui, "Veritabanı");
            ui.label("SQLite (harici sunucu gerektirmez)");
            ui.end_row();
        });

    ui.add_space(8.0);
    widgets::subheading(ui, "Veri dizini");
    ui.add_space(2.0);
    widgets::path_label(ui, &app.config().data_dir().display().to_string());
}
