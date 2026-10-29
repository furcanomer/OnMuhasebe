// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Henüz geliştirilmemiş bölümler için sade bilgilendirme ekranı.
//!
//! Cari hesap, ürün/stok ve alış/satış faturası bölümleri tamamlanmıştır; bu
//! ekran kullanıcı menüden diğer bölümlere tıkladığında sahte içerik yerine ne
//! olduğunu açıkça anlatmak için gösterilir.

use egui::{Label, RichText};

use crate::app::{OnMuhasebeApp, View};
use crate::ui::theme::{Palette, metrics};
use crate::ui::widgets;

/// Geliştirilmemiş bir bölüm için bilgilendirme ekranı çizer.
pub fn show(ui: &mut egui::Ui, app: &mut OnMuhasebeApp, view: View) {
    ui.label(
        RichText::new(view.title())
            .size(22.0)
            .strong()
            .color(Palette::TEXT),
    );
    ui.add_space(6.0);
    ui.add(
        Label::new(
            RichText::new(view.description())
                .size(13.5)
                .color(Palette::MUTED),
        )
        .wrap(),
    );
    ui.add_space(metrics::SECTION_GAP + 2.0);

    widgets::card(ui, "Bu bölüm henüz geliştirilmedi", |ui| {
        ui.add(
            Label::new(
                RichText::new(
                    "Bu bölümün ekranı ve veri tabloları henüz oluşturulmamıştır. \
                     Sahte veri göstermek yerine, bölüm gerçekten geliştirildiğinde \
                     burası çalışan ekranla değiştirilecektir.",
                )
                .color(Palette::TEXT),
            )
            .wrap(),
        );

        ui.add_space(10.0);
        widgets::subheading(ui, "Bu bölümde planlananlar");
        ui.add_space(4.0);

        for item in planned_items(view) {
            widgets::bullet(ui, item);
        }
    });

    ui.add_space(metrics::SECTION_GAP);

    if ui.button("Genel Bakış'a dön").clicked() {
        app.set_view(View::Overview);
    }
}

/// Bölümün geliştirildiğinde içereceği konular.
fn planned_items(view: View) -> &'static [&'static str] {
    match view {
        View::Overview => &["Genel durum özeti"],
        View::Customers => &[
            "Cari hareketler ve ekstre",
            "Borç / alacak bakiyesinin hesaplanması",
            "Cari bazlı raporlar",
        ],
        View::Inventory => &[
            "Ürün ve hizmet kartları",
            "Stok giriş / çıkış hareketleri",
            "Kritik stok uyarıları",
        ],
        View::PurchaseInvoices => &[
            "Tedarikçi faturaları",
            "Fatura kalemleri ve toplamlar",
            "Ödeme durumu takibi",
        ],
        View::SalesInvoices => &[
            "Müşteri faturaları",
            "Fatura kalemleri ve toplamlar",
            "Tahsilat durumu takibi",
        ],
        View::CashAndBank => &[
            "Kasa hesapları",
            "Banka hesapları",
            "Tahsilat ve ödeme hareketleri",
        ],
        View::IncomeExpense => &[
            "Gelir kayıtları",
            "Gider kayıtları ve kategorileri",
            "Dönemsel özetler",
        ],
        View::Reports => &[
            "Cari, stok, kasa ve satış raporları",
            "Tarih aralığına göre filtreleme",
            "Dışa aktarma (örneğin CSV)",
        ],
        View::Settings => &[
            "Şirket bilgileri",
            "Para birimi ve varsayılan değerler",
            "Yedekleme ve geri yükleme",
        ],
    }
}
