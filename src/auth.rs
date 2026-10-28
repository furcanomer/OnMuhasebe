// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Kullanıcı, rol ve şifre işlemlerinin ortak iş modeli.
//!
//! Şifreler **asla düz metin saklanmaz**. [`hash_password`] Argon2id ile PHC
//! biçiminde bir özet üretir; [`verify_password`] yalnızca bu özeti doğrular.
//! Giriş denemesi sınırı [`LoginThrottle`] içinde **bellekte** tutulur ve
//! veritabanına yazılmaz.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use argon2::Argon2;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use rand_core::OsRng;

use crate::error::{AppError, Result};

/// Bir kullanıcının uygulama içindeki rolü.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    /// Tüm işlemleri yapabilen yönetici.
    Yonetici,
    /// Kayıt ekleyebilen, iptal/pasife alma ve yönetim işlemlerini yapamayan operatör.
    Operator,
}

impl Role {
    /// Seçim listesinde gösterilecek roller.
    pub const ALL: [Self; 2] = [Self::Yonetici, Self::Operator];

    /// Menüde ve kullanıcı listesinde gösterilen Türkçe etiket.
    pub fn label(self) -> &'static str {
        match self {
            Self::Yonetici => "Yönetici",
            Self::Operator => "Operatör",
        }
    }

    /// Veritabanındaki `role` değeri.
    pub fn as_db(self) -> &'static str {
        match self {
            Self::Yonetici => "yonetici",
            Self::Operator => "operator",
        }
    }

    /// Veritabanı değerini role çevirir.
    pub fn from_db(value: &str) -> Result<Self> {
        match value {
            "yonetici" => Ok(Self::Yonetici),
            "operator" => Ok(Self::Operator),
            other => Err(AppError::InvalidRecord(format!(
                "bilinmeyen kullanıcı rolü: {other}"
            ))),
        }
    }

    /// Yönetici yetkisi gerektiren işlemler için kısa kontrol.
    pub fn is_manager(self) -> bool {
        self == Self::Yonetici
    }
}

/// Veritabanından okunan kullanıcı kaydı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    /// Birincil anahtar.
    pub id: i64,
    /// Büyük/küçük harf duyarsız benzersiz kullanıcı adı.
    pub username: String,
    /// Argon2id PHC biçimindeki şifre özeti.
    pub password_hash: String,
    /// Kullanıcı rolü.
    pub role: Role,
    /// İsteğe bağlı görünen ad.
    pub display_name: Option<String>,
    /// Hesap aktif mi?
    pub active: bool,
    /// Son başarılı giriş zamanı (SQLite UTC metni).
    pub last_login_at: Option<String>,
    /// Oluşturulma zamanı.
    pub created_at: String,
    /// Son güncelleme zamanı.
    pub updated_at: String,
}

impl User {
    /// Listelerde gösterilecek ad: görünen ad varsa o, yoksa kullanıcı adı.
    pub fn label(&self) -> &str {
        self.display_name
            .as_deref()
            .filter(|name| !name.trim().is_empty())
            .unwrap_or(&self.username)
    }

    /// Arayüzde kullanılacak oturum kimliği.
    pub fn actor(&self) -> Actor {
        Actor {
            user_id: self.id,
            username: self.username.clone(),
            display_name: self.display_name.clone(),
            role: self.role,
        }
    }
}

/// Oturum açmış kullanıcının veritabanı katmanına taşınan kimliği.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actor {
    /// Kullanıcı birincil anahtarı.
    pub user_id: i64,
    /// Kullanıcı adı.
    pub username: String,
    /// Görünen ad (varsa).
    pub display_name: Option<String>,
    /// Yetki kontrolünde kullanılan rol.
    pub role: Role,
}

impl Actor {
    /// Listelerde gösterilecek ad.
    pub fn label(&self) -> &str {
        self.display_name
            .as_deref()
            .filter(|name| !name.trim().is_empty())
            .unwrap_or(&self.username)
    }

    /// Yönetici mi?
    pub fn is_manager(&self) -> bool {
        self.role.is_manager()
    }
}

/// Şifrenin en az uzunluğu.
pub const MIN_PASSWORD_LENGTH: usize = 8;

/// Şifre kurallarını doğrular: en az 8 karakter, en az bir harf, en az bir rakam.
pub fn validate_password(password: &str) -> Result<()> {
    let long_enough = password.chars().count() >= MIN_PASSWORD_LENGTH;
    let has_letter = password.chars().any(char::is_alphabetic);
    let has_digit = password.chars().any(char::is_numeric);

    if long_enough && has_letter && has_digit {
        Ok(())
    } else {
        Err(AppError::InvalidPassword)
    }
}

/// Şifreyi Argon2id ile PHC biçiminde özetler.
pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| AppError::PasswordHash(error.to_string()))
}

