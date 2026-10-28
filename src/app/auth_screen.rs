// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! İlk yönetici, giriş, kilit ve oturum durumu.
//!
//! Oturum yalnızca bellekte tutulur; uygulama kapanınca biter. "Beni hatırla"
//! yoktur. Hatalı giriş sayacı [`LoginThrottle`] içinde, veritabanına
//! yazılmadan tutulur.

use std::time::Instant;

use crate::auth::{
    Actor, LoginThrottle, Role, User, hash_password, mask_username, validate_password,
    verify_password,
};
use crate::error::{AppError, Result};

use super::OnMuhasebeApp;

/// Kimlik doğrulama ekranı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthScreen {
    /// Hiç kullanıcı yokken zorunlu ilk yönetici oluşturma.
    FirstAdmin,
    /// Kullanıcı adı + şifre.
    Login,
    /// Hareketsizlik sonrası kilit ekranı.
    Locked,
    /// Oturum açık; normal uygulama.
    Authenticated,
}

/// Açık oturumun bellek içi bilgisi.
#[derive(Debug, Clone)]
pub struct Session {
    /// Veritabanı katmanına taşınan kimlik.
    pub actor: Actor,
    /// Kilitli mi?
    pub locked: bool,
    /// Son kullanıcı etkileşimi.
    pub last_activity: Instant,
}

/// Giriş/ilk yönetici formunun geçici alanları.
#[derive(Debug)]
pub struct AuthState {
    /// Hangi ekran gösterilecek?
    pub screen: AuthScreen,
    /// Kullanıcı adı alanı.
    pub username: String,
    /// Görünen ad alanı (ilk yönetici).
    pub display_name: String,
    /// Şifre alanı.
    pub password: String,
    /// Şifre tekrar alanı.
    pub password_confirm: String,
    /// Genel hata mesajı.
    pub error: Option<String>,
    /// Başarı/bilgi mesajı (ör. yönetici şifre sıfırlaması).
    pub notice: Option<String>,
    /// Açık oturum.
    pub session: Option<Session>,
    /// Hatalı giriş sınırlayıcısı.
    pub throttle: LoginThrottle,
}

impl Default for AuthState {
    fn default() -> Self {
        Self {
            screen: AuthScreen::Login,
            username: String::new(),
            display_name: String::new(),
            password: String::new(),
            password_confirm: String::new(),
            error: None,
            notice: None,
            session: None,
            throttle: LoginThrottle::default(),
        }
    }
}

impl AuthState {
    /// Form alanlarını temizler (hata/notice korunur).
    pub fn clear_secrets(&mut self) {
        self.password.clear();
        self.password_confirm.clear();
    }

    /// Kullanıcıya gösterilecek hata metni.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

impl OnMuhasebeApp {
    /// Aktif kimlik doğrulama ekranı.
    pub fn auth_screen(&self) -> AuthScreen {
        self.auth.screen
    }

    /// Giriş formu durumu (arayüz salt okunur alanlar için).
    pub fn auth_state(&self) -> &AuthState {
        &self.auth
    }

    /// Giriş formu durumu (arayüz alan güncellemeleri için).
    pub fn auth_state_mut(&mut self) -> &mut AuthState {
        &mut self.auth
    }

    /// Oturum açık mı?
    pub fn is_authenticated(&self) -> bool {
        self.auth.screen == AuthScreen::Authenticated && self.auth.session.is_some()
    }

    /// Oturum kilitli mi?
    pub fn is_locked(&self) -> bool {
        self.auth
            .session
            .as_ref()
            .is_some_and(|session| session.locked)
    }

    /// Oturumdaki aktör.
    pub fn current_actor(&self) -> Option<&Actor> {
        self.auth.session.as_ref().map(|session| &session.actor)
    }

    /// Oturumdaki kullanıcı yönetici mi?
    pub fn is_manager(&self) -> bool {
        self.current_actor()
            .is_some_and(|actor| actor.role == Role::Yonetici)
    }

