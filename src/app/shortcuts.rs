// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Klavye kısayollarının tek tanım tablosu ve uygulama komutları.

use egui::{Context, Key, Modifiers};

use crate::invoice::InvoiceType;

use super::{
    CancelTarget, CustomerScreen, IncomeExpenseScreen, InvoiceScreen, OnMuhasebeApp, ProductScreen,
    View,
};

/// Kısayolun tetiklediği uygulama komutu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutAction {
    /// Yeni kayıt formu.
    New,
    /// Aktif formu kaydet.
    Save,
    /// Global aramayı aç.
    Search,
    /// Aktif listeyi yenile.
    Refresh,
    /// Formu/modalı kapat.
    Cancel,
    /// Ayarlar ekranı.
    Settings,
    /// Oturumu kapatma onayı.
    Logout,
    /// Numaralı bölüme git.
    Navigate(View),
}

/// Tek bir kısayol tanımı.
#[derive(Debug, Clone, Copy)]
pub struct ShortcutDef {
    /// Tetiklenen komut.
    pub action: ShortcutAction,
    /// Tuş.
    pub key: Key,
    /// Ctrl basılı mı?
    pub ctrl: bool,
    /// Metin alanı odaklıyken de çalışır mı?
    pub global: bool,
    /// Kullanıcıya gösterilen açıklama.
    pub label: &'static str,
    /// Yardım tablosundaki grup.
    pub group: &'static str,
}

impl ShortcutDef {
    /// Kısayolun kullanıcıya gösterilen tuş bileşimi.
    pub fn keys_label(self) -> String {
        let prefix = if self.ctrl { "Ctrl+" } else { "" };
        format!("{prefix}{}", self.key.name())
    }
}

/// Uygulamadaki **tüm** kısayollar. Yeni kısayol yalnızca buraya eklenir.
pub const SHORTCUTS: [ShortcutDef; 17] = [
    ShortcutDef {
        action: ShortcutAction::New,
        key: Key::N,
        ctrl: true,
        global: false,
        label: "Yeni kayıt",
        group: "Kayıt",
    },
    ShortcutDef {
        action: ShortcutAction::Save,
        key: Key::S,
        ctrl: true,
        global: false,
        label: "Aktif formu kaydet",
        group: "Kayıt",
    },
    ShortcutDef {
        action: ShortcutAction::Search,
        key: Key::K,
        ctrl: true,
        global: true,
        label: "Global arama",
        group: "Arama",
    },
    ShortcutDef {
        action: ShortcutAction::Search,
        key: Key::F,
        ctrl: true,
        global: true,
        label: "Global arama",
        group: "Arama",
    },
    ShortcutDef {
        action: ShortcutAction::Refresh,
        key: Key::F5,
        ctrl: false,
        global: true,
        label: "Aktif listeyi yenile",
        group: "Genel",
    },
    ShortcutDef {
        action: ShortcutAction::Cancel,
        key: Key::Escape,
        ctrl: false,
        global: false,
        label: "Formu / pencereyi kapat",
        group: "Genel",
    },
    ShortcutDef {
        action: ShortcutAction::Settings,
        key: Key::Comma,
        ctrl: true,
        global: false,
        label: "Ayarlar",
        group: "Genel",
    },
    ShortcutDef {
        action: ShortcutAction::Logout,
        key: Key::Q,
        ctrl: true,
        global: true,
        label: "Oturumu kapat",
        group: "Genel",
    },
    ShortcutDef {
        action: ShortcutAction::Navigate(View::Overview),
        key: Key::Num1,
        ctrl: true,
        global: false,
        label: "Genel Bakış",
        group: "Bölümler",
    },
    ShortcutDef {
        action: ShortcutAction::Navigate(View::Customers),
        key: Key::Num2,
        ctrl: true,
        global: false,
        label: "Cari Hesaplar",
        group: "Bölümler",
    },
    ShortcutDef {
        action: ShortcutAction::Navigate(View::Inventory),
        key: Key::Num3,
        ctrl: true,
        global: false,
        label: "Ürün ve Stok",
        group: "Bölümler",
    },
    ShortcutDef {
        action: ShortcutAction::Navigate(View::PurchaseInvoices),
        key: Key::Num4,
        ctrl: true,
        global: false,
        label: "Alış Faturaları",
        group: "Bölümler",
    },
    ShortcutDef {
        action: ShortcutAction::Navigate(View::SalesInvoices),
        key: Key::Num5,
        ctrl: true,
        global: false,
        label: "Satış Faturaları",
        group: "Bölümler",
    },
    ShortcutDef {
        action: ShortcutAction::Navigate(View::CashAndBank),
        key: Key::Num6,
        ctrl: true,
        global: false,
        label: "Kasa ve Banka",
        group: "Bölümler",
    },
    ShortcutDef {
        action: ShortcutAction::Navigate(View::IncomeExpense),
        key: Key::Num7,
        ctrl: true,
        global: false,
        label: "Gelir ve Gider",
        group: "Bölümler",
    },
    ShortcutDef {
        action: ShortcutAction::Navigate(View::Reports),
        key: Key::Num8,
        ctrl: true,
        global: false,
        label: "Raporlar",
        group: "Bölümler",
    },
    ShortcutDef {
        action: ShortcutAction::Navigate(View::Settings),
        key: Key::Num9,
        ctrl: true,
        global: false,
        label: "Ayarlar",
        group: "Bölümler",
    },
];

