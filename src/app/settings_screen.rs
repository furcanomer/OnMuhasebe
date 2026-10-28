// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Ayarlar bölümünün durumu ve işlemleri.
//!
//! Dört sekme: Genel, Güvenlik, Yedekleme, Kullanıcılar. Kullanıcılar sekmesi
//! yalnızca yöneticiye gösterilir; yedek geri yükleme de yönetici işlemidir.
//! Tüm SQL çağrıları `db` katmanındadır.

use std::path::PathBuf;

use crate::auth::{Role, User, hash_password, mask_username, validate_password};
use crate::backup::{self, BackupEntry, BackupKind};
use crate::db::UserDraft;
use crate::error::{AppError, Result};
use crate::settings::AppSettings;

use super::OnMuhasebeApp;

/// Ayarlar ekranı sekmeleri.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    /// Dil, otomatik kilit, log seviyesi.
    General,
    /// Kendi şifresini değiştirme.
    Security,
    /// Manuel yedek, rotasyon ve geri yükleme.
    Backup,
    /// Kullanıcı yönetimi (yalnızca yönetici).
    Users,
}

impl SettingsTab {
    /// Sekme etiketi.
    pub fn label(self) -> &'static str {
        match self {
            Self::General => "Genel",
            Self::Security => "Güvenlik",
            Self::Backup => "Yedekleme",
            Self::Users => "Kullanıcılar",
        }
    }
}

/// Yeni kullanıcı / düzenleme formu.
#[derive(Debug, Clone)]
pub struct UserFormState {
    /// Düzenlenen kullanıcı; `None` ise yeni kayıt.
    pub editing_id: Option<i64>,
    /// Kullanıcı adı (düzenlemede değiştirilemez).
    pub username: String,
    /// Görünen ad.
    pub display_name: String,
    /// Rol.
    pub role: Role,
    /// Aktif mi?
    pub active: bool,
    /// Yeni kullanıcı şifresi.
    pub password: String,
    /// Yeni kullanıcı şifre tekrarı.
    pub password_confirm: String,
}

impl Default for UserFormState {
    fn default() -> Self {
        Self {
            editing_id: None,
            username: String::new(),
            display_name: String::new(),
            role: Role::Operator,
            active: true,
            password: String::new(),
            password_confirm: String::new(),
        }
    }
}

/// Şifre değiştirme formu.
#[derive(Debug, Default, Clone)]
pub struct PasswordFormState {
    /// Mevcut şifre.
    pub current: String,
    /// Yeni şifre.
    pub new_password: String,
    /// Yeni şifre tekrarı.
    pub confirm: String,
}

/// Ayarlar ekranının geçici durumu.
#[derive(Debug)]
pub struct SettingsState {
    /// Aktif sekme.
    pub tab: SettingsTab,
    /// Önbelleğe alınmış kullanıcılar.
    pub users: Vec<User>,
    /// Kullanıcı listesi hatası.
    pub users_error: Option<String>,
    /// Kullanıcı formu açık mı?
    pub user_form_open: bool,
    /// Kullanıcı formu.
    pub user_form: UserFormState,
    /// Kullanıcı formu hatası.
    pub user_form_error: Option<String>,
    /// Kullanıcı işlemi bilgisi.
    pub users_notice: Option<String>,
    /// Şifresi sıfırlanacak kullanıcı.
    pub reset_user_id: Option<i64>,
    /// Yönetici şifre sıfırlama: yeni şifre.
    pub reset_password: String,
    /// Yönetici şifre sıfırlama: tekrar.
    pub reset_password_confirm: String,
    /// Silme onayı bekleyen kullanıcı.
    pub pending_delete_user: Option<i64>,
    /// Kendi şifre formu.
    pub password_form: PasswordFormState,
    /// Şifre formu hatası.
    pub password_error: Option<String>,
    /// Şifre formu bilgisi.
    pub password_notice: Option<String>,
    /// Genel/backup ayar hatası.
    pub settings_error: Option<String>,
    /// Genel/backup ayar bilgisi.
    pub settings_notice: Option<String>,
    /// Son yedekler.
    pub backups: Vec<BackupEntry>,
    /// Yedek listesi hatası.
    pub backup_error: Option<String>,
    /// Yedek işlemi bilgisi.
    pub backup_notice: Option<String>,
    /// Toplam yedek boyutu (bayt).
    pub backup_total_bytes: u64,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            tab: SettingsTab::General,
            users: Vec::new(),
            users_error: None,
            user_form_open: false,
            user_form: UserFormState::default(),
            user_form_error: None,
            users_notice: None,
            reset_user_id: None,
            reset_password: String::new(),
            reset_password_confirm: String::new(),
            pending_delete_user: None,
            password_form: PasswordFormState::default(),
            password_error: None,
            password_notice: None,
            settings_error: None,
            settings_notice: None,
            backups: Vec::new(),
            backup_error: None,
            backup_notice: None,
            backup_total_bytes: 0,
        }
    }
}

