// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Gerçek GUI geri yükleme kesinti koruması (`restore.journal`).

use std::path::{Path, PathBuf};

use on_muhasebe::db::Database;
use on_muhasebe::restore_journal::{self, RestoreJournal, RestoreStep, apply_safety_backup};
fn open_database(directory: &Path) -> (PathBuf, Database) {
    let path = directory.join("on_muhasebe.db");
    let database = Database::open(&path).expect("veritabanı açılmalı");
    (path, database)
}

fn safety_backup(database: &Database, path: &Path) {
    database.checkpoint().expect("checkpoint");
    std::fs::copy(database.path().expect("dosya veritabanı"), path)
        .expect("güvenlik yedeği kopyalanmalı");
}

fn journal_for(
    directory: &Path,
    database_path: &Path,
    safety: &Path,
    step: RestoreStep,
) -> (PathBuf, RestoreJournal) {
    let mut journal = RestoreJournal::new(
        directory.join("kaynak.onm-backup"),
        safety.to_path_buf(),
        database_path.to_path_buf(),
    );
    journal.step = step;
    let journal_path = RestoreJournal::path(directory);
    journal.write(&journal_path).expect("günlük yazılmalı");
    (journal_path, journal)
}

#[test]
fn yarida_kalan_restore_tespit_edilir_ve_mevcut_db_acilir() {
    let directory = tempfile::tempdir().expect("geçici dizin");
    let (database_path, database) = open_database(directory.path());
    let safety = directory.path().join("on_muhasebe.db.pre-restore-test");
    safety_backup(&database, &safety);
    let (journal_path, _journal) = journal_for(
        directory.path(),
        &database_path,
        &safety,
        RestoreStep::Copying,
    );

    // Veritabanı yine de açılabilmeli; yarıda kalan işlem otomatik uygulanmaz.
    let reopened = Database::open(&database_path).expect("mevcut veritabanı açılmalı");
    assert_eq!(reopened.schema_version().expect("şema"), 7);
    assert!(
        journal_path.exists(),
        "günlük kullanıcı kararı verilmeden silinmemeli"
    );

    let detected = restore_journal::detect(directory.path())
        .expect("günlük okunmalı")
        .expect("yarıda kalan restore tespit edilmeli");
    assert_eq!(detected.step, RestoreStep::Copying);
    assert!(detected.is_interrupted());
    assert_eq!(detected.target_db, database_path);
}

#[test]
fn done_adimindaki_journal_sessizce_silinir() {
    let directory = tempfile::tempdir().expect("geçici dizin");
    let (database_path, database) = open_database(directory.path());
    let safety = directory.path().join("on_muhasebe.db.pre-restore-done");
    safety_backup(&database, &safety);
    let (journal_path, _journal) =
        journal_for(directory.path(), &database_path, &safety, RestoreStep::Done);

    let detected = restore_journal::detect(directory.path()).expect("günlük okunmalı");
    assert!(detected.is_none(), "done adımı uyarı üretmemeli");
    assert!(!journal_path.exists(), "done günlüğü silinmeli");
}

#[test]
fn journal_yoksa_uyari_uretmez() {
    let directory = tempfile::tempdir().expect("geçici dizin");
    let (database_path, _database) = open_database(directory.path());

    let detected = restore_journal::detect(directory.path()).expect("denetim başarılı");
    assert!(detected.is_none());
    assert!(!RestoreJournal::path(directory.path()).exists());
    assert!(database_path.exists());
}

#[test]
fn guvenlik_yedegine_don_veriyi_kurtarir() {
    let directory = tempfile::tempdir().expect("geçici dizin");
    let (database_path, database) = open_database(directory.path());
    database
        .set_metadata("stage14_marker", "once")
        .expect("işaret yazılmalı");

    let safety = directory.path().join("on_muhasebe.db.pre-restore-recover");
    safety_backup(&database, &safety);
    database
        .set_metadata("stage14_marker", "sonra")
        .expect("işaret güncellenmeli");
    database.checkpoint().expect("checkpoint");
    drop(database);

    let (journal_path, journal) = journal_for(
        directory.path(),
        &database_path,
        &safety,
        RestoreStep::Reopening,
    );
    assert_eq!(
        restore_journal::detect(directory.path())
            .expect("günlük okunmalı")
            .expect("restore tespit edilmeli")
            .step,
        RestoreStep::Reopening
    );

    apply_safety_backup(&journal).expect("güvenlik yedeği geri kopyalanmalı");
    RestoreJournal::remove(&journal_path).expect("günlük silinmeli");

    let recovered = Database::open(&database_path).expect("kurtarılan veritabanı açılmalı");
    assert_eq!(
        recovered.metadata("stage14_marker").expect("meta okunmalı"),
        Some("once".to_owned()),
        "güvenlik yedeğindeki veri korunmalı"
    );
    assert!(
        restore_journal::detect(directory.path())
            .expect("denetim")
            .is_none()
    );
}
