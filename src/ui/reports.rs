// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Rapor ekranları.
//!
//! Bu modül **yalnızca çizim yapar**. Veriyi
//! [`OnMuhasebeApp::reports_state`] üzerinden okur; filtre değişikliklerini
//! [`Action`] değerlerine dönüştürüp uygulama katmanına devreder. Hiçbir SQL
//! bu dosyada bulunmaz.
//!
//! ## Yerleşim
//!
//! Rapor sekmeleri ve filtre alanları `horizontal_wrapped` ile sarılır. Tablo
//! hücreleri sabit genişlikte ve uzun metinler kısaltılmıştır; böylece 900x600
//! gibi dar pencerelerde yatay taşma oluşmaz.

use egui::{Align, Button, ComboBox, Grid, Label, RichText, Sides, TextEdit, Ui};

use crate::app::{OnMuhasebeApp, ReportsState};
use crate::customer::CustomerType;
use crate::report::{
    CustomerBalanceReport, CustomerBalanceSortColumn, CustomerStatement, IncomeExpenseReport,
    ReportKind, SalesPurchaseBreakdown, SalesPurchaseGrouping, SalesPurchaseReport, StockReport,
    StockStatus,
};
use crate::ui::format::format_iso_date_short;
use crate::ui::theme::{Palette, metrics};
use crate::ui::{transfer, widgets};

/// Kullanıcı eylemi.
#[derive(Debug, Clone, Copy)]
enum Action {
    Tab(ReportKind),
    Apply,
    Reset,
    SelectCustomer(Option<i64>),
    Grouping(SalesPurchaseGrouping),
    SelectAccount(Option<i64>),
    Sort(CustomerBalanceSortColumn),
    ExportCsv,
}

impl Action {
    fn apply(self, app: &mut OnMuhasebeApp) {
        match self {
            Self::Tab(tab) => app.set_report_tab(tab),
            Self::Apply => app.apply_report_filters(),
            Self::Reset => app.reset_reports_filters(),
            Self::SelectCustomer(customer_id) => app.set_report_customer(customer_id),
            Self::Grouping(grouping) => app.set_sales_grouping(grouping),
            Self::SelectAccount(account_id) => app.set_cash_report_account(account_id),
            Self::Sort(column) => app.toggle_customer_balance_sort(column),
            Self::ExportCsv => {
                if let Some(path) = transfer::save_csv_dialog(&app.report_csv_default_name()) {
                    let path = OnMuhasebeApp::csv_path(path);
                    let _ = app.export_report_csv(path);
                }
            }
        }
    }
}

/// Raporlar ekranını çizer.
pub fn show(ui: &mut Ui, app: &mut OnMuhasebeApp) {
    let mut action = None;

    page_header(ui, app.reports_state().tab);
    ui.add_space(metrics::SECTION_GAP);

    tabs(ui, app.reports_state(), &mut action);
    ui.add_space(metrics::SECTION_GAP);

    if let Some(error) = app.reports_state().error.as_deref() {
        widgets::error_card(ui, "Rapor oluşturulamadı", error);
        ui.add_space(metrics::SECTION_GAP);
    }

    filter_bar(ui, app, &mut action);
    ui.add_space(metrics::SECTION_GAP);

    match app.reports_state().tab {
        ReportKind::CustomerStatement => {
            customer_statement_body(ui, app.reports_state());
        }
        ReportKind::CustomerBalances => {
            customer_balance_body(ui, app.reports_state(), &mut action);
        }
        ReportKind::StockStatus => stock_body(ui, app.reports_state()),
        ReportKind::SalesPurchase => sales_body(ui, app.reports_state()),
        ReportKind::CashSummary => cash_body(ui, app.reports_state()),
        ReportKind::IncomeExpense => income_body(ui, app.reports_state()),
    }

    if let Some(action) = action {
        action.apply(app);
    }
}

/// Sayfa başlığı ve salt okunurluk bilgisi.
fn page_header(ui: &mut Ui, tab: ReportKind) {
    Sides::new().height(56.0).shrink_right().truncate().show(
        ui,
        |ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("Raporlar")
                        .size(22.0)
                        .strong()
                        .color(Palette::TEXT),
                );
                ui.add(
                    Label::new(
                        RichText::new(tab.description())
                            .size(13.0)
                            .color(Palette::MUTED),
                    )
                    .wrap(),
                );
            });
        },
        |ui| {
            ui.label(
                RichText::new("Salt okunur")
                    .size(12.0)
                    .color(Palette::ACCENT),
            );
        },
    );
}

/// Rapor sekmeleri.
fn tabs(ui: &mut Ui, state: &ReportsState, action: &mut Option<Action>) {
    ui.horizontal_wrapped(|ui| {
        for tab in ReportKind::ALL {
            if ui
                .selectable_label(state.tab == tab, RichText::new(tab.label()).size(12.5))
                .clicked()
                && state.tab != tab
            {
                *action = Some(Action::Tab(tab));
            }
        }
    });
    ui.add_space(4.0);
    ui.add(
        Label::new(
            RichText::new(
                "Raporlar salt okunurdur. İptal edilen fatura, tahsilat/ödeme ve \
                 gelir/gider kayıtları hiçbir rapora katılmaz.",
            )
            .size(11.5)
            .color(Palette::MUTED),
        )
        .wrap(),
    );
}

/// Aktif raporun filtre çubuğu.
fn filter_bar(ui: &mut Ui, app: &mut OnMuhasebeApp, action: &mut Option<Action>) {
    let tab = app.reports_state().tab;
    widgets::card(ui, "Filtreler", |ui| {
        let state = app.reports_state_mut();
        ui.horizontal_wrapped(|ui| {
            if tab.uses_date_range() {
                date_fields(ui, state, tab);
            }
            if ui.button("Uygula").clicked() {
                *action = Some(Action::Apply);
            }
            if ui.button("Sıfırla").clicked() {
                *action = Some(Action::Reset);
            }
            if ui.button("CSV dışa aktar").clicked() {
                *action = Some(Action::ExportCsv);
            }
        });

        match tab {
            ReportKind::CustomerStatement => customer_statement_filters(ui, state, action),
            ReportKind::CustomerBalances => customer_balance_filters(ui, state),
            ReportKind::StockStatus => stock_filters(ui, state),
            ReportKind::SalesPurchase => sales_filters(ui, state, action),
            ReportKind::CashSummary => cash_filters(ui, state, action),
            ReportKind::IncomeExpense => {}
        }
    });
}