impl OnMuhasebeApp {
    /// Klavye kısayolları yardım penceresi açık mı?
    pub fn shortcuts_help_open(&self) -> bool {
        self.shortcuts_open
    }

    /// Klavye kısayolları yardım penceresini açar.
    pub fn open_shortcuts_help(&mut self) {
        self.shortcuts_open = true;
    }

    /// Klavye kısayolları yardım penceresini kapatır.
    pub fn close_shortcuts_help(&mut self) {
        self.shortcuts_open = false;
    }

    /// Kısayolları işler. Metin alanı odaklıyken yalnızca `global` tanımlar
    /// çalışır; böylece yazarken `Ctrl+N` gibi komutlar yanlışlıkla tetiklenmez.
    pub fn handle_shortcuts(&mut self, context: &Context) {
        if self.onboarding.open {
            if self.onboarding.skip_confirm
                && context.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape))
            {
                self.cancel_onboarding_skip();
            }
            return;
        }

        if self.search.open {
            let close = context.input_mut(|input| {
                input.consume_key(Modifiers::CTRL, Key::K)
                    || input.consume_key(Modifiers::NONE, Key::Escape)
            });
            if close {
                self.close_search_palette();
            }
            return;
        }

        if self.shortcuts_open {
            if context.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape)) {
                self.close_shortcuts_help();
            }
            return;
        }

        if self.error_details_open {
            if context.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape)) {
                self.error_details_open = false;
            }
            return;
        }

        if self.logout_confirm {
            if context.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape)) {
                self.logout_confirm = false;
            }
            return;
        }

        if self.cancel_confirm.is_some() {
            if context.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape)) {
                self.cancel_confirm = None;
            }
            return;
        }

        if self.about_open {
            if context.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape)) {
                self.about_open = false;
            }
            return;
        }

        let wants_keyboard = context.egui_wants_keyboard_input();
        let mut triggered = None;
        for shortcut in SHORTCUTS {
            if wants_keyboard && !shortcut.global {
                continue;
            }
            let modifiers = if shortcut.ctrl {
                Modifiers::CTRL
            } else {
                Modifiers::NONE
            };
            if context.input_mut(|input| input.consume_key(modifiers, shortcut.key)) {
                triggered = Some(shortcut.action);
                break;
            }
        }

        if let Some(action) = triggered {
            self.execute_shortcut(action);
        }
    }

    /// Kısayol komutunu uygular.
    pub fn execute_shortcut(&mut self, action: ShortcutAction) {
        match action {
            ShortcutAction::New => self.new_in_current_view(),
            ShortcutAction::Save => self.save_current_form(),
            ShortcutAction::Search => self.open_search_palette(),
            ShortcutAction::Refresh => self.refresh_current_view(),
            ShortcutAction::Cancel => self.request_cancel_current_form(),
            ShortcutAction::Settings => self.set_view(View::Settings),
            ShortcutAction::Logout => self.logout_confirm = true,
            ShortcutAction::Navigate(view) => self.set_view(view),
        }
    }

    /// Aktif bölüme uygun yeni kayıt formunu açar.
    pub fn new_in_current_view(&mut self) {
        match self.view {
            View::Customers => self.open_customer_new_form(),
            View::Inventory => self.open_product_new_form(),
            View::PurchaseInvoices => {
                self.set_invoice_section(InvoiceType::Purchase);
                self.open_invoice_new_form();
            }
            View::SalesInvoices => {
                self.set_invoice_section(InvoiceType::Sales);
                self.open_invoice_new_form();
            }
            View::CashAndBank => self.open_cash_account_new_form(),
            View::IncomeExpense => {
                self.open_income_expense_entry_form(crate::income_expense::EntryType::Gider)
            }
            View::Overview | View::Reports | View::Settings => {}
        }
    }

    /// Açık formu kaydeder; form yoksa hiçbir şey yapmaz.
    pub fn save_current_form(&mut self) {
        match self.view {
            View::Customers if self.customer.screen == CustomerScreen::Form => {
                self.submit_customer_form();
            }
            View::Customers => {}
            View::Inventory => match self.product.screen {
                ProductScreen::Form => self.submit_product_form(),
                ProductScreen::MovementForm => self.submit_movement_form(),
                ProductScreen::List | ProductScreen::Detail => {}
            },
            View::PurchaseInvoices | View::SalesInvoices => match self.invoice.screen {
                InvoiceScreen::Form => {
                    if self.invoice.pending.is_some() {
                        self.confirm_invoice_save();
                    } else {
                        self.submit_invoice_form();
                    }
                }
                InvoiceScreen::List | InvoiceScreen::Detail => {}
            },
            View::CashAndBank => match self.cash.screen {
                super::CashScreen::AccountForm => self.submit_cash_account_form(),
                super::CashScreen::MovementForm => {
                    if self.cash.payment_pending.is_some() {
                        self.confirm_payment_save();
                    } else {
                        self.submit_payment_form();
                    }
                }
                _ => {}
            },
            View::IncomeExpense => match self.income_expense.screen {
                IncomeExpenseScreen::EntryForm => {
                    if self.income_expense.entry_pending.is_some() {
                        self.confirm_income_expense_entry_save();
                    } else {
                        self.submit_income_expense_entry_form();
                    }
                }
                IncomeExpenseScreen::CategoryForm => {
                    self.submit_income_expense_category_form();
                }
                _ => {}
            },
            View::Overview | View::Reports | View::Settings => {}
        }
    }

    /// Aktif bölümün listesini/özetini yeniler.
    pub fn refresh_current_view(&mut self) {
        self.refresh_status();
        match self.view {
            View::Overview => {
                self.refresh_customer_list();
                self.refresh_product_list();
                self.refresh_invoice_list();
            }
            View::Customers => self.refresh_customer_list(),
            View::Inventory => self.refresh_product_list(),
            View::PurchaseInvoices | View::SalesInvoices => self.refresh_invoice_list(),
            View::CashAndBank => {
                self.refresh_cash_account_list();
                self.refresh_payment_list();
            }
            View::IncomeExpense => {
                self.refresh_income_expense_list();
                self.refresh_income_expense_category_list();
            }
            View::Reports => self.enter_reports_section(),
            View::Settings => {
                self.refresh_settings_users();
                self.refresh_backup_list();
            }
        }
    }

    /// Açık formu iptal etmek ister. Değişiklik varsa onay ister.
    pub fn request_cancel_current_form(&mut self) {
        if self.onboarding.open || self.search.open || self.shortcuts_open {
            return;
        }
        let target = match self.view {
            View::Customers if self.customer.screen == CustomerScreen::Form => {
                Some(CancelTarget::Customer)
            }
            View::Customers => None,
            View::Inventory => match self.product.screen {
                ProductScreen::Form => Some(CancelTarget::Product),
                ProductScreen::MovementForm => Some(CancelTarget::Movement),
                _ => None,
            },
            View::PurchaseInvoices | View::SalesInvoices
                if self.invoice.screen == InvoiceScreen::Form =>
            {
                Some(CancelTarget::Invoice)
            }
            View::CashAndBank => match self.cash.screen {
                super::CashScreen::AccountForm => Some(CancelTarget::CashAccount),
                super::CashScreen::MovementForm => Some(CancelTarget::Payment),
                _ => None,
            },
            View::IncomeExpense => match self.income_expense.screen {
                IncomeExpenseScreen::EntryForm => Some(CancelTarget::IncomeEntry),
                IncomeExpenseScreen::CategoryForm => Some(CancelTarget::Category),
                _ => None,
            },
            _ => None,
        };

        let Some(target) = target else {
            return;
        };

        if self.cancel_target_dirty(target) {
            self.cancel_confirm = Some(target);
        } else {
            self.force_cancel_target(target);
        }
    }

    /// Onaylanan form iptalini uygular.
    pub fn confirm_cancel_current_form(&mut self) {
        if let Some(target) = self.cancel_confirm.take() {
            self.force_cancel_target(target);
        }
    }

    /// İptal onayını kapatır.
    pub fn dismiss_cancel_confirmation(&mut self) {
        self.cancel_confirm = None;
    }

    fn force_cancel_target(&mut self, target: CancelTarget) {
        match target {
            CancelTarget::Customer => self.cancel_customer_form(),
            CancelTarget::Product => self.cancel_product_form(),
            CancelTarget::Movement => self.cancel_movement_form(),
            CancelTarget::Invoice => self.cancel_invoice_form(),
            CancelTarget::CashAccount => self.cancel_cash_account_form(),
            CancelTarget::Payment => self.open_payment_list(),
            CancelTarget::IncomeEntry => self.open_income_expense_list(),
            CancelTarget::Category => self.cancel_income_expense_category_form(),
        }
    }

    fn cancel_target_dirty(&self, target: CancelTarget) -> bool {
        match target {
            CancelTarget::Customer => self.customer_form_dirty(),
            CancelTarget::Product => self.product_form_dirty(),
            CancelTarget::Movement => self.movement_form_dirty(),
            CancelTarget::Invoice => self.invoice_form_dirty(),
            CancelTarget::CashAccount => self.cash_account_form_dirty(),
            CancelTarget::Payment => self.payment_form_dirty(),
            CancelTarget::IncomeEntry => self.income_entry_form_dirty(),
            CancelTarget::Category => self.category_form_dirty(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kisayol_tablosunda_tekrar_eden_tus_yoktur() {
        for (index, shortcut) in SHORTCUTS.iter().enumerate() {
            for other in &SHORTCUTS[index + 1..] {
                assert!(
                    !(shortcut.key == other.key && shortcut.ctrl == other.ctrl),
                    "aynı tuş iki kez tanımlanmış: {} / {}",
                    shortcut.label,
                    other.label
                );
            }
        }
    }

    #[test]
    fn tum_kisayollarin_etiketi_vardir() {
        for shortcut in SHORTCUTS {
            assert!(!shortcut.label.is_empty());
            assert!(!shortcut.keys_label().is_empty());
        }
    }

    #[test]
    fn metin_alaninda_global_olmayan_kisayol_engellenir() {
        assert!(
            SHORTCUTS
                .iter()
                .filter(|shortcut| shortcut.action == ShortcutAction::Search)
                .all(|shortcut| shortcut.global)
        );
    }
}