impl SettingsState {
    /// Form ve geçici mesajları sıfırlar (çıkışta çağrılır).
    pub fn reset_forms(&mut self) {
        self.user_form_open = false;
        self.user_form = UserFormState::default();
        self.user_form_error = None;
        self.users_notice = None;
        self.reset_user_id = None;
        self.reset_password.clear();
        self.reset_password_confirm.clear();
        self.pending_delete_user = None;
        self.password_form = PasswordFormState::default();
        self.password_error = None;
        self.password_notice = None;
        self.settings_error = None;
        self.settings_notice = None;
        self.backup_error = None;
        self.backup_notice = None;
        self.tab = SettingsTab::General;
    }
}

impl OnMuhasebeApp {
    fn actor_log_name(&self) -> String {
        self.current_actor()
            .map(|actor| mask_username(&actor.username))
            .unwrap_or_else(|| "-".to_owned())
    }

    /// Tipli uygulama ayarları.
    pub fn settings(&self) -> &AppSettings {
        &self.settings
    }

    /// Ayarları düzenlemek için değiştirilebilir erişim.
    pub fn settings_mut(&mut self) -> &mut AppSettings {
        &mut self.settings
    }

    /// Ayarlar ekranı durumu.
    pub fn settings_state(&self) -> &SettingsState {
        &self.settings_screen
    }

    /// Ayarlar ekranı durumu (form alanları için).
    pub fn settings_state_mut(&mut self) -> &mut SettingsState {
        &mut self.settings_screen
    }

    /// Veritabanından ayarları okur ve log seviyesini uygular.
    pub fn load_settings(&mut self) {
        let Some(database) = self.database.as_ref() else {
            self.settings = AppSettings::default();
            return;
        };
        match AppSettings::load(database) {
            Ok(settings) => {
                crate::logging::set_info_enabled(settings.log_info);
                self.settings = settings;
            }
            Err(error) => {
                self.settings = AppSettings::default();
                self.settings_screen.settings_error = Some(error.to_string());
                tracing::error!(error = %error, "ayarlar okunamadı");
            }
        }
    }

    /// Kullanıcıları yeniden yükler; operatörde liste boş kalır.
    pub fn refresh_settings_users(&mut self) {
        self.settings_screen.users_error = None;
        if !self.is_manager() {
            self.settings_screen.users.clear();
            return;
        }
        let Some(database) = self.database.as_ref() else {
            self.settings_screen.users.clear();
            return;
        };
        match database.user_list() {
            Ok(users) => self.settings_screen.users = users,
            Err(error) => {
                self.settings_screen.users_error = Some(error.to_string());
                tracing::error!(error = %error, "kullanıcı listesi okunamadı");
            }
        }
    }

    /// Yedek listesini ve toplam boyutu yeniler.
    pub fn refresh_backup_list(&mut self) {
        self.settings_screen.backup_error = None;
        let directory = self.config.backups_dir();
        match backup::list_backups(&directory) {
            Ok(entries) => {
                self.settings_screen.backup_total_bytes =
                    entries.iter().map(|entry| entry.size_bytes).sum();
                self.settings_screen.backups = entries;
            }
            Err(error) => {
                self.settings_screen.backup_error = Some(error.to_string());
                tracing::error!(error = %error, "yedek listesi okunamadı");
            }
        }
    }