/// Tarih alanları.
fn date_fields(ui: &mut Ui, state: &mut ReportsState, tab: ReportKind) {
    let optional = matches!(
        tab,
        ReportKind::CustomerStatement | ReportKind::CustomerBalances
    );
    let suffix = if optional { " (isteğe bağlı)" } else { "" };
    ui.label(RichText::new(format!("Başlangıç{suffix}:")).color(Palette::MUTED));
    ui.add(
        TextEdit::singleline(&mut state.date_from)
            .hint_text("YYYY-MM-DD")
            .desired_width(105.0),
    );
    ui.label(RichText::new(format!("Bitiş{suffix}:")).color(Palette::MUTED));
    ui.add(
        TextEdit::singleline(&mut state.date_to)
            .hint_text("YYYY-MM-DD")
            .desired_width(105.0),
    );
}

/// Cari ekstre filtreleri.
fn customer_statement_filters(ui: &mut Ui, state: &mut ReportsState, action: &mut Option<Action>) {
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Cari:").color(Palette::MUTED));
        let selected_text = state.selected_customer_text();
        let options = state.customer_options();
        if let Some(id) =
            widgets::searchable_select(ui, "rapor_cari_secimi", &selected_text, &options, 300.0)
        {
            *action = Some(Action::SelectCustomer(Some(id)));
        }
        if state.customer_id.is_some() && ui.small_button("Temizle").clicked() {
            *action = Some(Action::SelectCustomer(None));
        }
        if options.is_empty() {
            ui.label(
                RichText::new("Kayıtlı cari hesap yok.")
                    .size(11.5)
                    .color(Palette::MUTED),
            );
        }
    });
}

/// Cari bakiye filtreleri.
fn customer_balance_filters(ui: &mut Ui, state: &mut ReportsState) {
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Ara:").color(Palette::MUTED));
        ui.add(
            TextEdit::singleline(&mut state.customer_balance_filter.search)
                .hint_text("Kod veya ad")
                .desired_width(170.0),
        );

        ui.label(RichText::new("Tür:").color(Palette::MUTED));
        ComboBox::from_id_salt("rapor_cari_turu")
            .selected_text(
                state
                    .customer_balance_filter
                    .kind
                    .map(CustomerType::label)
                    .unwrap_or("Tümü"),
            )
            .width(120.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut state.customer_balance_filter.kind, None, "Tümü");
                for kind in CustomerType::ALL {
                    ui.selectable_value(
                        &mut state.customer_balance_filter.kind,
                        Some(kind),
                        kind.label(),
                    );
                }
            });

        ui.label(RichText::new("Durum:").color(Palette::MUTED));
        ComboBox::from_id_salt("rapor_cari_durumu")
            .selected_text(active_label(state.customer_balance_filter.active))
            .width(90.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut state.customer_balance_filter.active, None, "Tümü");
                ui.selectable_value(
                    &mut state.customer_balance_filter.active,
                    Some(true),
                    "Aktif",
                );
                ui.selectable_value(
                    &mut state.customer_balance_filter.active,
                    Some(false),
                    "Pasif",
                );
            });

        ui.checkbox(
            &mut state.customer_balance_filter.only_nonzero,
            "Yalnızca sıfır olmayan bakiyeler",
        );
    });
}

/// Stok filtreleri.
fn stock_filters(ui: &mut Ui, state: &mut ReportsState) {
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Ara:").color(Palette::MUTED));
        ui.add(
            TextEdit::singleline(&mut state.stock_filter.search)
                .hint_text("Kod veya ad")
                .desired_width(180.0),
        );

        ui.label(RichText::new("Durum:").color(Palette::MUTED));
        ComboBox::from_id_salt("rapor_stok_durumu")
            .selected_text(
                state
                    .stock_filter
                    .status
                    .map(StockStatus::label)
                    .unwrap_or("Tümü"),
            )
            .width(100.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut state.stock_filter.status, None, "Tümü");
                for status in StockStatus::ALL {
                    ui.selectable_value(
                        &mut state.stock_filter.status,
                        Some(status),
                        status.label(),
                    );
                }
            });

        ui.checkbox(
            &mut state.stock_filter.only_active,
            "Yalnızca aktif ürünler",
        );
    });
}

/// Satış/alış gruplama filtreleri.
fn sales_filters(ui: &mut Ui, state: &mut ReportsState, action: &mut Option<Action>) {
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Gruplama:").color(Palette::MUTED));
        for grouping in SalesPurchaseGrouping::ALL {
            if ui
                .selectable_label(
                    state.sales_grouping == grouping,
                    RichText::new(grouping.label()).size(12.5),
                )
                .clicked()
                && state.sales_grouping != grouping
            {
                *action = Some(Action::Grouping(grouping));
            }
        }
        ui.label(
            RichText::new("Yalnızca kesinleşmiş faturalar; iptaller hariç.")
                .size(11.5)
                .color(Palette::MUTED),
        );
    });
}

