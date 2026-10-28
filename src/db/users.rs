// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Kullanıcı hesaplarının veri erişimi.
//!
//! Şifre özetleri [`crate::auth`] tarafından üretilir; bu modül yalnızca PHC
//! metnini saklar. Kullanıcı silinebilen tek kayıt türüdür; ancak kendini
//! silme ve son aktif yönetici korumaları veritabanı katmanında uygulanır.

use rusqlite::{OptionalExtension, params};

use super::Database;
use crate::auth::{Actor, Role, User};
use crate::error::{AppError, Result};

/// Yeni kullanıcı oluşturmak için doğrulanmış alanlar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserDraft {
    /// Kullanıcı adı.
    pub username: String,
    /// İsteğe bağlı görünen ad.
    pub display_name: Option<String>,
    /// Argon2id PHC özeti.
    pub password_hash: String,
    /// Rol.
    pub role: Role,
    /// Başlangıçta aktif mi?
    pub active: bool,
}

fn normalize_display_name(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn map_user_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<User> {
    let role: String = row.get(3)?;
    Ok(User {
        id: row.get(0)?,
        username: row.get(1)?,
        password_hash: row.get(2)?,
        role: Role::from_db(&role).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                3,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        display_name: row.get(4)?,
        active: row.get::<_, i64>(5)? != 0,
        last_login_at: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

const USER_COLUMNS: &str = "id, username, password_hash, role, display_name, active, \
                            last_login_at, created_at, updated_at";

impl Database {
    /// Kayıtlı kullanıcı sayısı (ilk yönetici akışı için).
    pub fn user_count(&self) -> Result<usize> {
        let count: i64 = self
            .connection
            .query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))?;
        Ok(count.max(0) as usize)
    }

    /// Aktif yönetici sayısı (son yönetici koruması için).
    pub fn active_admin_count(&self) -> Result<usize> {
        let count: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM users WHERE role = 'yonetici' AND active = 1",
            [],
            |row| row.get(0),
        )?;
        Ok(count.max(0) as usize)
    }

    /// Kullanıcı adıyla kayıt (büyük/küçük harf duyarsız).
    pub fn user_by_username(&self, username: &str) -> Result<Option<User>> {
        let user = self
            .connection
            .query_row(
                &format!("SELECT {USER_COLUMNS} FROM users WHERE username = ?1"),
                [username.trim()],
                map_user_row,
            )
            .optional()?;
        Ok(user)
    }

    /// Birincil anahtarla kullanıcı.
    pub fn user_by_id(&self, id: i64) -> Result<Option<User>> {
        let user = self
            .connection
            .query_row(
                &format!("SELECT {USER_COLUMNS} FROM users WHERE id = ?1"),
                [id],
                map_user_row,
            )
            .optional()?;
        Ok(user)
    }

    /// Tüm kullanıcıları listeler (yalnızca yönetici).
    pub fn user_list(&self) -> Result<Vec<User>> {
        self.require_admin("kullanıcı listesini görüntüleme")?;
        let mut statement = self.connection.prepare(&format!(
            "SELECT {USER_COLUMNS} FROM users ORDER BY active DESC, username COLLATE NOCASE"
        ))?;
        let rows = statement.query_map([], map_user_row)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(AppError::from)
    }

    /// İlk yöneticiyi oluşturur. Yalnızca kullanıcı tablosu boşken çalışır.
    pub fn create_initial_admin(
        &self,
        username: &str,
        display_name: Option<&str>,
        password_hash: &str,
    ) -> Result<User> {
        if self.user_count()? != 0 {
            return Err(AppError::SetupAlreadyCompleted);
        }
        let draft = UserDraft {
            username: username.trim().to_owned(),
            display_name: normalize_display_name(display_name),
            password_hash: password_hash.to_owned(),
            role: Role::Yonetici,
            active: true,
        };
        self.insert_user_row(&draft)
    }

    /// Yeni kullanıcı ekler (yalnızca yönetici).
    pub fn user_insert(&self, draft: &UserDraft) -> Result<User> {
        self.require_admin("kullanıcı ekleme")?;
        self.insert_user_row(draft)
    }

    fn insert_user_row(&self, draft: &UserDraft) -> Result<User> {
        let username = draft.username.trim();
        if username.is_empty() {
            return Err(AppError::InvalidRecord(
                "kullanıcı adı boş olamaz".to_owned(),
            ));
        }
        if self.user_by_username(username)?.is_some() {
            return Err(AppError::UsernameTaken(username.to_owned()));
        }

        self.connection.execute(
            "INSERT INTO users
                 (username, password_hash, role, display_name, active)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                username,
                draft.password_hash,
                draft.role.as_db(),
                draft.display_name,
                i64::from(draft.active),
            ],
        )?;
        let id = self.connection.last_insert_rowid();
        self.user_by_id(id)?.ok_or(AppError::UserNotFound(id))
    }

    /// Görünen ad, rol ve aktifliği günceller (yalnızca yönetici).
    ///
    /// Kendi rolünü düşürme ve son aktif yöneticiyi pasife alma engellenir.
    pub fn user_update(
        &self,
        id: i64,
        display_name: Option<&str>,
        role: Role,
        active: bool,
    ) -> Result<()> {
        self.require_admin("kullanıcı düzenleme")?;
        let current = self.user_by_id(id)?.ok_or(AppError::UserNotFound(id))?;

        if let Some(actor) = self.actor()
            && actor.user_id == id
            && (role != current.role || active != current.active)
        {
            return Err(AppError::SelfOperationProtected);
        }

        let was_active_admin = current.role == Role::Yonetici && current.active;
        let will_be_active_admin = role == Role::Yonetici && active;
        if was_active_admin && !will_be_active_admin && self.active_admin_count()? <= 1 {
            return Err(AppError::LastAdminProtected);
        }

        self.connection.execute(
            "UPDATE users
             SET display_name = ?2, role = ?3, active = ?4, updated_at = datetime('now')
             WHERE id = ?1",
            params![
                id,
                normalize_display_name(display_name),
                role.as_db(),
                i64::from(active),
            ],
        )?;
        Ok(())
    }

    /// Kullanıcıyı kalıcı olarak siler (yalnızca yönetici).
    pub fn user_delete(&self, id: i64) -> Result<()> {
        self.require_admin("kullanıcı silme")?;
        let current = self.user_by_id(id)?.ok_or(AppError::UserNotFound(id))?;

        if let Some(actor) = self.actor()
            && actor.user_id == id
        {
            return Err(AppError::SelfOperationProtected);
        }
        if current.role == Role::Yonetici && current.active && self.active_admin_count()? <= 1 {
            return Err(AppError::LastAdminProtected);
        }

        self.connection
            .execute("DELETE FROM users WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Şifre özetini günceller. Kullanıcı kendi şifresini, yönetici ise
    /// başka bir kullanıcının şifresini değiştirebilir.
    pub fn user_set_password_hash(&self, id: i64, password_hash: &str) -> Result<()> {
        self.require_self_or_admin(id, "şifre değiştirme")?;
        self.user_by_id(id)?.ok_or(AppError::UserNotFound(id))?;
        self.connection.execute(
            "UPDATE users
             SET password_hash = ?2, updated_at = datetime('now')
             WHERE id = ?1",
            params![id, password_hash],
        )?;
        Ok(())
    }

    /// Başarılı girişten sonra son giriş zamanını yazar.
    pub fn user_set_last_login(&self, id: i64) -> Result<()> {
        self.connection.execute(
            "UPDATE users SET last_login_at = datetime('now') WHERE id = ?1",
            [id],
        )?;
        Ok(())
    }

    /// Oturumdaki aktörü döndürür (testler ve loglama için).
    pub fn current_actor(&self) -> Option<Actor> {
        self.actor()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::hash_password;

    fn database_with_admin() -> Database {
        let database = Database::open_in_memory().unwrap();
        let user = database
            .create_initial_admin(
                "admin",
                Some("Yönetici"),
                &hash_password("GucluSifre1").unwrap(),
            )
            .unwrap();
        database.set_actor(Some(user.actor()));
        database
    }

    #[test]
    fn ilk_yonetici_yalnizca_bir_kez_olusturulur() {
        let database = database_with_admin();
        assert_eq!(database.user_count().unwrap(), 1);
        let again =
            database.create_initial_admin("ikinci", None, &hash_password("GucluSifre1").unwrap());
        assert!(matches!(again, Err(AppError::SetupAlreadyCompleted)));
    }

    #[test]
    fn kullanici_adi_harf_duyarsiz_benzersizdir() {
        let database = database_with_admin();
        let draft = UserDraft {
            username: "ADMIN".to_owned(),
            display_name: None,
            password_hash: hash_password("GucluSifre1").unwrap(),
            role: Role::Operator,
            active: true,
        };
        let error = database.user_insert(&draft).unwrap_err();
        assert!(matches!(error, AppError::UsernameTaken(_)));
    }

    #[test]
    fn son_yonetici_silinemez_ve_pasife_alinamaz() {
        let database = database_with_admin();
        let admin = database.user_by_username("admin").unwrap().unwrap();
        assert!(matches!(
            database
                .user_update(admin.id, None, Role::Operator, true)
                .unwrap_err(),
            AppError::SelfOperationProtected
        ));

        database.set_actor(None);
        assert!(matches!(
            database
                .user_update(admin.id, None, Role::Operator, true)
                .unwrap_err(),
            AppError::LastAdminProtected
        ));
        assert!(matches!(
            database.user_delete(admin.id).unwrap_err(),
            AppError::LastAdminProtected
        ));
    }

    #[test]
    fn operator_yonetici_islemlerini_yapamaz() {
        let database = database_with_admin();
        let operator = database
            .user_insert(&UserDraft {
                username: "operator".to_owned(),
                display_name: None,
                password_hash: hash_password("GucluSifre1").unwrap(),
                role: Role::Operator,
                active: true,
            })
            .unwrap();
        database.set_actor(Some(operator.actor()));
        assert!(matches!(
            database.user_list().unwrap_err(),
            AppError::PermissionDenied { .. }
        ));
    }

    #[test]
    fn kullanici_silme_korumalari() {
        let database = database_with_admin();
        let admin = database.user_by_username("admin").unwrap().unwrap();
        assert!(matches!(
            database.user_delete(admin.id).unwrap_err(),
            AppError::SelfOperationProtected
        ));

        database.set_actor(None);
        assert!(matches!(
            database.user_delete(admin.id).unwrap_err(),
            AppError::LastAdminProtected
        ));
    }
}
