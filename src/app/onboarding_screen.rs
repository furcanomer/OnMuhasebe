// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! İlk çalıştırma sihirbazının uygulama durumu ve komutları.

use crate::cash::{CashAccountDraft, Field as CashField, FieldErrors as CashFieldErrors};
use crate::error::AppError;
use crate::income_expense::{CategoryDraft, CategoryField, EntryType};
use crate::onboarding::{
    CATEGORY_PACKAGES, CompanyInfo, ONBOARDING_COMPLETED_KEY, OnboardingPlan, OnboardingStep,
};

use super::{FocusTarget, OnMuhasebeApp, View};

/// Sihirbazın geçici arayüz durumu.
#[derive(Debug)]
pub struct OnboardingState {
    /// Modal açık mı?
    pub open: bool,
    /// Aktif adım.
    pub step: OnboardingStep,
    /// Şirket bilgileri.
    pub company: CompanyInfo,
    /// İlk kasa/banka hesabı taslağı.
    pub cash_draft: CashAccountDraft,
    /// Kasa hesabı alan hataları.
    pub cash_errors: CashFieldErrors,
    /// Kategori paketlerinin seçim durumu (indeksler [`CATEGORY_PACKAGES`] ile eşleşir).
    pub selected_categories: Vec<bool>,
    /// Genel hata.
    pub error: Option<String>,
    /// Atlama onayı bekleniyor mu?
    pub skip_confirm: bool,
}

impl Default for OnboardingState {
    fn default() -> Self {
        Self {
            open: false,
            step: OnboardingStep::Company,
            company: CompanyInfo::default(),
            cash_draft: CashAccountDraft::default(),
            cash_errors: CashFieldErrors::default(),
            selected_categories: vec![true; CATEGORY_PACKAGES.len()],
            error: None,
            skip_confirm: false,
        }
    }
}

impl OnMuhasebeApp {
    /// Sihirbaz durumunu okur.
    pub fn onboarding_state(&self) -> &OnboardingState {
        &self.onboarding
    }

    /// Sihirbaz durumunu değiştirilebilir olarak verir.
    pub fn onboarding_state_mut(&mut self) -> &mut OnboardingState {
        &mut self.onboarding
    }

    /// Sihirbazı açar; şirket bilgileri veritabanından yüklenir.
    pub fn open_onboarding(&mut self) {
        let company = self
            .database
            .as_ref()
            .and_then(|database| CompanyInfo::load(database).ok())
            .unwrap_or_default();
        self.onboarding.open = true;
        self.onboarding.step = OnboardingStep::Company;
        self.onboarding.company = company;
        self.onboarding.cash_draft = CashAccountDraft::default();
        self.onboarding.cash_errors = CashFieldErrors::default();
        self.onboarding.selected_categories = vec![true; CATEGORY_PACKAGES.len()];
        self.onboarding.error = None;
        self.onboarding.skip_confirm = false;
        self.focus_target = Some(FocusTarget::OnboardingCompanyName);
    }

    /// İlk yönetici oluşturulduktan sonra sihirbazı başlatır.
    pub fn begin_onboarding_after_first_admin(&mut self) {
        if let Some(database) = self.database.as_ref()
            && let Err(error) = database.setting_set(ONBOARDING_COMPLETED_KEY, "0")
        {
            tracing::error!(error = %error, "sihirbaz durumu yazılamadı");
        }
        self.open_onboarding();
    }

    /// Giriş sonrası yarım kalmış sihirbazı (yalnızca `0` işaretliyse) açar.
    pub fn maybe_open_onboarding(&mut self) {
        let should_open = self
            .database
            .as_ref()
            .and_then(|database| {
                database
                    .setting_get(ONBOARDING_COMPLETED_KEY)
                    .ok()
                    .flatten()
            })
            .is_some_and(|value| value == "0");
        if should_open {
            self.open_onboarding();
        }
    }

    /// Ayarlar ekranından sihirbazı yeniden açar.
    pub fn reopen_onboarding(&mut self) {
        self.open_onboarding();
    }

    /// Sihirbazı bir sonraki adıma geçirir (gerekli doğrulamayı yapar).
    pub fn onboarding_next(&mut self) {
        self.onboarding.error = None;
        match self.onboarding.step {
            OnboardingStep::Company => {
                self.onboarding.step = OnboardingStep::Cash;
                self.focus_target = Some(FocusTarget::OnboardingCashName);
            }
            OnboardingStep::Cash => {
                if self.onboarding.cash_draft.name.trim().is_empty() {
                    self.onboarding.cash_errors = CashFieldErrors::default();
                    self.onboarding.step = OnboardingStep::Categories;
                } else {
                    match self.onboarding.cash_draft.validate() {
                        Ok(_) => {
                            self.onboarding.cash_errors = CashFieldErrors::default();
                            self.onboarding.step = OnboardingStep::Categories;
                        }
                        Err(errors) => {
                            self.onboarding.cash_errors = errors;
                            self.focus_target = Some(FocusTarget::OnboardingCashName);
                        }
                    }
                }
            }
            OnboardingStep::Categories => {
                self.onboarding.step = OnboardingStep::Summary;
            }
            OnboardingStep::Summary => {}
        }
    }