    /// Arayüz oturumu yokken (birim testleri/komut satırı) yönetici işlemlerine
    /// izin verir. Gerçek arayüzde operatör oturumu açıksa `false` döner.
    pub fn can_administer(&self) -> bool {
        self.is_manager()
            || (self.auth.session.is_none()
                && self
                    .database
                    .as_ref()
                    .is_none_or(|database| database.actor().is_none()))
    }

    /// İlk yönetici oluşturma ekranı gerekli mi?
    pub fn needs_first_admin(&self) -> bool {
        self.auth.screen == AuthScreen::FirstAdmin
    }

    /// İlk yöneticiyi oluşturur ve otomatik giriş yapar.
    pub fn create_initial_admin(&mut self) -> Result<()> {
        self.auth.error = None;
        let Some(database) = self.database.as_ref() else {
            let message = "Veritabanı açık değil.".to_owned();
            self.auth.error = Some(message.clone());
            return Err(AppError::Maintenance(message));
        };

        let username = self.auth.username.trim();
        if username.is_empty() {
            let error = AppError::InvalidRecord("Kullanıcı adı boş olamaz.".to_owned());
            self.auth.error = Some(error.to_string());
            return Err(error);
        }
        if self.auth.password != self.auth.password_confirm {
            let error = AppError::Maintenance("Şifreler eşleşmiyor.".to_owned());
            self.auth.error = Some(error.to_string());
            return Err(error);
        }
        if let Err(error) = validate_password(&self.auth.password) {
            self.auth.error = Some(error.to_string());
            return Err(error);
        }
        let hash = match hash_password(&self.auth.password) {
            Ok(hash) => hash,
            Err(error) => {
                self.auth.error = Some(error.to_string());
                return Err(error);
            }
        };
        let user = match database.create_initial_admin(
            username,
            Some(self.auth.display_name.as_str()),
            &hash,
        ) {
            Ok(user) => user,
            Err(error) => {
                self.auth.error = Some(error.to_string());
                return Err(error);
            }
        };

        let actor = user.actor();
        database.set_actor(Some(actor.clone()));
        database.user_set_last_login(user.id)?;
        tracing::info!(
            user = %mask_username(&user.username),
            "ilk yönetici oluşturuldu"
        );
        if let Err(error) = database.setting_set(crate::onboarding::ONBOARDING_COMPLETED_KEY, "0") {
            tracing::error!(error = %error, "sihirbaz durumu yazılamadı");
        }
        self.finish_login(user);
        Ok(())
    }

    /// Giriş dener (arayüz düğmesi).
    pub fn attempt_login(&mut self) -> Result<()> {
        self.attempt_login_at(Instant::now())
    }

    /// Testlerde zaman enjekte edilebilen giriş.
    pub fn attempt_login_at(&mut self, now: Instant) -> Result<()> {
        self.auth.error = None;
        self.auth.notice = None;
        let username = self.auth.username.trim().to_owned();
        if username.is_empty() || self.auth.password.is_empty() {
            self.auth.error = Some("Kullanıcı adı veya şifre hatalı.".to_owned());
            return Err(AppError::InvalidCredentials);
        }

        if let Err(remaining) = self.auth.throttle.check(&username, now) {
            let message = format!(
                "Çok fazla hatalı giriş denemesi. {remaining} saniye sonra tekrar deneyin."
            );
            self.auth.error = Some(message);
            return Err(AppError::LoginThrottled(remaining));
        }

        let Some(database) = self.database.as_ref() else {
            let message = "Veritabanı açık değil.".to_owned();
            self.auth.error = Some(message.clone());
            return Err(AppError::Maintenance(message));
        };

        let user = database.user_by_username(&username)?;
        let valid = user.as_ref().is_some_and(|user| {
            user.active && verify_password(&user.password_hash, &self.auth.password)
        });

        if !valid {
            let masked = mask_username(&username);
            if let Some(remaining) = self.auth.throttle.record_failure(&username, now) {
                tracing::warn!(user = %masked, "başarısız giriş: bekleme başladı");
                let message = format!(
                    "Çok fazla hatalı giriş denemesi. {remaining} saniye sonra tekrar deneyin."
                );
                self.auth.error = Some(message);
                return Err(AppError::LoginThrottled(remaining));
            }
            tracing::warn!(user = %masked, "başarısız giriş denemesi");
            self.auth.error = Some("Kullanıcı adı veya şifre hatalı.".to_owned());
            return Err(AppError::InvalidCredentials);
        }

        let user = user.expect("doğrulanan kullanıcı bulunmalı");
        self.auth.throttle.record_success(&username);
        database.user_set_last_login(user.id)?;
        tracing::info!(user = %mask_username(&user.username), "oturum açıldı");
        self.finish_login(user);
        Ok(())
    }