    /// Genel ayarları kaydeder (yalnızca yönetici).
    pub fn save_general_settings(&mut self) -> Result<()> {
        if !self.can_administer() {
            tracing::warn!(
                user = %self.actor_log_name(),
                "yetki reddedildi: ayarları değiştirme"
            );
            return Err(AppError::PermissionDenied {
                operation: "ayarları değiştirme".to_owned(),
            });
        }
        if let Err(error) = self.settings.validate() {
            self.settings_screen.settings_error = Some(error.to_string());
            return Err(error);
        }
        let Some(database) = self.database.as_ref() else {
            let error = AppError::Maintenance("Veritabanı açık değil.".to_owned());
            self.settings_screen.settings_error = Some(error.to_string());
            return Err(error);
        };
        if let Err(error) = self.settings.save_general(database) {
            self.settings_screen.settings_error = Some(error.to_string());
            return Err(error);
        }
        crate::logging::set_info_enabled(self.settings.log_info);
        self.settings_screen.settings_error = None;
        self.settings_screen.settings_notice = Some("Ayarlar kaydedildi.".to_owned());
        tracing::info!("genel ayarlar güncellendi");
        Ok(())
    }

    /// Yedekleme/rotasyon ayarlarını kaydeder (yalnızca yönetici).
    pub fn save_backup_settings(&mut self) -> Result<()> {
        if !self.can_administer() {
            tracing::warn!(
                user = %self.actor_log_name(),
                "yetki reddedildi: yedekleme ayarlarını değiştirme"
            );
            return Err(AppError::PermissionDenied {
                operation: "yedekleme ayarlarını değiştirme".to_owned(),
            });
        }
        let Some(database) = self.database.as_ref() else {
            let error = AppError::Maintenance("Veritabanı açık değil.".to_owned());
            self.settings_screen.settings_error = Some(error.to_string());
            return Err(error);
        };
        if let Err(error) = self.settings.save_backup(database) {
            self.settings_screen.settings_error = Some(error.to_string());
            return Err(error);
        }
        self.settings_screen.settings_error = None;
        self.settings_screen.settings_notice = Some("Yedekleme ayarları kaydedildi.".to_owned());
        tracing::info!("yedekleme ayarları güncellendi");
        Ok(())
    }

    /// Kullanıcının istediği manual yedeği alır.
    pub fn create_manual_backup(&mut self) -> Result<PathBuf> {
        if !self.is_authenticated() {
            return Err(AppError::NotAuthenticated);
        }
        let Some(database) = self.database.as_ref() else {
            return Err(AppError::Maintenance("Veritabanı açık değil.".to_owned()));
        };
        let directory = self.config.backups_dir();
        let target = backup::unique_backup_path(&directory, BackupKind::Manual);
        let result = backup::create_backup(database, &target);
        match result {
            Ok(()) => {
                tracing::info!(path = %target.display(), "manuel yedek alındı");
                self.settings_screen.backup_notice =
                    Some(format!("Manuel yedek alındı: {}", target.display()));
                self.settings_screen.backup_error = None;
                self.refresh_backup_list();
                Ok(target)
            }
            Err(error) => {
                tracing::error!(error = %error, "manuel yedek alınamadı");
                self.settings_screen.backup_notice = None;
                self.settings_screen.backup_error = Some(format!("Yedek alınamadı: {error}"));
                Err(error)
            }
        }
    }

    /// Yeni kullanıcı formunu açar.
    pub fn open_new_user_form(&mut self) {
        self.settings_screen.user_form_open = true;
        self.settings_screen.user_form = UserFormState::default();
        self.settings_screen.user_form_error = None;
        self.settings_screen.users_notice = None;
    }

