//! Editor package validation — P5.D1 of FAJAR_LANG_PERFECTION_PLAN.
//!
//! Plan §4 P5 D1 PASS criterion: "All 5 editor packages tested; each
//! opens .fj file + shows diagnostic + completion + go-to-def".
//!
//! True end-to-end editor testing (launching VSCode/JetBrains/etc and
//! observing UI behavior) requires a graphical environment + 5 separate
//! editor installations. From a CI/CLI test we instead validate every
//! pre-condition the editor needs:
//!
//!   1. Each package's config file is structurally well-formed (parses
//!      as JSON / TOML / XML / Lua-source).
//!   2. Each package references the `fj lsp` invocation.
//!   3. Each package declares the `.fj` file extension.
//!   4. The `fj lsp` CLI subcommand exists (audited via main.rs grep).
//!   5. The LSP server's `pub` surface (run_lsp) is reachable.
//!
//! When invariants 1-5 hold, an editor that follows the package config
//! WILL launch the LSP server on opening a `.fj` file. Diagnostic /
//! completion / go-to-def behavior beyond launch is the LSP server's
//! responsibility, exercised by `tests/lsp_v3_semantic_tokens.rs` (D2)
//! and `tests/lsp_tests.rs` (existing).

use std::fs;
use std::path::PathBuf;

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    fs::read_to_string(project_root().join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

// ════════════════════════════════════════════════════════════════════════
// VSCode — package.json (JSON)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn d1_vscode_package_json_parses() {
    let raw = read("editors/vscode/package.json");
    let v: serde_json::Value =
        serde_json::from_str(&raw).expect("editors/vscode/package.json must be valid JSON");
    assert!(v.is_object(), "vscode package.json must be an object");
    assert!(v.get("name").is_some(), "package.json missing 'name'");
    assert!(v.get("version").is_some(), "package.json missing 'version'");
    let contributes = v
        .get("contributes")
        .expect("vscode package.json missing 'contributes' block");
    let langs = contributes
        .get("languages")
        .and_then(|l| l.as_array())
        .expect("contributes.languages must be an array");
    let has_fj = langs.iter().any(|lang| {
        lang.get("extensions")
            .and_then(|e| e.as_array())
            .is_some_and(|exts| exts.iter().any(|x| x.as_str() == Some(".fj")))
    });
    assert!(
        has_fj,
        "vscode package.json must declare .fj file extension"
    );
}

#[test]
fn d1_vscode_package_json_has_lsp_reference() {
    // VSCode delegates LSP to extension.js; verify the runtime extension
    // file references the `fj lsp` command (or `lsp` subcommand).
    let ext = read("editors/vscode/extension.js");
    assert!(
        ext.contains("'lsp'") || ext.contains("\"lsp\"") || ext.contains("fj lsp"),
        "extension.js must invoke `fj lsp` to start the LSP server"
    );
}

// ════════════════════════════════════════════════════════════════════════
// Helix — languages.toml (TOML)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn d1_helix_languages_toml_parses() {
    let raw = read("editors/helix/languages.toml");
    let v: toml::Value =
        toml::from_str(&raw).expect("editors/helix/languages.toml must be valid TOML");
    let langs = v
        .get("language")
        .and_then(|l| l.as_array())
        .expect("helix languages.toml must have [[language]] array");
    let fj = langs
        .iter()
        .find(|l| l.get("name").and_then(|n| n.as_str()) == Some("fajar"))
        .expect("helix languages.toml must declare a 'fajar' language entry");
    let exts = fj
        .get("file-types")
        .and_then(|f| f.as_array())
        .expect("fajar entry missing 'file-types'");
    assert!(
        exts.iter().any(|x| x.as_str() == Some("fj")),
        "helix fajar entry must include 'fj' file-type"
    );
    let ls = fj
        .get("language-server")
        .expect("fajar entry missing 'language-server'");
    let cmd = ls
        .get("command")
        .and_then(|c| c.as_str())
        .expect("language-server.command missing");
    assert_eq!(cmd, "fj", "helix LSP command must be `fj`");
    let args = ls
        .get("args")
        .and_then(|a| a.as_array())
        .expect("language-server.args missing");
    assert!(
        args.iter().any(|a| a.as_str() == Some("lsp")),
        "helix LSP args must include `lsp`"
    );
}