/// Düz metin şifreyi PHC özetiyle doğrular.
pub fn verify_password(password_hash: &str, password: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(password_hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

/// Argon2id PHC biçimini (algoritma parametreleri ve tuz) doğrular.
pub fn is_argon2id_hash(value: &str) -> bool {
    PasswordHash::new(value)
        .map(|parsed| parsed.algorithm.as_str().eq_ignore_ascii_case("argon2id"))
        .unwrap_or(false)
}

/// Loglarda kullanıcı adını `a***n` biçiminde maskeler.
pub fn mask_username(username: &str) -> String {
    let chars: Vec<char> = username.chars().collect();
    match chars.len() {
        0 => "***".to_owned(),
        1 => "***".to_owned(),
        _ => format!("{}***{}", chars[0], chars[chars.len() - 1]),
    }
}

#[derive(Debug, Clone, Copy)]
struct FailureState {
    count: u32,
    locked_until: Option<Instant>,
}

/// Aynı kullanıcı için hatalı girişleri sayan bellek içi sınırlayıcı.
///
/// Varsayılan politika: 5 hatalı denemeden sonra 30 saniye bekleme. Bekleme
/// süresi dolunca sayaç sıfırlanır. Veritabanına hiçbir şey yazılmaz.
#[derive(Debug, Clone)]
pub struct LoginThrottle {
    failures: HashMap<String, FailureState>,
    max_attempts: u32,
    lockout: Duration,
}

impl Default for LoginThrottle {
    fn default() -> Self {
        Self::with_policy(5, Duration::from_secs(30))
    }
}

impl LoginThrottle {
    /// Testler ve yapılandırma için özel politika.
    pub fn with_policy(max_attempts: u32, lockout: Duration) -> Self {
        Self {
            failures: HashMap::new(),
            max_attempts: max_attempts.max(1),
            lockout,
        }
    }

    fn key(username: &str) -> String {
        username.trim().to_lowercase()
    }

    /// Giriş denenebilir mi? Kilitliyse kalan saniyeyi döndürür.
    pub fn check(&mut self, username: &str, now: Instant) -> std::result::Result<(), u64> {
        let key = Self::key(username);
        let Some(state) = self.failures.get_mut(&key) else {
            return Ok(());
        };

        if let Some(until) = state.locked_until {
            if now < until {
                let remaining = until.saturating_duration_since(now);
                return Err(remaining.as_secs().max(1));
            }
            // Süre doldu: sayaç sıfırlanır ve normal girişe dönülür.
            self.failures.remove(&key);
        }
        Ok(())
    }

    /// Hatalı bir denemeyi kaydeder; eşik aşıldıysa kalan saniyeyi döndürür.
    pub fn record_failure(&mut self, username: &str, now: Instant) -> Option<u64> {
        let key = Self::key(username);
        let state = self.failures.entry(key).or_insert(FailureState {
            count: 0,
            locked_until: None,
        });
        state.count = state.count.saturating_add(1);

        if state.count >= self.max_attempts {
            let until = now.checked_add(self.lockout).unwrap_or(now);
            state.locked_until = Some(until);
            state.count = self.max_attempts;
            Some(self.lockout.as_secs().max(1))
        } else {
            None
        }
    }

    /// Başarılı girişte sayaç silinir.
    pub fn record_success(&mut self, username: &str) {
        self.failures.remove(&Self::key(username));
    }

    /// Test/ayar değişikliği için tüm sayaçları temizler.
    pub fn clear(&mut self) {
        self.failures.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sifre_kurallari_turkce_karakterleri_kabul_eder() {
        assert!(validate_password("gucluSifre1").is_ok());
        assert!(validate_password("Şifre123").is_ok());
        assert!(validate_password("kisa1").is_err());
        assert!(validate_password("sadeceharf").is_err());
        assert!(validate_password("12345678").is_err());
    }

    #[test]
    fn argon2id_ozeti_uretilir_ve_dogrulanir() {
        let hash = hash_password("GucluSifre1").expect("özet üretilmeli");
        assert!(is_argon2id_hash(&hash), "Argon2id PHC bekleniyordu: {hash}");
        assert!(!hash.contains("GucluSifre1"));
        assert!(verify_password(&hash, "GucluSifre1"));
        assert!(!verify_password(&hash, "yanlis"));
    }

    #[test]
    fn ayni_sifre_icin_farkli_tuz_ve_ozet_uretilir() {
        let first = hash_password("GucluSifre1").unwrap();
        let second = hash_password("GucluSifre1").unwrap();
        assert_ne!(first, second, "her özet kendi tuzunu kullanmalı");
    }

    #[test]
    fn kullanici_adi_log_icin_maskelenir() {
        assert_eq!(mask_username("admin"), "a***n");
        assert_eq!(mask_username("a"), "***");
        assert_eq!(mask_username(""), "***");
    }

    #[test]
    fn bes_hatali_denemeden_sonra_kilit_ve_sifirlama() {
        let now = Instant::now();
        let mut throttle = LoginThrottle::with_policy(5, Duration::from_secs(30));
        assert!(throttle.check("admin", now).is_ok());

        for _ in 0..4 {
            assert_eq!(throttle.record_failure("admin", now), None);
            assert!(throttle.check("admin", now).is_ok());
        }
        assert_eq!(throttle.record_failure("admin", now), Some(30));
        let remaining = throttle.check("admin", now).expect_err("kilitli olmalı");
        assert_eq!(remaining, 30);

        // 30 saniye sonra sayaç sıfırlanır ve yeniden denenebilir.
        let later = now + Duration::from_secs(31);
        assert!(throttle.check("admin", later).is_ok());
        assert_eq!(throttle.record_failure("admin", later), None);
    }
}