    /// Var olan kullanıcıyı düzenleme formuna yükler.
    pub fn open_edit_user_form(&mut self, id: i64) {
        let Some(user) = self
            .settings_screen
            .users
            .iter()
            .find(|user| user.id == id)
            .cloned()
        else {
            self.settings_screen.user_form_error = Some("Kullanıcı bulunamadı.".to_owned());
            return;
        };
        self.settings_screen.user_form_open = true;
        self.settings_screen.user_form = UserFormState {
            editing_id: Some(user.id),
            username: user.username,
            display_name: user.display_name.unwrap_or_default(),
            role: user.role,
            active: user.active,
            password: String::new(),
            password_confirm: String::new(),
        };
        self.settings_screen.user_form_error = None;
        self.settings_screen.users_notice = None;
    }

    /// Kullanıcı formunu kapatır.
    pub fn cancel_user_form(&mut self) {
        self.settings_screen.user_form_open = false;
        self.settings_screen.user_form = UserFormState::default();
        self.settings_screen.user_form_error = None;
    }

    /// Kullanıcı formunu kaydeder (yeni veya düzenleme).
    pub fn submit_user_form(&mut self) -> Result<()> {
        if !self.can_administer() {
            tracing::warn!(
                user = %self.actor_log_name(),
                "yetki reddedildi: kullanıcı yönetimi"
            );
            return Err(AppError::PermissionDenied {
                operation: "kullanıcı yönetimi".to_owned(),
            });
        }
        let form = self.settings_screen.user_form.clone();
        let Some(database) = self.database.as_ref() else {
            return Err(AppError::Maintenance("Veritabanı açık değil.".to_owned()));
        };

        let result = if let Some(id) = form.editing_id {
            database.user_update(id, Some(form.display_name.as_str()), form.role, form.active)
        } else {
            if form.password != form.password_confirm {
                let error = AppError::Maintenance("Şifreler eşleşmiyor.".to_owned());
                self.settings_screen.user_form_error = Some(error.to_string());
                return Err(error);
            }
            if let Err(error) = validate_password(&form.password) {
                self.settings_screen.user_form_error = Some(error.to_string());
                return Err(error);
            }
            let hash = match hash_password(&form.password) {
                Ok(hash) => hash,
                Err(error) => {
                    self.settings_screen.user_form_error = Some(error.to_string());
                    return Err(error);
                }
            };
            database
                .user_insert(&UserDraft {
                    username: form.username.clone(),
                    display_name: Some(form.display_name.clone()),
                    password_hash: hash,
                    role: form.role,
                    active: form.active,
                })
                .map(|_| ())
        };

        match result {
            Ok(()) => {
                tracing::info!(
                    actor = %self.actor_log_name(),
                    "kullanıcı kaydedildi"
                );
                self.settings_screen.user_form_open = false;
                self.settings_screen.user_form = UserFormState::default();
                self.settings_screen.user_form_error = None;
                self.settings_screen.users_notice = Some("Kullanıcı kaydedildi.".to_owned());
                self.refresh_settings_users();
                Ok(())
            }
            Err(error) => {
                self.settings_screen.user_form_error = Some(error.to_string());
                Err(error)
            }
        }
    }

    /// Kullanıcıyı siler.
    pub fn delete_user(&mut self, id: i64) -> Result<()> {
        let Some(database) = self.database.as_ref() else {
            return Err(AppError::Maintenance("Veritabanı açık değil.".to_owned()));
        };
        match database.user_delete(id) {
            Ok(()) => {
                tracing::info!(user_id = id, "kullanıcı silindi");
                self.settings_screen.users_notice = Some("Kullanıcı silindi.".to_owned());
                self.refresh_settings_users();
                Ok(())
            }
            Err(error) => {
                self.settings_screen.users_error = Some(error.to_string());
                Err(error)
            }
        }
    }