/// Kasa/banka hesap filtresi.
fn cash_filters(ui: &mut Ui, state: &mut ReportsState, action: &mut Option<Action>) {
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Hesap:").color(Palette::MUTED));
        let current = state.cash_account_id;
        let mut selected = current;
        ComboBox::from_id_salt("rapor_kasa_hesabi")
            .selected_text(state.selected_account_text())
            .width(260.0)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(current.is_none(), "Tüm hesaplar")
                    .clicked()
                {
                    selected = None;
                }
                for account in &state.accounts {
                    let label = format!("{} · {}", account.name, account.account_type.label());
                    if ui
                        .selectable_label(current == Some(account.id), label)
                        .clicked()
                    {
                        selected = Some(account.id);
                    }
                }
            });
        if selected != current {
            *action = Some(Action::SelectAccount(selected));
        }
        if state.accounts.is_empty() {
            ui.label(
                RichText::new("Kayıtlı kasa/banka hesabı yok.")
                    .size(11.5)
                    .color(Palette::MUTED),
            );
        }
        ui.label(
            RichText::new("Açılış, dönem başındaki kesin hareketlerden hesaplanır.")
                .size(11.5)
                .color(Palette::MUTED),
        );
    });
}

/// Cari ekstre gövdesi.
fn customer_statement_body(ui: &mut Ui, state: &ReportsState) {
    let Some(statement) = state.customer_statement.as_ref() else {
        widgets::card(ui, "Cari seçilmedi", |ui| {
            ui.add(
                Label::new(
                    RichText::new("Ekstre için bir cari hesap seçin.").color(Palette::MUTED),
                )
                .wrap(),
            );
        });
        return;
    };

    statement_header(ui, statement);
    ui.add_space(metrics::SECTION_GAP);

    if statement.rows.is_empty() {
        widgets::card(ui, "Hareket yok", |ui| {
            ui.add(
                Label::new(
                    RichText::new("Seçili dönemde bu cari için hareket bulunmuyor.")
                        .color(Palette::MUTED),
                )
                .wrap(),
            );
        });
    } else {
        widgets::card(ui, "Hareketler", |ui| {
            Grid::new("rapor_cari_ekstre")
                .spacing([4.0, 4.0])
                .show(ui, |ui| {
                    header_cell(ui, "Tarih", 60.0, false);
                    header_cell(ui, "Tür", 68.0, false);
                    header_cell(ui, "Belge No", 72.0, false);
                    header_cell(ui, "Açıklama", 128.0, false);
                    header_cell(ui, "Borç", 70.0, true);
                    header_cell(ui, "Alacak", 70.0, true);
                    header_cell(ui, "Bakiye", 74.0, true);
                    ui.end_row();

                    for row in &statement.rows {
                        cell(
                            ui,
                            &format_iso_date_short(&row.date),
                            60.0,
                            false,
                            Palette::MUTED,
                            false,
                        );
                        cell(
                            ui,
                            row.kind.label(),
                            68.0,
                            false,
                            if row.kind.is_debit() {
                                Palette::TEXT
                            } else {
                                Palette::ACCENT
                            },
                            false,
                        );
                        cell(
                            ui,
                            row.document_no.as_deref().unwrap_or("—"),
                            72.0,
                            false,
                            Palette::MUTED,
                            false,
                        );
                        cell(
                            ui,
                            row.description.as_deref().unwrap_or("—"),
                            128.0,
                            false,
                            Palette::MUTED,
                            false,
                        );
                        money_cell(ui, row.debit, 70.0, Palette::TEXT);
                        money_cell(ui, row.credit, 70.0, Palette::TEXT);
                        money_cell(ui, row.balance, 74.0, Palette::TEXT);
                        ui.end_row();
                    }
                });
        });
    }

    ui.add_space(metrics::SECTION_GAP);
    widgets::card(ui, "Alt toplam", |ui| {
        Grid::new("rapor_cari_ekstre_toplam")
            .num_columns(2)
            .spacing([18.0, 6.0])
            .show(ui, |ui| {
                totals_row(ui, "Dönem başı bakiye", statement.opening_balance);
                totals_row(ui, "Dönem borç toplamı", statement.total_debit);
                totals_row(ui, "Dönem alacak toplamı", statement.total_credit);
                totals_row(ui, "Dönem sonu bakiye", statement.closing_balance);
            });
    });

    if statement.rows.is_empty() && !statement.opening_balance.is_zero() {
        ui.add_space(6.0);
        ui.add(
            Label::new(
                RichText::new(
                    "Dönem içinde hareket yok; dönem sonu bakiye dönem başı bakiyesine eşittir.",
                )
                .size(11.5)
                .color(Palette::MUTED),
            )
            .wrap(),
        );
    }
}

/// Cari ekstre başlık kartı.
fn statement_header(ui: &mut Ui, statement: &CustomerStatement) {
    widgets::card(ui, "Cari hesap", |ui| {
        Grid::new("rapor_cari_ekstre_baslik")
            .num_columns(2)
            .spacing([18.0, 6.0])
            .show(ui, |ui| {
                widgets::row_label(ui, "Kod");
                ui.label(RichText::new(&statement.customer_code).monospace());
                ui.end_row();
                widgets::row_label(ui, "Ad");
                ui.label(&statement.customer_name);
                ui.end_row();
                widgets::row_label(ui, "Dönem başı");
                ui.label(
                    RichText::new(statement.opening_balance.format_with_symbol())
                        .strong()
                        .color(Palette::TEXT),
                );
                ui.end_row();
            });
    });
}