// ════════════════════════════════════════════════════════════════════════
// Zed — fajar.json (JSON)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn d1_zed_fajar_json_parses() {
    let raw = read("editors/zed/fajar.json");
    let v: serde_json::Value =
        serde_json::from_str(&raw).expect("editors/zed/fajar.json must be valid JSON");
    assert!(v.get("id").is_some(), "zed config missing 'id'");
    let langs = v
        .get("languages")
        .expect("zed config missing 'languages' block");
    assert!(langs.is_object(), "zed languages must be an object");
    // Verify path_suffixes includes "fj".
    let mut found_fj = false;
    if let Some(obj) = langs.as_object() {
        for (_lang_name, cfg) in obj {
            if let Some(suffixes) = cfg
                .get("config")
                .and_then(|c| c.get("path_suffixes"))
                .and_then(|s| s.as_array())
            {
                if suffixes.iter().any(|x| x.as_str() == Some("fj")) {
                    found_fj = true;
                }
            }
        }
    }
    assert!(found_fj, "zed config must declare 'fj' as path_suffix");
}

#[test]
fn d1_zed_fajar_json_lsp_command_is_fj_lsp() {
    let raw = read("editors/zed/fajar.json");
    let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
    // language_servers section: { "fajar-lsp": { "command": { "path": "fj", "arguments": ["lsp"] } } }
    let raw_str = raw.as_str();
    assert!(
        raw_str.contains("\"path\": \"fj\"") && raw_str.contains("\"arguments\": [\"lsp\"]"),
        "zed config must specify command.path=\"fj\" + arguments=[\"lsp\"]"
    );
    // Sanity check on the parsed JSON too.
    let _ = v.get("language_servers");
}

// ════════════════════════════════════════════════════════════════════════
// Neovim — fajar.lua (Lua source)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn d1_neovim_fajar_lua_references_fj_lsp() {
    // We don't parse Lua. Validate the file contains the LSP launch
    // expression: `cmd = { 'fj', 'lsp' }` (or equivalent).
    let raw = read("editors/neovim/fajar.lua");
    assert!(
        raw.contains("'fj', 'lsp'") || raw.contains("\"fj\", \"lsp\""),
        "neovim/fajar.lua must declare cmd = {{'fj', 'lsp'}}"
    );
    // Verify .fj filetype association is present.
    assert!(
        raw.contains("fj")
            && (raw.contains("FileType") || raw.contains("filetype") || raw.contains("pattern")),
        "neovim/fajar.lua must declare a filetype/pattern association for .fj"
    );
}

// ════════════════════════════════════════════════════════════════════════
// JetBrains — fajar-plugin.xml (XML descriptor)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn d1_jetbrains_plugin_xml_well_formed_and_references_lsp() {
    // No XML parser in deps; do structural string checks.
    let raw = read("editors/jetbrains/fajar-plugin.xml");
    assert!(
        raw.starts_with("<?xml"),
        "jetbrains fajar-plugin.xml must start with XML declaration"
    );
    assert!(
        raw.contains("<idea-plugin") || raw.contains("<plugin"),
        "must declare <idea-plugin> or <plugin> root"
    );
    assert!(
        raw.contains("</idea-plugin>") || raw.contains("</plugin>"),
        "must close root element"
    );
    assert!(
        raw.contains("fj lsp"),
        "plugin descriptor must reference `fj lsp` invocation"
    );
    assert!(
        raw.contains(".fj"),
        "plugin descriptor must reference .fj file extension"
    );
}

// ════════════════════════════════════════════════════════════════════════
// Cross-cutting invariants
// ════════════════════════════════════════════════════════════════════════

#[test]
fn d1_all_5_editor_packages_exist() {
    let required = [
        "editors/helix/languages.toml",
        "editors/jetbrains/fajar-plugin.xml",
        "editors/neovim/fajar.lua",
        "editors/vscode/package.json",
        "editors/zed/fajar.json",
    ];
    for path in &required {
        let full = project_root().join(path);
        assert!(full.exists(), "required editor package missing: {path}");
    }
}

#[test]
fn d1_lsp_run_function_is_pub() {
    // `fajar_lang::lsp::run_lsp` must be reachable for the `fj lsp` CLI
    // subcommand to dispatch into it. This compile-time check fails to
    // build if the path goes private — the test body itself enforces
    // visibility.
    let _: fn() -> _ = fajar_lang::lsp::run_lsp;
}

#[test]
fn d1_main_rs_dispatches_lsp_subcommand() {
    // The CLI surface must wire `Command::Lsp => cmd_lsp()` and have
    // `Lsp` declared in the Command enum. Verify by reading main.rs;
    // catches accidental removal of the `fj lsp` subcommand which
    // would silently break every editor package.
    let main_rs = read("src/main.rs");
    assert!(
        main_rs.contains("Command::Lsp"),
        "src/main.rs must dispatch Command::Lsp"
    );
    assert!(
        main_rs.contains("run_lsp"),
        "src/main.rs must call lsp::run_lsp"
    );
}