    /// Bir önceki adıma döner.
    pub fn onboarding_previous(&mut self) {
        self.onboarding.error = None;
        self.onboarding.step = self.onboarding.step.previous();
    }

    /// Kategori paketi seçimini değiştirir.
    pub fn onboarding_toggle_category(&mut self, index: usize) {
        if let Some(selected) = self.onboarding.selected_categories.get_mut(index) {
            *selected = !*selected;
        }
    }

    /// Tüm kategori paketlerini seçer/seçimi kaldırır.
    pub fn onboarding_set_all_categories(&mut self, selected: bool) {
        self.onboarding.selected_categories.fill(selected);
    }

    /// Sihirbazı atlamak için onay ister.
    pub fn request_onboarding_skip(&mut self) {
        self.onboarding.skip_confirm = true;
    }

    /// Atlama onayını kapatır.
    pub fn cancel_onboarding_skip(&mut self) {
        self.onboarding.skip_confirm = false;
    }

    /// Sihirbazı atlar; yalnızca `onboarding_completed=1` yazar.
    pub fn confirm_onboarding_skip(&mut self) {
        let result = match self.database.as_ref() {
            Some(database) => database.mark_onboarding_completed(),
            None => Err(AppError::Maintenance("Veritabanı açık değil.".to_owned())),
        };
        match result {
            Ok(()) => {
                self.onboarding.open = false;
                self.onboarding.skip_confirm = false;
                self.onboarding.error = None;
                self.settings_screen.settings_notice = Some(
                    "Kurulum sihirbazı atlandı; daha sonra Ayarlar'dan açabilirsiniz.".to_owned(),
                );
            }
            Err(error) => {
                self.onboarding.skip_confirm = false;
                self.onboarding.error = Some(error.to_string());
            }
        }
    }

    /// Sihirbazı tamamlar ve tüm yazımları tek transaction içinde uygular.
    pub fn onboarding_finish(&mut self) {
        self.onboarding.error = None;

        let cash = if self.onboarding.cash_draft.name.trim().is_empty() {
            None
        } else {
            match self.onboarding.cash_draft.validate() {
                Ok(cash) => Some(cash),
                Err(errors) => {
                    self.onboarding.cash_errors = errors;
                    self.onboarding.step = OnboardingStep::Cash;
                    self.focus_target = Some(FocusTarget::OnboardingCashName);
                    return;
                }
            }
        };

        let mut categories = Vec::new();
        for (index, package) in CATEGORY_PACKAGES.iter().enumerate() {
            if !self
                .onboarding
                .selected_categories
                .get(index)
                .copied()
                .unwrap_or(false)
            {
                continue;
            }
            let draft = CategoryDraft {
                name: package.name.to_owned(),
                category_type: package.category_type,
                description: String::new(),
                active: true,
            };
            match draft.validate() {
                Ok(category) => categories.push(category),
                Err(errors) => {
                    self.onboarding.error = Some(
                        errors
                            .get(CategoryField::Name)
                            .unwrap_or("Kategori doğrulanamadı.")
                            .to_owned(),
                    );
                    self.onboarding.step = OnboardingStep::Categories;
                    return;
                }
            }
        }

        let plan = OnboardingPlan {
            company: self.onboarding.company.clone(),
            cash,
            categories,
        };

        let result = match self.database.as_ref() {
            Some(database) => database.apply_onboarding(&plan),
            None => Err(AppError::Maintenance("Veritabanı açık değil.".to_owned())),
        };

        match result {
            Ok(()) => {
                self.onboarding.open = false;
                self.onboarding.step = OnboardingStep::Company;
                self.onboarding.skip_confirm = false;
                self.onboarding.error = None;
                self.settings_screen.settings_notice = Some("Kurulum tamamlandı.".to_owned());
                self.load_settings();
                self.refresh_customer_list();
                self.refresh_product_list();
                self.enter_cash_section();
                self.enter_income_expense_section();
                self.set_view(View::Overview);
                self.request_scroll_reset();
            }
            Err(AppError::CashAccountNameTaken(name)) => {
                self.onboarding.step = OnboardingStep::Cash;
                self.onboarding.cash_errors.push(
                    CashField::Name,
                    format!("'{name}' adlı kasa/banka hesabı zaten var."),
                );
            }
            Err(error) => {
                self.onboarding.error = Some(error.to_string());
            }
        }
    }

    /// Özet adımında seçilen kategorilerin tür bazlı sayısı.
    pub fn onboarding_selected_category_count(&self, entry_type: EntryType) -> usize {
        CATEGORY_PACKAGES
            .iter()
            .enumerate()
            .filter(|(index, package)| {
                package.category_type == entry_type
                    && self
                        .onboarding
                        .selected_categories
                        .get(*index)
                        .copied()
                        .unwrap_or(false)
            })
            .count()
    }
}