/// Cari bakiye özeti gövdesi.
fn customer_balance_body(ui: &mut Ui, state: &ReportsState, action: &mut Option<Action>) {
    let report = &state.customer_balance_report;
    widgets::card(ui, "Cari bakiyeleri", |ui| {
        if report.rows.is_empty() {
            ui.add(
                Label::new(
                    RichText::new("Filtrelere uygun cari hesap bulunmuyor.").color(Palette::MUTED),
                )
                .wrap(),
            );
            return;
        }

        Grid::new("rapor_cari_bakiye")
            .spacing([3.0, 4.0])
            .show(ui, |ui| {
                sort_header(
                    ui,
                    "Kod",
                    44.0,
                    CustomerBalanceSortColumn::Code,
                    state,
                    action,
                );
                sort_header(
                    ui,
                    "Cari Adı",
                    82.0,
                    CustomerBalanceSortColumn::Name,
                    state,
                    action,
                );
                sort_header(
                    ui,
                    "Tür",
                    58.0,
                    CustomerBalanceSortColumn::Kind,
                    state,
                    action,
                );
                sort_header(
                    ui,
                    "Satış",
                    64.0,
                    CustomerBalanceSortColumn::Sales,
                    state,
                    action,
                );
                sort_header(
                    ui,
                    "Alış",
                    64.0,
                    CustomerBalanceSortColumn::Purchases,
                    state,
                    action,
                );
                sort_header(
                    ui,
                    "Tahsilat",
                    64.0,
                    CustomerBalanceSortColumn::Collections,
                    state,
                    action,
                );
                sort_header(
                    ui,
                    "Ödeme",
                    64.0,
                    CustomerBalanceSortColumn::Payments,
                    state,
                    action,
                );
                sort_header(
                    ui,
                    "Bakiye",
                    70.0,
                    CustomerBalanceSortColumn::Balance,
                    state,
                    action,
                );
                header_cell(ui, "Durum", 56.0, false);
                ui.end_row();

                for row in &report.rows {
                    cell(ui, &row.code, 44.0, false, Palette::MUTED, true);
                    cell(ui, &row.name, 82.0, false, Palette::TEXT, false);
                    cell(ui, row.kind.label(), 58.0, false, Palette::MUTED, false);
                    money_cell(ui, row.sales_total, 64.0, Palette::TEXT);
                    money_cell(ui, row.purchase_total, 64.0, Palette::TEXT);
                    money_cell(ui, row.collection_total, 64.0, Palette::TEXT);
                    money_cell(ui, row.payment_total, 64.0, Palette::TEXT);
                    money_cell(
                        ui,
                        row.balance,
                        70.0,
                        if row.balance.kurus() < 0 {
                            Palette::DANGER
                        } else {
                            Palette::TEXT
                        },
                    );
                    cell(
                        ui,
                        row.status().label(),
                        56.0,
                        false,
                        match row.status() {
                            crate::report::CustomerBalanceStatus::Borclu => Palette::TEXT,
                            crate::report::CustomerBalanceStatus::Alacakli => Palette::DANGER,
                            crate::report::CustomerBalanceStatus::Kapali => Palette::MUTED,
                        },
                        false,
                    );
                    ui.end_row();
                }
            });
    });

    ui.add_space(metrics::SECTION_GAP);
    balance_footer(ui, report);
}

/// Cari bakiye alt toplamları.
fn balance_footer(ui: &mut Ui, report: &CustomerBalanceReport) {
    widgets::card(ui, "Alt toplam", |ui| {
        Grid::new("rapor_cari_bakiye_toplam")
            .num_columns(2)
            .spacing([18.0, 6.0])
            .show(ui, |ui| {
                totals_row(
                    ui,
                    "Alacaklı cariler (bize borçlu)",
                    report.total_receivable,
                );
                totals_row(ui, "Borçlu cariler (bizim borcumuz)", report.total_payable);
                totals_row(ui, "Net bakiye", report.net_balance);
                widgets::row_label(ui, "Cari sayısı");
                ui.label(format!("{} adet", report.rows.len()));
                ui.end_row();
            });
    });
}

/// Stok durumu gövdesi.
fn stock_body(ui: &mut Ui, state: &ReportsState) {
    let report = &state.stock_report;
    widgets::card(ui, "Ürün stokları", |ui| {
        if report.rows.is_empty() {
            ui.add(
                Label::new(
                    RichText::new("Filtrelere uygun ürün bulunmuyor.").color(Palette::MUTED),
                )
                .wrap(),
            );
            return;
        }

        Grid::new("rapor_stok_durumu")
            .spacing([5.0, 4.0])
            .show(ui, |ui| {
                header_cell(ui, "Kod", 65.0, false);
                header_cell(ui, "Ürün Adı", 160.0, false);
                header_cell(ui, "Birim", 50.0, false);
                header_cell(ui, "Mevcut Stok", 82.0, true);
                header_cell(ui, "Min. Stok", 72.0, true);
                header_cell(ui, "Durum", 65.0, false);
                ui.end_row();

                for row in &report.rows {
                    cell(ui, &row.code, 65.0, false, Palette::MUTED, true);
                    cell(ui, &row.name, 160.0, false, Palette::TEXT, false);
                    cell(
                        ui,
                        row.unit.short_label(),
                        50.0,
                        false,
                        Palette::MUTED,
                        false,
                    );
                    cell(
                        ui,
                        &row.current_stock.format_turkish(),
                        82.0,
                        true,
                        status_color(row.status),
                        true,
                    );
                    cell(
                        ui,
                        &row.min_stock.format_turkish(),
                        72.0,
                        true,
                        Palette::MUTED,
                        false,
                    );
                    cell(
                        ui,
                        row.status.label(),
                        65.0,
                        false,
                        status_color(row.status),
                        true,
                    );
                    ui.end_row();
                }
            });
    });

    ui.add_space(metrics::SECTION_GAP);
    stock_footer(ui, report);
}

/// Stok alt bilgi sayıları.
fn stock_footer(ui: &mut Ui, report: &StockReport) {
    widgets::card(ui, "Alt toplam", |ui| {
        Grid::new("rapor_stok_toplam")
            .num_columns(2)
            .spacing([18.0, 6.0])
            .show(ui, |ui| {
                widgets::row_label(ui, "Toplam aktif ürün");
                ui.label(format!("{} adet", report.stats.active_products));
                ui.end_row();
                widgets::row_label(ui, "Düşük stoklu");
                ui.label(format!("{} adet", report.stats.low_stock));
                ui.end_row();
                widgets::row_label(ui, "Sıfır stoklu");
                ui.label(format!("{} adet", report.stats.zero_stock));
                ui.end_row();
                widgets::row_label(ui, "Eksi stoklu");
                ui.label(format!("{} adet", report.stats.negative_stock));
                ui.end_row();
                totals_row(ui, "Toplam stok değeri (alış)", report.stats.purchase_value);
                totals_row(ui, "Tahmini stok değeri (satış)", report.stats.sale_value);
            });
    });
    ui.add_space(6.0);
    ui.add(
        Label::new(
            RichText::new("Stok değerleri yalnızca bilgi amaçlıdır; maliyet hesabı değildir.")
                .size(11.5)
                .color(Palette::MUTED),
        )
        .wrap(),
    );
}