    /// Kullanıcıyı aktif/pasif yapar.
    pub fn toggle_user_active(&mut self, id: i64) -> Result<()> {
        let Some(user) = self
            .settings_screen
            .users
            .iter()
            .find(|user| user.id == id)
            .cloned()
        else {
            return Err(AppError::UserNotFound(id));
        };
        let Some(database) = self.database.as_ref() else {
            return Err(AppError::Maintenance("Veritabanı açık değil.".to_owned()));
        };
        match database.user_update(id, user.display_name.as_deref(), user.role, !user.active) {
            Ok(()) => {
                self.settings_screen.users_notice = Some(if user.active {
                    "Kullanıcı pasife alındı.".to_owned()
                } else {
                    "Kullanıcı aktifleştirildi.".to_owned()
                });
                self.refresh_settings_users();
                Ok(())
            }
            Err(error) => {
                self.settings_screen.users_error = Some(error.to_string());
                Err(error)
            }
        }
    }

    /// Şifre sıfırlama formunu açar.
    pub fn open_reset_user_form(&mut self, id: i64) {
        self.settings_screen.reset_user_id = Some(id);
        self.settings_screen.reset_password.clear();
        self.settings_screen.reset_password_confirm.clear();
        self.settings_screen.users_error = None;
    }

    /// Açık şifre sıfırlama formunu kaydeder.
    pub fn submit_reset_user_form(&mut self) -> Result<()> {
        let Some(id) = self.settings_screen.reset_user_id else {
            return Ok(());
        };
        let password = self.settings_screen.reset_password.clone();
        let confirm = self.settings_screen.reset_password_confirm.clone();
        let result = self.reset_user_password(id, &password, &confirm);
        match &result {
            Ok(()) => {
                self.settings_screen.reset_user_id = None;
                self.settings_screen.reset_password.clear();
                self.settings_screen.reset_password_confirm.clear();
            }
            Err(error) => {
                self.settings_screen.users_error = Some(error.to_string());
            }
        }
        result
    }

    /// Yöneticinin başka bir kullanıcının şifresini sıfırlaması.
    pub fn reset_user_password(
        &mut self,
        id: i64,
        new_password: &str,
        confirm_password: &str,
    ) -> Result<()> {
        if !self.can_administer() {
            tracing::warn!(
                user = %self.actor_log_name(),
                "yetki reddedildi: şifre sıfırlama"
            );
            return Err(AppError::PermissionDenied {
                operation: "şifre sıfırlama".to_owned(),
            });
        }
        if new_password != confirm_password {
            return Err(AppError::Maintenance("Şifreler eşleşmiyor.".to_owned()));
        }
        validate_password(new_password)?;
        let hash = hash_password(new_password)?;
        let Some(database) = self.database.as_ref() else {
            return Err(AppError::Maintenance("Veritabanı açık değil.".to_owned()));
        };
        database.user_set_password_hash(id, &hash)?;
        database.setting_set(&format!("password_reset_required_{id}"), "1")?;
        tracing::info!(
            user_id = id,
            "kullanıcı şifresi yönetici tarafından sıfırlandı"
        );
        self.settings_screen.users_notice =
            Some("Şifre sıfırlandı; kullanıcıya bir sonraki girişte bildirilecek.".to_owned());
        Ok(())
    }

    /// Kendi şifresini değiştirir (form üzerinden).
    pub fn submit_password_change(&mut self) -> Result<()> {
        let form = self.settings_screen.password_form.clone();
        let result = self.change_own_password(&form.current, &form.new_password, &form.confirm);
        match result {
            Ok(()) => {
                self.settings_screen.password_form = PasswordFormState::default();
                self.settings_screen.password_error = None;
                self.settings_screen.password_notice = Some("Şifreniz değiştirildi.".to_owned());
                Ok(())
            }
            Err(error) => {
                self.settings_screen.password_error = Some(error.to_string());
                self.settings_screen.password_notice = None;
                Err(error)
            }
        }
    }

    /// Yedek boyutu uyarısı gösterilmeli mi?
    pub fn backup_size_warning(&self) -> bool {
        self.settings
            .backup_size_exceeds_warning(self.settings_screen.backup_total_bytes)
    }

    /// Operatörün yalnızca Güvenlik sekmesini görmesini sağlar.
    pub fn restrict_settings_for_operator(&mut self) {
        if !self.is_manager() {
            self.settings_screen.tab = SettingsTab::Security;
        }
    }
}
