// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, @furcanomer

//! Aşama 14: BSD 3-Clause lisansı, @furcanomer telif sahibi ve exe metadata.

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(relative: &str) -> String {
    std::fs::read_to_string(root().join(relative))
        .unwrap_or_else(|error| panic!("{relative} okunamadı: {error}"))
}

fn has_mit_token(text: &str) -> bool {
    text.split(|character: char| !character.is_alphanumeric())
        .any(|token| token.eq_ignore_ascii_case("mit"))
}

fn source_files(directory: &Path, output: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if path.is_dir() {
            if matches!(name, "target" | "dist" | ".git") {
                continue;
            }
            source_files(&path, output);
        } else if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("rs" | "toml" | "md" | "rtf" | "wxs")
        ) {
            // Denetçinin kendisi "MIT" sözcüğünü test metninde kullanır.
            if name == "stage14_license.rs" {
                continue;
            }
            output.push(path);
        }
    }
}

#[test]
fn lisans_bsd_3_clause_ve_telif_furcanomer() {
    let license = read("LICENSE");
    assert!(license.starts_with("BSD 3-Clause License"));
    assert!(license.contains("Copyright (c) 2026, @furcanomer"));
    assert!(!license.contains("MIT License"));
}

#[test]
fn cargo_paket_bilgileri_bsd_ve_furcanomer() {
    let manifest = read("Cargo.toml");
    assert!(manifest.contains("license = \"BSD-3-Clause\""));
    assert!(manifest.contains("authors = [\"@furcanomer\"]"));
    assert!(manifest.contains("https://github.com/furcanomer/OnMuhasebe"));
    assert!(manifest.contains("winres = \"0.1\""));
}

#[test]
fn readme_ve_hakkinda_metni_bsd_ve_gelistirici_icerir() {
    let readme = read("README.md");
    assert!(readme.contains("BSD 3-Clause"));
    assert!(readme.contains("@furcanomer"));
    assert!(!readme.contains("MIT Lisansı"));

    let about = read("src/ui/layout.rs");
    assert!(about.contains("BSD 3-Clause Lisansı ile açık kaynak"));
    assert!(about.contains("Geliştirici: @furcanomer"));
}

#[test]
fn kaynakta_mit_referansi_kalmaz() {
    let mut files = Vec::new();
    source_files(&root().join("src"), &mut files);
    source_files(&root().join("tests"), &mut files);
    for file in ["README.md", "Cargo.toml", "LICENSE"] {
        files.push(root().join(file));
    }
    for file in ["wix/main.wxs", "wix/LICENSE.rtf", "wix/YENI_SURUM.md"] {
        files.push(root().join(file));
    }

    for file in files {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        assert!(
            !has_mit_token(&text),
            "MIT referansı kalmamalı: {}",
            file.display()
        );
    }
}

#[cfg(windows)]
#[test]
fn exe_metadata_furcanomer_icerir() {
    let candidates = [
        root().join("target/release/OnMuhasebe.exe"),
        root().join("target/debug/OnMuhasebe.exe"),
    ];
    let Some(exe) = candidates.iter().find(|path| path.is_file()) else {
        eprintln!("exe bulunamadı; metadata testi release derlemesinden sonra çalıştırılmalı");
        return;
    };

    let script = format!(
        "(Get-Item -LiteralPath '{}').VersionInfo | Format-List CompanyName,LegalCopyright,ProductName,ProductVersion | Out-String",
        exe.display()
    );
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .output()
        .expect("PowerShell çalıştırılmalı");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "PowerShell başarısız: {text}");
    assert!(
        text.contains("@furcanomer"),
        "CompanyName/LegalCopyright @furcanomer içermeli: {text}"
    );
    assert!(
        text.to_lowercase().contains("muhasebe"),
        "ProductName Ön Muhasebe içermeli: {text}"
    );
}