/// Satış/alış özeti gövdesi.
fn sales_body(ui: &mut Ui, state: &ReportsState) {
    let report = &state.sales_report;
    if report.invoice_count() == 0 && report.breakdown.is_empty() {
        widgets::card(ui, "Kayıt yok", |ui| {
            ui.add(
                Label::new(
                    RichText::new(
                        "Seçili dönemde kesinleşmiş satış/alış faturası bulunmuyor \
                         (iptaller hariç).",
                    )
                    .color(Palette::MUTED),
                )
                .wrap(),
            );
        });
        return;
    }

    sales_cards(ui, report);
    ui.add_space(metrics::SECTION_GAP);

    widgets::card(ui, "Grup dökümü", |ui| match &report.breakdown {
        SalesPurchaseBreakdown::Customer(rows) => {
            if rows.is_empty() {
                empty_label(ui, "Grup dökümünde cari bulunmuyor.");
                return;
            }
            Grid::new("rapor_satis_cari")
                .spacing([5.0, 4.0])
                .show(ui, |ui| {
                    header_cell(ui, "Kod", 62.0, false);
                    header_cell(ui, "Cari Adı", 145.0, false);
                    header_cell(ui, "Satış Fatura", 64.0, true);
                    header_cell(ui, "Alış Fatura", 64.0, true);
                    header_cell(ui, "Satış", 78.0, true);
                    header_cell(ui, "Alış", 78.0, true);
                    ui.end_row();
                    for row in rows {
                        cell(ui, &row.code, 62.0, false, Palette::MUTED, true);
                        cell(ui, &row.name, 145.0, false, Palette::TEXT, false);
                        cell(
                            ui,
                            &row.sales_count.to_string(),
                            64.0,
                            true,
                            Palette::TEXT,
                            false,
                        );
                        cell(
                            ui,
                            &row.purchase_count.to_string(),
                            64.0,
                            true,
                            Palette::TEXT,
                            false,
                        );
                        money_cell(ui, row.sales_total, 78.0, Palette::TEXT);
                        money_cell(ui, row.purchase_total, 78.0, Palette::TEXT);
                        ui.end_row();
                    }
                });
        }
        SalesPurchaseBreakdown::Product(rows) => {
            if rows.is_empty() {
                empty_label(ui, "Grup dökümünde ürün bulunmuyor.");
                return;
            }
            Grid::new("rapor_satis_urun")
                .spacing([4.0, 4.0])
                .show(ui, |ui| {
                    header_cell(ui, "Kod", 62.0, false);
                    header_cell(ui, "Ürün Adı", 125.0, false);
                    header_cell(ui, "Birim", 42.0, false);
                    header_cell(ui, "Satış Miktarı", 68.0, true);
                    header_cell(ui, "Satış Tutarı", 72.0, true);
                    header_cell(ui, "Alış Miktarı", 68.0, true);
                    header_cell(ui, "Alış Tutarı", 72.0, true);
                    ui.end_row();
                    for row in rows {
                        cell(ui, &row.code, 62.0, false, Palette::MUTED, true);
                        cell(ui, &row.name, 125.0, false, Palette::TEXT, false);
                        cell(
                            ui,
                            row.unit.short_label(),
                            42.0,
                            false,
                            Palette::MUTED,
                            false,
                        );
                        cell(
                            ui,
                            &row.sold_quantity.format_with_label(row.unit.short_label()),
                            68.0,
                            true,
                            Palette::TEXT,
                            false,
                        );
                        money_cell(ui, row.sales_total, 72.0, Palette::TEXT);
                        cell(
                            ui,
                            &row.purchased_quantity
                                .format_with_label(row.unit.short_label()),
                            68.0,
                            true,
                            Palette::TEXT,
                            false,
                        );
                        money_cell(ui, row.purchase_total, 72.0, Palette::TEXT);
                        ui.end_row();
                    }
                });
        }
    });

    ui.add_space(metrics::SECTION_GAP);
    widgets::card(ui, "Alt toplam", |ui| {
        Grid::new("rapor_satis_toplam")
            .num_columns(2)
            .spacing([18.0, 6.0])
            .show(ui, |ui| {
                widgets::row_label(ui, "Kesin satış faturası");
                ui.label(format!("{} adet", report.sales_count));
                ui.end_row();
                widgets::row_label(ui, "Kesin alış faturası");
                ui.label(format!("{} adet", report.purchase_count));
                ui.end_row();
                widgets::row_label(ui, "Toplam fatura");
                ui.label(format!("{} adet", report.invoice_count()));
                ui.end_row();
                totals_row(ui, "Satış toplamı", report.sales_total);
                totals_row(ui, "Alış toplamı", report.purchase_total);
                totals_row(ui, "Net (satış - alış)", report.net());
            });
    });
}

