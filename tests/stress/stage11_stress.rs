// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Ağır testler: büyük kullanıcı listesi ve sanal aralık hesabı.
//!
//! Bu testler `#[ignore]` işaretlidir; `cargo test --release -- --ignored
//! --test-threads=1` ile çalıştırılır.

use std::time::Instant;

use on_muhasebe::ui::virtual_list::visible_range;

use super::{build_test_db, raw_connection};

/// 10.000 kullanıcı yalnızca liste ve sanal aralık üzerinden doğrulanır;
/// Argon2 hesaplaması yapılmaz (hash alanı dolu bir yer tutucudur).
#[test]
#[ignore = "ağır test: cargo test --release -- --ignored"]
fn on_bin_kullanici_listesi_ve_sanal_aralik() {
    let directory = tempfile::tempdir().expect("geçici dizin");
    let path = directory.path().join("kullanicilar.db");
    let database = build_test_db(&path);

    let start = Instant::now();
    {
        let connection = raw_connection(&database);
        connection
            .execute_batch(
                "WITH RECURSIVE sayac(no) AS (
                     VALUES(1)
                     UNION ALL
                     SELECT no + 1 FROM sayac WHERE no < 10000
                 )
                 INSERT INTO users (username, password_hash, role, active)
                 SELECT printf('kullanici-%05d', no), '$argon2id$yer-tutucu', 'operator', 1
                 FROM sayac;",
            )
            .expect("10.000 kullanıcı eklenmeli");
    }
    let seed_ms = start.elapsed().as_millis();

    let start = Instant::now();
    let users = database.user_list().expect("kullanıcı listesi");
    let list_ms = start.elapsed().as_millis();
    assert_eq!(users.len(), 10_000);
    assert_eq!(users.first().unwrap().username, "kullanici-00001");
    assert_eq!(users.last().unwrap().username, "kullanici-10000");

    let range = visible_range(users.len(), 44.0, 44.0 * 5_000.0, 450.0, 3);
    assert_eq!(range.start, 4_997);
    assert_eq!(range.end, 5_014);
    assert!(
        range.len() < 30,
        "10.000 kullanıcıda yalnızca görünür satırlar çizilmeli: {range:?}"
    );
    assert!(
        seed_ms < 15_000,
        "10.000 kullanıcı toplu ekleme 15 sn'yi aştı: {seed_ms} ms"
    );
    assert!(
        list_ms < 2_000,
        "10.000 kullanıcı listesi 2 sn'yi aştı: {list_ms} ms"
    );
}

/// 1.000.000 cari satırında kaydırma konumundan bağımsız olarak yalnızca
/// küçük bir görünür aralık hesaplanır.
#[test]
#[ignore = "ağır test: cargo test --release -- --ignored"]
fn bir_milyon_cari_sanal_aralik() {
    let start = Instant::now();
    for offset in [0.0, 92.0 * 250_000.0, 92.0 * 999_999.0] {
        let range = visible_range(1_000_000, 92.0, offset, 800.0, 5);
        assert!(!range.is_empty(), "aralık boş olmamalı: {offset}");
        assert!(
            range.len() < 30,
            "1M cari için görünür aralık küçük olmalı: {range:?}"
        );
        assert!(range.end <= 1_000_000);
    }
    assert_eq!(
        visible_range(1_000_000, 92.0, 92.0 * 999_999.0, 800.0, 5).end,
        1_000_000
    );
    assert!(
        start.elapsed().as_millis() < 100,
        "sanal aralık hesabı 100 ms'yi aştı"
    );
}