    fn finish_login(&mut self, user: User) {
        let actor = user.actor();
        if let Some(database) = self.database.as_ref() {
            database.set_actor(Some(actor.clone()));
        }
        self.auth.screen = AuthScreen::Authenticated;
        self.auth.session = Some(Session {
            actor,
            locked: false,
            last_activity: Instant::now(),
        });
        self.auth.clear_secrets();
        self.auth.error = None;
        self.auth.notice = None;
        self.view = super::View::Overview;

        if let Some(database) = self.database.as_ref() {
            let key = format!("password_reset_required_{}", user.id);
            if database
                .setting_get(&key)
                .ok()
                .flatten()
                .is_some_and(|value| value == "1")
            {
                self.auth.notice = Some(
                    "Şifreniz yönetici tarafından sıfırlandı. Güvenlik için şifrenizi değiştirin."
                        .to_owned(),
                );
                let _ = database.setting_set(&key, "0");
            }
        }

        self.load_settings();
        self.refresh_status();
        self.refresh_customer_list();
        self.refresh_product_list();
        self.set_invoice_section(crate::invoice::InvoiceType::Purchase);
        self.enter_income_expense_section();
        self.refresh_settings_users();
        self.refresh_backup_list();
        self.maybe_open_onboarding();
    }

    /// Oturumu kapatır ve giriş ekranına döner.
    pub fn logout(&mut self) {
        if let Some(actor) = self.current_actor() {
            tracing::info!(user = %mask_username(&actor.username), "oturum kapatıldı");
        }
        if let Some(database) = self.database.as_ref() {
            database.set_actor(None);
        }
        self.auth.session = None;
        self.auth.throttle.clear();
        self.auth.clear_secrets();
        self.auth.error = None;
        self.auth.notice = None;
        self.auth.username.clear();
        self.auth.display_name.clear();
        self.auth.screen = if self
            .database
            .as_ref()
            .and_then(|database| database.user_count().ok())
            .unwrap_or(1)
            == 0
        {
            AuthScreen::FirstAdmin
        } else {
            AuthScreen::Login
        };
        self.view = super::View::Overview;
        self.settings_screen.reset_forms();
        self.onboarding.open = false;
        self.search.open = false;
        self.shortcuts_open = false;
        self.logout_confirm = false;
        self.cancel_confirm = None;
        self.error_details_open = false;
        self.focus_target = None;
    }

    /// Hareketsizlik süresini sıfırlar.
    pub fn touch_activity(&mut self) {
        self.touch_activity_at(Instant::now());
    }

    /// Testlerde zaman enjekte edilebilen hareket kaydı.
    pub fn touch_activity_at(&mut self, now: Instant) {
        if let Some(session) = self.auth.session.as_mut()
            && !session.locked
        {
            session.last_activity = now;
        }
    }

    /// Otomatik kilidi kontrol eder; kilitlendiyse `true`.
    pub fn check_auto_lock(&mut self) -> bool {
        self.check_auto_lock_at(Instant::now())
    }