/// Satış/alış özet kartları.
fn sales_cards(ui: &mut Ui, report: &SalesPurchaseReport) {
    widgets::card(ui, "Dönem toplamları", |ui| {
        Grid::new("rapor_satis_kartlari")
            .num_columns(2)
            .spacing([18.0, 6.0])
            .show(ui, |ui| {
                widgets::row_label(ui, "Kesin satış faturası");
                ui.label(
                    RichText::new(format!("{} adet", report.sales_count))
                        .strong()
                        .color(Palette::TEXT),
                );
                ui.end_row();
                widgets::row_label(ui, "Satış tutarı");
                ui.label(
                    RichText::new(report.sales_total.format_with_symbol())
                        .strong()
                        .color(Palette::SUCCESS),
                );
                ui.end_row();
                widgets::row_label(ui, "Kesin alış faturası");
                ui.label(
                    RichText::new(format!("{} adet", report.purchase_count))
                        .strong()
                        .color(Palette::TEXT),
                );
                ui.end_row();
                widgets::row_label(ui, "Alış tutarı");
                ui.label(
                    RichText::new(report.purchase_total.format_with_symbol())
                        .strong()
                        .color(Palette::DANGER),
                );
                ui.end_row();
                widgets::row_label(ui, "Net (satış - alış)");
                ui.label(
                    RichText::new(report.net().format_with_symbol())
                        .size(15.0)
                        .strong()
                        .color(if report.net().kurus() < 0 {
                            Palette::DANGER
                        } else {
                            Palette::TEXT
                        }),
                );
                ui.end_row();
            });
    });
}

/// Kasa/banka hareket özeti gövdesi.
fn cash_body(ui: &mut Ui, state: &ReportsState) {
    let Some(report) = state.cash_report.as_ref() else {
        widgets::card(ui, "Rapor yok", |ui| {
            empty_label(ui, "Kasa/banka hareket raporu için hesap bulunmuyor.");
        });
        return;
    };

    if report.accounts.is_empty() {
        widgets::card(ui, "Hesap yok", |ui| {
            empty_label(ui, "Kayıtlı kasa/banka hesabı bulunmuyor.");
        });
        return;
    }

    cash_cards(ui, report);
    ui.add_space(metrics::SECTION_GAP);
    cash_accounts(ui, report);
    ui.add_space(metrics::SECTION_GAP);
    cash_movements(ui, report);
    ui.add_space(metrics::SECTION_GAP);
    cash_footer(ui, report);
}

/// Kasa/banka özet kartları.
fn cash_cards(ui: &mut Ui, report: &crate::report::CashMovementReport) {
    widgets::card(ui, "Dönem toplamları", |ui| {
        Grid::new("rapor_kasa_kartlari")
            .num_columns(2)
            .spacing([18.0, 6.0])
            .show(ui, |ui| {
                totals_row(ui, "Dönem başı bakiye", report.opening_balance);
                totals_row(ui, "Tahsilat toplamı", report.collections);
                totals_row(ui, "Ödeme toplamı", report.payments);
                totals_row(ui, "Gelir toplamı", report.incomes);
                totals_row(ui, "Gider toplamı", report.expenses);
                totals_row(ui, "Dönem sonu bakiye", report.closing_balance);
            });
    });
}

/// Hesap bazlı döküm.
fn cash_accounts(ui: &mut Ui, report: &crate::report::CashMovementReport) {
    widgets::card(ui, "Hesap bazlı döküm", |ui| {
        Grid::new("rapor_kasa_hesaplari")
            .spacing([4.0, 4.0])
            .show(ui, |ui| {
                header_cell(ui, "Hesap", 120.0, false);
                header_cell(ui, "Açılış", 72.0, true);
                header_cell(ui, "Tahsilat", 72.0, true);
                header_cell(ui, "Ödeme", 72.0, true);
                header_cell(ui, "Gelir", 72.0, true);
                header_cell(ui, "Gider", 72.0, true);
                header_cell(ui, "Kapanış", 76.0, true);
                ui.end_row();
                for row in &report.accounts {
                    cell(ui, &row.account_name, 120.0, false, Palette::TEXT, true);
                    money_cell(ui, row.opening_balance, 72.0, Palette::TEXT);
                    money_cell(ui, row.collections, 72.0, Palette::SUCCESS);
                    money_cell(ui, row.payments, 72.0, Palette::DANGER);
                    money_cell(ui, row.incomes, 72.0, Palette::SUCCESS);
                    money_cell(ui, row.expenses, 72.0, Palette::DANGER);
                    money_cell(ui, row.closing_balance, 76.0, Palette::TEXT);
                    ui.end_row();
                }
            });
    });
}

/// Birleşik kasa/banka hareket listesi.
fn cash_movements(ui: &mut Ui, report: &crate::report::CashMovementReport) {
    widgets::card(ui, "Hareketler", |ui| {
        if !report.has_movements() {
            empty_label(ui, "Seçili dönemde kasa/banka hareketi bulunmuyor.");
            return;
        }
        for movement in &report.movements {
            cash_movement_row(ui, movement);
            ui.add_space(4.0);
        }
    });
}

/// Tek birleşik kasa/banka hareketi.
fn cash_movement_row(ui: &mut Ui, movement: &crate::report::CashReportMovement) {
    let inflow = matches!(
        movement.kind,
        crate::report::CashMovementKind::Collection | crate::report::CashMovementKind::Income
    );
    let color = if inflow {
        Palette::SUCCESS
    } else {
        Palette::DANGER
    };

    widgets::surface_tinted(ui, Palette::SURFACE, Palette::BORDER, |ui| {
        Sides::new().height(24.0).shrink_left().show(
            ui,
            |ui| {
                ui.label(
                    RichText::new(format_iso_date_short(&movement.date))
                        .monospace()
                        .size(12.0)
                        .color(Palette::MUTED),
                );
                ui.label(
                    RichText::new(movement.kind.label())
                        .size(12.0)
                        .strong()
                        .color(color),
                );
                ui.add(
                    Label::new(
                        RichText::new(&movement.party)
                            .size(12.0)
                            .color(Palette::TEXT),
                    )
                    .truncate(),
                );
                ui.add(
                    Label::new(
                        RichText::new(&movement.account_name)
                            .size(11.5)
                            .color(Palette::MUTED),
                    )
                    .truncate(),
                );
            },
            |ui| {
                ui.label(
                    RichText::new(movement.amount.format_with_symbol())
                        .size(12.5)
                        .strong()
                        .color(color),
                );
                ui.label(
                    RichText::new(if inflow { "Giriş" } else { "Çıkış" })
                        .size(11.5)
                        .color(Palette::MUTED),
                );
            },
        );

        if let Some(description) = movement.description.as_deref() {
            ui.add(
                Label::new(RichText::new(description).size(11.5).color(Palette::MUTED)).truncate(),
            );
        }
    });
}

