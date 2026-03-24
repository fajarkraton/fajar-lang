//! FajarOS regression tests for Fajar Lang compiler.
//!
//! Verifies that the Fajar Lang compiler can lex and parse FajarOS source files.
//! Sprint 7 of Master Implementation Plan v7.0.

use std::path::Path;

fn fajaros_dir() -> String {
    std::env::var("FAJAROS_DIR")
        .unwrap_or_else(|_| "/home/primecore/Documents/fajaros-x86".to_string())
}

fn fajaros_exists() -> bool {
    Path::new(&fajaros_dir()).is_dir()
}

// ════════════════════════════════════════════════════════════════════════
// 1. Repository exists
// ════════════════════════════════════════════════════════════════════════

#[test]
fn fajaros_repo_exists() {
    let dir = fajaros_dir();
    if !fajaros_exists() {
        eprintln!("SKIP: fajaros-x86 not cloned at {dir}");
        return;
    }
    assert!(Path::new(&dir).join("Makefile").exists());
    assert!(Path::new(&dir).join("kernel/main.fj").exists());
}

// ════════════════════════════════════════════════════════════════════════
// 2. All .fj files lex successfully
// ════════════════════════════════════════════════════════════════════════

#[test]
fn fajaros_all_files_lex() {
    if !fajaros_exists() {
        eprintln!("SKIP: fajaros-x86 not cloned");
        return;
    }

    let mut total = 0;
    let mut failed = 0;
    let mut failures = Vec::new();

    for entry in walkdir(&fajaros_dir()) {
        if entry.ends_with(".fj") && !entry.contains("/build/") {
            total += 1;
            let source = match std::fs::read_to_string(&entry) {
                Ok(s) => s,
                Err(_) => {
                    failed += 1;
                    failures.push(format!("READ: {entry}"));
                    continue;
                }
            };
            if fajar_lang::lexer::tokenize(&source).is_err() {
                failed += 1;
                failures.push(format!("LEX: {entry}"));
            }
        }
    }

    assert!(total >= 80, "expected 80+ .fj files, found {total}");
    assert!(
        failed == 0,
        "{failed}/{total} files failed to lex:\n{}",
        failures.join("\n")
    );
}

// ════════════════════════════════════════════════════════════════════════
// 3. Combined.fj parses
// ════════════════════════════════════════════════════════════════════════

#[test]
fn fajaros_combined_parses() {
    let dir = fajaros_dir();
    let combined = format!("{dir}/build/combined.fj");
    if !Path::new(&combined).exists() {
        eprintln!("SKIP: build/combined.fj not found (run `make build` first)");
        return;
    }

    let source = std::fs::read_to_string(&combined).unwrap();
    let tokens = fajar_lang::lexer::tokenize(&source).expect("combined.fj should lex");
    let _program = fajar_lang::parser::parse(tokens).expect("combined.fj should parse");
}

// ════════════════════════════════════════════════════════════════════════
// 4. Key kernel files parse individually
// ════════════════════════════════════════════════════════════════════════

fn lex_file(relative_path: &str) {
    if !fajaros_exists() {
        return;
    }
    let dir = fajaros_dir();
    let path = format!("{dir}/{relative_path}");
    if !Path::new(&path).exists() {
        return;
    }
    let source = std::fs::read_to_string(&path).unwrap();
    fajar_lang::lexer::tokenize(&source)
        .unwrap_or_else(|e| panic!("{relative_path} failed to lex: {e:?}"));
}

#[test]
fn lex_kernel_main() {
    lex_file("kernel/main.fj");
}
#[test]
fn lex_kernel_boot() {
    lex_file("kernel/core/boot.fj");
}
#[test]
fn lex_kernel_mm() {
    lex_file("kernel/core/mm.fj");
}
#[test]
fn lex_kernel_sched() {
    lex_file("kernel/core/sched.fj");
}
#[test]
fn lex_kernel_ipc() {
    lex_file("kernel/core/ipc.fj");
}
#[test]
fn lex_kernel_syscall() {
    lex_file("kernel/core/syscall.fj");
}
#[test]
fn lex_driver_serial() {
    lex_file("drivers/serial.fj");
}
#[test]
fn lex_driver_nvme() {
    lex_file("drivers/nvme.fj");
}
#[test]
fn lex_driver_vga() {
    lex_file("drivers/vga.fj");
}
#[test]
fn lex_driver_keyboard() {
    lex_file("drivers/keyboard.fj");
}
#[test]
fn lex_fs_vfs() {
    lex_file("fs/vfs.fj");
}
#[test]
fn lex_fs_fat32() {
    lex_file("fs/fat32.fj");
}
#[test]
fn lex_shell_commands() {
    lex_file("shell/commands.fj");
}
#[test]
fn lex_service_vfs() {
    lex_file("services/vfs/main.fj");
}
#[test]
fn lex_service_net() {
    lex_file("services/net/main.fj");
}
#[test]
fn lex_service_shell() {
    lex_file("services/shell/main.fj");
}

// ════════════════════════════════════════════════════════════════════════
// 5. File count and LOC tracking
// ════════════════════════════════════════════════════════════════════════

#[test]
fn fajaros_file_count() {
    if !fajaros_exists() {
        return;
    }
    let count = walkdir(&fajaros_dir())
        .iter()
        .filter(|p| p.ends_with(".fj") && !p.contains("/build/"))
        .count();
    assert!(count >= 80, "expected 80+ .fj files, found {count}");
}

#[test]
fn fajaros_loc_count() {
    if !fajaros_exists() {
        return;
    }
    let mut total_loc = 0;
    for entry in walkdir(&fajaros_dir()) {
        if entry.ends_with(".fj") && !entry.contains("/build/") {
            if let Ok(source) = std::fs::read_to_string(&entry) {
                total_loc += source.lines().count();
            }
        }
    }
    assert!(total_loc >= 20000, "expected 20K+ LOC, found {total_loc}");
}

// ════════════════════════════════════════════════════════════════════════
// 6. Manifest compatibility
// ════════════════════════════════════════════════════════════════════════

#[test]
fn fajaros_fj_toml_exists() {
    if !fajaros_exists() {
        return;
    }
    let dir = fajaros_dir();
    assert!(Path::new(&format!("{dir}/fj.toml")).exists());
}

#[test]
fn fajaros_fj_toml_parses() {
    if !fajaros_exists() {
        return;
    }
    let dir = fajaros_dir();
    let path = format!("{dir}/fj.toml");
    let content = std::fs::read_to_string(&path).unwrap();
    // Basic TOML parse (may not have [kernel]/[[service]] yet)
    assert!(content.contains("[project]") || content.contains("[package]"));
}

// ════════════════════════════════════════════════════════════════════════
// Helper: recursive file walker
// ════════════════════════════════════════════════════════════════════════

fn walkdir(dir: &str) -> Vec<String> {
    let mut result = Vec::new();
    walkdir_inner(Path::new(dir), &mut result);
    result
}

fn walkdir_inner(dir: &Path, result: &mut Vec<String>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walkdir_inner(&path, result);
            } else {
                result.push(path.display().to_string());
            }
        }
    }
}