    /// Testlerde zaman enjekte edilebilen otomatik kilit.
    pub fn check_auto_lock_at(&mut self, now: Instant) -> bool {
        if !self.is_authenticated() || self.is_locked() {
            return false;
        }
        let Some(duration) = self.settings.auto_lock_duration() else {
            return false;
        };
        let Some(session) = self.auth.session.as_ref() else {
            return false;
        };
        if now.saturating_duration_since(session.last_activity) < duration {
            return false;
        }

        self.lock_now();
        true
    }

    /// Oturumu hemen kilitler.
    pub fn lock_now(&mut self) {
        let username = self
            .auth
            .session
            .as_ref()
            .map(|session| session.actor.username.clone());
        if let Some(session) = self.auth.session.as_mut() {
            session.locked = true;
        }
        if let Some(username) = username {
            self.auth.screen = AuthScreen::Locked;
            self.auth.clear_secrets();
            self.auth.error = None;
            tracing::info!(user = %mask_username(&username), "oturum otomatik kilitlendi");
        }
    }

    /// Kilit ekranında şifre ile oturumu açar.
    pub fn unlock(&mut self) -> Result<()> {
        self.unlock_at(Instant::now())
    }

    /// Testlerde zaman enjekte edilebilen kilit açma.
    pub fn unlock_at(&mut self, now: Instant) -> Result<()> {
        self.auth.error = None;
        let Some(actor) = self.current_actor().cloned() else {
            return Err(AppError::NotAuthenticated);
        };
        if let Err(remaining) = self.auth.throttle.check(&actor.username, now) {
            let message =
                format!("Çok fazla hatalı deneme. {remaining} saniye sonra tekrar deneyin.");
            self.auth.error = Some(message);
            return Err(AppError::LoginThrottled(remaining));
        }
        let Some(database) = self.database.as_ref() else {
            return Err(AppError::NotAuthenticated);
        };
        let user = database
            .user_by_username(&actor.username)?
            .ok_or(AppError::UserNotFound(actor.user_id))?;
        if !user.active || !verify_password(&user.password_hash, &self.auth.password) {
            if let Some(remaining) = self.auth.throttle.record_failure(&actor.username, now) {
                self.auth.error = Some(format!(
                    "Çok fazla hatalı deneme. {remaining} saniye sonra tekrar deneyin."
                ));
                return Err(AppError::LoginThrottled(remaining));
            }
            self.auth.error = Some("Kullanıcı adı veya şifre hatalı.".to_owned());
            return Err(AppError::InvalidCredentials);
        }

        self.auth.throttle.record_success(&actor.username);
        self.auth.clear_secrets();
        if let Some(session) = self.auth.session.as_mut() {
            session.locked = false;
            session.last_activity = now;
        }
        self.auth.screen = AuthScreen::Authenticated;
        tracing::info!(
            user = %mask_username(&actor.username),
            "oturum kilidi açıldı"
        );
        Ok(())
    }

    /// Herkesin kendi şifresini değiştirmesi.
    pub fn change_own_password(
        &mut self,
        current_password: &str,
        new_password: &str,
        confirm_password: &str,
    ) -> Result<()> {
        let Some(actor) = self.current_actor().cloned() else {
            return Err(AppError::NotAuthenticated);
        };
        let Some(database) = self.database.as_ref() else {
            return Err(AppError::NotAuthenticated);
        };
        let user = database
            .user_by_username(&actor.username)?
            .ok_or(AppError::UserNotFound(actor.user_id))?;
        if !verify_password(&user.password_hash, current_password) {
            return Err(AppError::InvalidCredentials);
        }
        if new_password != confirm_password {
            return Err(AppError::Maintenance("Şifreler eşleşmiyor.".to_owned()));
        }
        validate_password(new_password)?;
        let hash = hash_password(new_password)?;
        database.user_set_password_hash(user.id, &hash)?;
        database.setting_set(&format!("password_reset_required_{}", user.id), "0")?;
        tracing::info!(
            user = %mask_username(&user.username),
            "şifre değiştirildi"
        );
        Ok(())
    }
}