/// Kasa/banka alt toplamları.
fn cash_footer(ui: &mut Ui, report: &crate::report::CashMovementReport) {
    widgets::card(ui, "Alt toplam", |ui| {
        Grid::new("rapor_kasa_toplam")
            .num_columns(2)
            .spacing([18.0, 6.0])
            .show(ui, |ui| {
                totals_row(ui, "Dönem başı bakiye", report.opening_balance);
                totals_row(ui, "Tahsilat toplamı", report.collections);
                totals_row(ui, "Ödeme toplamı", report.payments);
                totals_row(ui, "Gelir toplamı", report.incomes);
                totals_row(ui, "Gider toplamı", report.expenses);
                totals_row(ui, "Dönem sonu bakiye", report.closing_balance);
            });
    });
}

/// Gelir/gider özeti gövdesi.
fn income_body(ui: &mut Ui, state: &ReportsState) {
    let Some(report) = state.income_expense_report.as_ref() else {
        widgets::card(ui, "Rapor yok", |ui| {
            empty_label(ui, "Gelir/gider raporu okunamadı.");
        });
        return;
    };

    if report.summary.is_empty() {
        widgets::card(ui, "Kayıt yok", |ui| {
            empty_label(ui, "Seçili dönemde gelir/gider kaydı bulunmuyor.");
        });
        return;
    }
    if !report.summary.has_confirmed() {
        widgets::card(ui, "Kesin kayıt yok", |ui| {
            empty_label(
                ui,
                "Seçili dönemde kesin gelir/gider kaydı bulunmuyor (iptaller hariç).",
            );
        });
        income_categories(ui, report);
        income_trend(ui, report);
        return;
    }

    income_cards(ui, report);
    ui.add_space(metrics::SECTION_GAP);
    income_categories(ui, report);
    ui.add_space(metrics::SECTION_GAP);
    income_accounts(ui, report);
    ui.add_space(metrics::SECTION_GAP);
    income_trend(ui, report);
    ui.add_space(metrics::SECTION_GAP);
    income_footer(ui, report);
}

/// Gelir/gider kartları.
fn income_cards(ui: &mut Ui, report: &IncomeExpenseReport) {
    widgets::card(ui, "Dönem toplamları", |ui| {
        Grid::new("rapor_gelir_gider_kartlari")
            .num_columns(2)
            .spacing([18.0, 6.0])
            .show(ui, |ui| {
                widgets::row_label(ui, "Gelir toplamı");
                ui.label(
                    RichText::new(report.summary.income_total.format_with_symbol())
                        .strong()
                        .color(Palette::SUCCESS),
                );
                ui.end_row();
                widgets::row_label(ui, "Gider toplamı");
                ui.label(
                    RichText::new(report.summary.expense_total.format_with_symbol())
                        .strong()
                        .color(Palette::DANGER),
                );
                ui.end_row();
                widgets::row_label(ui, "Net (gelir - gider)");
                ui.label(
                    RichText::new(report.summary.net().format_with_symbol())
                        .size(15.0)
                        .strong()
                        .color(if report.summary.net().kurus() < 0 {
                            Palette::DANGER
                        } else {
                            Palette::TEXT
                        }),
                );
                ui.end_row();
            });
    });
}

/// Kategori dağılımı ve yüzdeler.
fn income_categories(ui: &mut Ui, report: &IncomeExpenseReport) {
    widgets::card(ui, "Kategori dağılımı", |ui| {
        if report.summary.categories.is_empty() {
            empty_label(ui, "Kesin gelir/gider kaydı bulunmuyor.");
            return;
        }
        Grid::new("rapor_gelir_gider_kategorileri")
            .spacing([5.0, 4.0])
            .show(ui, |ui| {
                header_cell(ui, "Kategori", 170.0, false);
                header_cell(ui, "Tür", 60.0, false);
                header_cell(ui, "Tutar", 100.0, true);
                header_cell(ui, "Oran", 60.0, true);
                ui.end_row();
                for category in &report.summary.categories {
                    cell(
                        ui,
                        &category.category_name,
                        170.0,
                        false,
                        Palette::TEXT,
                        false,
                    );
                    cell(
                        ui,
                        category.entry_type.label(),
                        60.0,
                        false,
                        duplicate_type_color(category.entry_type),
                        false,
                    );
                    money_cell(ui, category.total, 100.0, Palette::TEXT);
                    cell(
                        ui,
                        &crate::report::percentage_label(category.percentage),
                        60.0,
                        true,
                        Palette::MUTED,
                        false,
                    );
                    ui.end_row();
                }
            });
    });
    ui.add_space(6.0);
    ui.add(
        Label::new(
            RichText::new(
                "Oranlar kendi türünün toplamına göre tam sayıdır; taban sıfırsa — gösterilir.",
            )
            .size(11.5)
            .color(Palette::MUTED),
        )
        .wrap(),
    );
}

/// Hesap dağılımı.
fn income_accounts(ui: &mut Ui, report: &IncomeExpenseReport) {
    widgets::card(ui, "Kasa/banka dağılımı", |ui| {
        if report.summary.accounts.is_empty() {
            empty_label(ui, "Kesin gelir/gider kaydı bulunmuyor.");
            return;
        }
        Grid::new("rapor_gelir_gider_hesaplari")
            .spacing([5.0, 4.0])
            .show(ui, |ui| {
                header_cell(ui, "Hesap", 170.0, false);
                header_cell(ui, "Gelir", 90.0, true);
                header_cell(ui, "Gider", 90.0, true);
                header_cell(ui, "Net", 90.0, true);
                ui.end_row();
                for account in &report.summary.accounts {
                    cell(
                        ui,
                        &account.cash_account_name,
                        170.0,
                        false,
                        Palette::TEXT,
                        false,
                    );
                    money_cell(ui, account.income_total, 90.0, Palette::SUCCESS);
                    money_cell(ui, account.expense_total, 90.0, Palette::DANGER);
                    money_cell(ui, account.net(), 90.0, Palette::TEXT);
                    ui.end_row();
                }
            });
    });
}

/// Son altı aylık metin trend tablosu.
fn income_trend(ui: &mut Ui, report: &IncomeExpenseReport) {
    widgets::card(ui, "Son altı ay", |ui| {
        Grid::new("rapor_gelir_gider_trend")
            .spacing([5.0, 4.0])
            .show(ui, |ui| {
                header_cell(ui, "Ay", 110.0, false);
                header_cell(ui, "Gelir", 95.0, true);
                header_cell(ui, "Gider", 95.0, true);
                header_cell(ui, "Net", 95.0, true);
                ui.end_row();
                for month in &report.trend {
                    cell(ui, &month.label(), 110.0, false, Palette::TEXT, false);
                    money_cell(ui, month.income_total, 95.0, Palette::SUCCESS);
                    money_cell(ui, month.expense_total, 95.0, Palette::DANGER);
                    money_cell(
                        ui,
                        month.net(),
                        95.0,
                        if month.net().kurus() < 0 {
                            Palette::DANGER
                        } else {
                            Palette::TEXT
                        },
                    );
                    ui.end_row();
                }
            });
    });
    ui.add_space(6.0);
    ui.add(
        Label::new(
            RichText::new("Trend, seçili dönem sonundan geriye altı takvim ayını kapsar.")
                .size(11.5)
                .color(Palette::MUTED),
        )
        .wrap(),
    );
}

/// Gelir/gider alt toplamları.
fn income_footer(ui: &mut Ui, report: &IncomeExpenseReport) {
    widgets::card(ui, "Alt toplam", |ui| {
        Grid::new("rapor_gelir_gider_toplam")
            .num_columns(2)
            .spacing([18.0, 6.0])
            .show(ui, |ui| {
                totals_row(ui, "Gelir toplamı", report.summary.income_total);
                totals_row(ui, "Gider toplamı", report.summary.expense_total);
                totals_row(ui, "Net (gelir - gider)", report.summary.net());
                widgets::row_label(ui, "Kesin kayıt sayısı");
                ui.label(format!("{} adet", report.summary.confirmed_count));
                ui.end_row();
            });
    });
}

/// Tür etiketini renklendirir (gelir/gider).
fn duplicate_type_color(entry_type: crate::income_expense::EntryType) -> egui::Color32 {
    match entry_type {
        crate::income_expense::EntryType::Gelir => Palette::SUCCESS,
        crate::income_expense::EntryType::Gider => Palette::DANGER,
    }
}

/// Stok durumuna göre renk.
fn status_color(status: StockStatus) -> egui::Color32 {
    match status {
        StockStatus::Yeterli => Palette::TEXT,
        StockStatus::Dusuk => Palette::WARNING,
        StockStatus::Sifir => Palette::DANGER,
        StockStatus::Eksi => Palette::DANGER,
    }
}

/// "Tümü / Aktif / Pasif" etiketi.
fn active_label(active: Option<bool>) -> &'static str {
    match active {
        None => "Tümü",
        Some(true) => "Aktif",
        Some(false) => "Pasif",
    }
}

/// Alt toplam satırı: solda etiket, sağda para.
fn totals_row(ui: &mut Ui, label: &str, amount: crate::money::Money) {
    widgets::row_label(ui, label);
    ui.label(
        RichText::new(amount.format_with_symbol())
            .strong()
            .color(if amount.kurus() < 0 {
                Palette::DANGER
            } else {
                Palette::TEXT
            }),
    );
    ui.end_row();
}

/// Tıklanabilir sıralama başlığı.
fn sort_header(
    ui: &mut Ui,
    label: &str,
    width: f32,
    column: CustomerBalanceSortColumn,
    state: &ReportsState,
    action: &mut Option<Action>,
) {
    let active = state.customer_sort_column == column;
    let arrow = if active {
        if state.customer_sort_ascending {
            " ▲"
        } else {
            " ▼"
        }
    } else {
        ""
    };
    let text = format!("{label}{arrow}");
    let response = ui.add_sized(
        [width, 18.0],
        Button::new(RichText::new(text).size(11.0).strong().color(if active {
            Palette::ACCENT
        } else {
            Palette::MUTED
        }))
        .frame(false),
    );
    if response.clicked() {
        *action = Some(Action::Sort(column));
    }
    response.on_hover_text("Sıralamak için tıklayın.");
}

/// Normal tablo hücresi.
fn cell(
    ui: &mut Ui,
    text: &str,
    width: f32,
    right_aligned: bool,
    color: egui::Color32,
    strong: bool,
) {
    let mut rich = RichText::new(text).size(11.5).color(color);
    if strong {
        rich = rich.strong();
    }
    let label = Label::new(rich)
        .halign(if right_aligned {
            Align::RIGHT
        } else {
            Align::LEFT
        })
        .truncate();
    ui.add_sized([width, 18.0], label);
}

/// Tablo başlık hücresi.
fn header_cell(ui: &mut Ui, text: &str, width: f32, right_aligned: bool) {
    cell(ui, text, width, right_aligned, Palette::MUTED, true);
}

/// Para hücresi; negatif tutarlar kırmızı gösterilir.
fn money_cell(ui: &mut Ui, amount: crate::money::Money, width: f32, color: egui::Color32) {
    let color = if amount.kurus() < 0 {
        Palette::DANGER
    } else {
        color
    };
    cell(ui, &amount.format_turkish(), width, true, color, false);
}

/// Kart içi açıklama.
fn empty_label(ui: &mut Ui, text: &str) {
    ui.add(Label::new(RichText::new(text).color(Palette::MUTED)).wrap());
}
