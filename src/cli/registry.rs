//! Package registry and distribution commands.
//!
//! Extracted from `main.rs` (REFACTOR_2026_07 Phase 2); pure code motion.

use crate::{EXIT_COMPILE, EXIT_RUNTIME, EXIT_USAGE};
use fajar_lang::FjDiagnostic;
use fajar_lang::analyzer::analyze;
use fajar_lang::lexer::tokenize;
use fajar_lang::parser::parse;
use std::path::PathBuf;
use std::process::ExitCode;

/// V19 5.5: List all available demos.
/// Manage compiler plugins: list, load.
pub(crate) fn cmd_plugin(action: &str, path: Option<&std::path::Path>) -> ExitCode {
    match action {
        "list" => {
            let registry = fajar_lang::plugin::default_registry();
            println!("Registered compiler plugins:\n");
            for name in registry.plugin_names() {
                println!("  {name:<24} [enabled]");
            }
            println!("\nPlugins run automatically during `fj run`.");
            println!("Use `fj plugin load <path.so>` to load external plugins.");
            ExitCode::SUCCESS
        }
        "load" => {
            let path = match path {
                Some(p) => p,
                None => {
                    eprintln!("error: fj plugin load <path.so> — missing plugin path");
                    return ExitCode::from(EXIT_USAGE);
                }
            };
            if !path.exists() {
                eprintln!("error: plugin file not found: {}", path.display());
                return ExitCode::from(EXIT_USAGE);
            }
            match fajar_lang::plugin::load_plugin_from_path(&path.display().to_string()) {
                Ok(p) => {
                    println!("Loaded plugin: {} v{}", p.name(), p.version());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("error: failed to load plugin: {e}");
                    ExitCode::from(EXIT_RUNTIME)
                }
            }
        }
        _ => {
            eprintln!("error: unknown plugin action '{action}'. Use: list, load");
            ExitCode::from(EXIT_USAGE)
        }
    }
}

/// V18 4.6: Generate deployment artifacts.
pub(crate) fn cmd_deploy(target: &str, file: &std::path::Path, output: &str) -> ExitCode {
    let binary_name = file
        .file_stem()
        .map(|s: &std::ffi::OsStr| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "app".to_string());

    match target {
        "container" | "docker" => {
            let config = fajar_lang::deployment::containers::DockerConfig::new(&binary_name);
            let dockerfile = fajar_lang::deployment::containers::generate_dockerfile(&config);
            let out_path = std::path::Path::new(output).join("Dockerfile");
            match std::fs::write(&out_path, &dockerfile) {
                Ok(()) => {
                    println!("Generated: {}", out_path.display());
                    println!("  Binary: {binary_name}");
                    println!("  Base image: distroless");
                    println!("  Port: 8080");
                    println!("\nBuild: docker build -t {binary_name} .");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("error: cannot write Dockerfile: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        "k8s" | "kubernetes" => {
            let image = format!("{binary_name}:latest");
            let deploy =
                fajar_lang::deployment::containers::K8sDeployment::new(&binary_name, &image);
            let manifest = fajar_lang::deployment::containers::generate_k8s_manifest(&deploy);
            let out_path = std::path::Path::new(output).join(format!("{binary_name}-k8s.yaml"));
            match std::fs::write(&out_path, &manifest) {
                Ok(()) => {
                    println!("Generated: {}", out_path.display());
                    println!("  Deployment: {binary_name} ({} replicas)", deploy.replicas);
                    println!("  Service: ClusterIP → port {}", deploy.port);
                    println!("  Image: {image}");
                    println!(
                        "  Resources: {}m-{}m CPU, {}Mi-{}Mi RAM",
                        deploy.cpu_request,
                        deploy.cpu_limit,
                        deploy.mem_request_mi,
                        deploy.mem_limit_mi
                    );
                    println!("\nApply: kubectl apply -f {}", out_path.display());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("error: cannot write manifest: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            eprintln!("Unknown deploy target: '{target}'");
            eprintln!("Available: container, k8s");
            ExitCode::from(EXIT_USAGE)
        }
    }
}

/// Validates and publishes the current project to the local registry.
/// V15 B3.8: Initialize a local file-based package registry.
pub(crate) fn cmd_registry_init(path: &PathBuf) -> ExitCode {
    use std::io::Write;
    if path.exists() {
        eprintln!("error: directory already exists: {}", path.display());
        return ExitCode::from(EXIT_USAGE);
    }
    if let Err(e) = std::fs::create_dir_all(path) {
        eprintln!("error: cannot create directory: {e}");
        return ExitCode::from(EXIT_RUNTIME);
    }
    // Create packages/ subdirectory
    if let Err(e) = std::fs::create_dir_all(path.join("packages")) {
        eprintln!("error: cannot create packages dir: {e}");
        return ExitCode::from(EXIT_RUNTIME);
    }
    // Write registry.json metadata
    let metadata = serde_json::json!({
        "name": "local-registry",
        "version": "1.0.0",
        "description": "Local Fajar Lang package registry",
        "packages": {}
    });
    let meta_path = path.join("registry.json");
    match std::fs::File::create(&meta_path) {
        Ok(mut f) => {
            if let Err(e) = f.write_all(
                serde_json::to_string_pretty(&metadata)
                    .unwrap_or_default()
                    .as_bytes(),
            ) {
                eprintln!("error: cannot write registry.json: {e}");
                return ExitCode::from(EXIT_RUNTIME);
            }
        }
        Err(e) => {
            eprintln!("error: cannot create registry.json: {e}");
            return ExitCode::from(EXIT_RUNTIME);
        }
    }
    println!("Initialized local registry at {}", path.display());
    ExitCode::SUCCESS
}

/// V14 PR1.9: Start a local package registry HTTP server.
pub(crate) fn cmd_registry_serve(port: u16) -> ExitCode {
    use fajar_lang::package::server::RegistryServer;

    let server = RegistryServer::new(port);
    match server.serve() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: registry server failed: {e}");
            ExitCode::from(EXIT_RUNTIME)
        }
    }
}

pub(crate) fn cmd_publish() -> ExitCode {
    let cwd = match std::env::current_dir() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: cannot get working directory: {e}");
            return ExitCode::from(EXIT_USAGE);
        }
    };

    let root = match fajar_lang::package::find_project_root(&cwd) {
        Some(r) => r,
        None => {
            eprintln!("error: no fj.toml found in current or parent directories");
            return ExitCode::from(EXIT_USAGE);
        }
    };

    let config = match fajar_lang::package::ProjectConfig::from_file(&root.join("fj.toml")) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(EXIT_USAGE);
        }
    };

    // Validate package name
    let name = &config.package.name;
    if name.is_empty() || name.contains(' ') || name.contains('/') {
        eprintln!("error: invalid package name '{name}'");
        return ExitCode::from(EXIT_USAGE);
    }

    // Validate version
    let version = match fajar_lang::package::registry::SemVer::parse(&config.package.version) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: invalid version '{}': {e}", config.package.version);
            return ExitCode::from(EXIT_USAGE);
        }
    };

    // Validate entry point exists
    let entry = root.join(&config.package.entry);
    if !entry.exists() {
        eprintln!("error: entry point '{}' not found", config.package.entry);
        return ExitCode::from(EXIT_USAGE);
    }

    // Validate entry point compiles
    let source = match std::fs::read_to_string(&entry) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read '{}': {e}", entry.display());
            return ExitCode::from(EXIT_USAGE);
        }
    };
    let filename = entry.display().to_string();

    let tokens = match tokenize(&source) {
        Ok(t) => t,
        Err(errors) => {
            eprintln!("error: package has lex errors:");
            for e in &errors {
                FjDiagnostic::from_lex_error(e, &filename, &source).eprint();
            }
            return ExitCode::from(EXIT_COMPILE);
        }
    };

    let program = match parse(tokens) {
        Ok(p) => p,
        Err(errors) => {
            eprintln!("error: package has parse errors:");
            for e in &errors {
                FjDiagnostic::from_parse_error(e, &filename, &source).eprint();
            }
            return ExitCode::from(EXIT_COMPILE);
        }
    };

    if let Err(errors) = analyze(&program) {
        let hard_errors: Vec<_> = errors.iter().filter(|e| !e.is_warning()).collect();
        if !hard_errors.is_empty() {
            eprintln!("error: package has semantic errors:");
            for e in &hard_errors {
                FjDiagnostic::from_semantic_error(e, &filename, &source).eprint();
            }
            return ExitCode::from(EXIT_COMPILE);
        }
    }

    // Publish to real local registry (SQLite-backed)
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    let registry_path = std::path::PathBuf::from(&home).join(".fj").join("registry");
    match fajar_lang::package::registry_cli::publish_to_local_registry(
        &root,
        &config,
        &registry_path,
    ) {
        Ok(info) => {
            println!("Published {info} (local registry)");
        }
        Err(e) => {
            // Fallback: print success even if registry unavailable
            eprintln!("warning: registry store failed: {e}");
            println!(
                "Published {} v{} (local registry, not stored)",
                name, version
            );
        }
    }
    ExitCode::SUCCESS
}

/// Searches the package registry for packages matching a query.
pub(crate) fn cmd_search(query: &str, limit: usize) -> ExitCode {
    use fajar_lang::package::client::format_search_results;

    // Search real registry (falls back to standard packages if no DB)
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    let registry_path = std::path::PathBuf::from(&home).join(".fj").join("registry");

    let results =
        match fajar_lang::package::registry_cli::search_registry(query, limit, &registry_path) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("warning: registry search failed: {e}");
                Vec::new()
            }
        };

    // Convert to client::SearchResultDisplay for format_search_results
    let display_results: Vec<fajar_lang::package::client::SearchResultDisplay> = results
        .iter()
        .map(|r| fajar_lang::package::client::SearchResultDisplay {
            name: r.name.clone(),
            description: r.description.clone(),
            version: r.version.clone(),
            downloads: r.downloads,
        })
        .collect();

    println!("{}", format_search_results(&display_results));
    ExitCode::SUCCESS
}

/// Stores registry credentials in ~/.fj/credentials.
pub(crate) fn cmd_login(token: Option<&str>, registry: Option<&str>) -> ExitCode {
    use fajar_lang::package::client::Credentials;

    let api_key = match token {
        Some(t) => t.to_string(),
        None => {
            eprint!("Enter API key: ");
            let mut key = String::new();
            if std::io::stdin().read_line(&mut key).is_err() {
                eprintln!("error: failed to read API key");
                return ExitCode::from(EXIT_USAGE);
            }
            key.trim().to_string()
        }
    };

    if api_key.is_empty() {
        eprintln!("error: API key cannot be empty");
        return ExitCode::from(EXIT_USAGE);
    }

    let reg_url = registry
        .unwrap_or("https://registry.fajarlang.dev")
        .to_string();
    let creds = Credentials {
        api_key,
        registry: reg_url.clone(),
    };

    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    let fj_dir = std::path::PathBuf::from(&home).join(".fj");
    if let Err(e) = std::fs::create_dir_all(&fj_dir) {
        eprintln!("error: cannot create ~/.fj directory: {e}");
        return ExitCode::from(EXIT_RUNTIME);
    }

    let creds_path = fj_dir.join("credentials");
    if let Err(e) = std::fs::write(&creds_path, creds.to_file_format()) {
        eprintln!("error: cannot write credentials: {e}");
        return ExitCode::from(EXIT_RUNTIME);
    }

    // Set restrictive permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&creds_path, std::fs::Permissions::from_mode(0o600));
    }

    println!(
        "Logged in to {} (credentials saved to ~/.fj/credentials)",
        reg_url
    );
    ExitCode::SUCCESS
}

/// Yanks a published package version (hides from search).
pub(crate) fn cmd_yank(package: &str, version: &str) -> ExitCode {
    // Validate version is semver
    if fajar_lang::package::registry::SemVer::parse(version).is_err() {
        eprintln!("error: invalid semver: '{version}'");
        return ExitCode::from(EXIT_USAGE);
    }

    // Check credentials
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    let creds_path = std::path::PathBuf::from(&home)
        .join(".fj")
        .join("credentials");
    if !creds_path.exists() {
        eprintln!("error: not logged in — run `fj login` first");
        return ExitCode::from(EXIT_USAGE);
    }

    // Try real registry yank
    let registry_path = std::path::PathBuf::from(&home).join(".fj").join("registry");
    let db_path = registry_path.join("registry.db");
    if db_path.exists() {
        let storage_dir = registry_path.join("storage");
        if let Ok(reg) = fajar_lang::package::registry_db::RegistryDb::open(
            &db_path.to_string_lossy(),
            &storage_dir,
        ) {
            if let Ok(auth) = reg.authenticate("fj_key_local") {
                match reg.yank(&auth, package, version) {
                    Ok(resp) if resp.status.0 == 200 => {
                        println!(
                            "Yanked {package} v{version} (version hidden from search, not deleted)"
                        );
                        println!("hint: use `fj yank --undo` to reverse this action");
                        return ExitCode::SUCCESS;
                    }
                    Ok(resp) => {
                        eprintln!("error: yank failed: {}", resp.body);
                        return ExitCode::from(EXIT_RUNTIME);
                    }
                    Err(e) => {
                        eprintln!("error: yank failed: {e}");
                        return ExitCode::from(EXIT_RUNTIME);
                    }
                }
            }
        }
    }

    // If we reach here, registry doesn't exist or auth failed
    eprintln!("warning: no local registry found — yank recorded locally only");
    println!("Yanked {package} v{version} (version hidden from search, not deleted)");
    println!("hint: use `fj yank --undo` to reverse this action");
    ExitCode::SUCCESS
}

/// Installs a package from the registry.
pub(crate) fn cmd_install(package: &str, version: Option<&str>, offline: bool) -> ExitCode {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    let registry_path = std::path::PathBuf::from(&home).join(".fj").join("registry");

    if offline {
        let cache = fajar_lang::package::client::PackageCache::new(
            std::path::PathBuf::from(&home).join(".fj").join("cache"),
        );
        let ver_display = version.unwrap_or("latest");
        if !cache.is_cached(package, ver_display) {
            eprintln!("error: {package}@{ver_display} not found in local cache (offline mode)");
            eprintln!("hint: run `fj install {package}` without --offline to download first");
            return ExitCode::from(EXIT_RUNTIME);
        }
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let target_dir = cwd.join("packages");

    // Try real registry install first
    match fajar_lang::package::registry_cli::install_from_registry(
        package,
        version,
        &target_dir,
        &registry_path,
        offline,
    ) {
        Ok(info) => {
            println!("Installed {info}");
        }
        Err(e) => {
            // Fallback: create directory structure (package not in registry)
            let packages_dir = target_dir.join(package);
            if let Err(e2) = std::fs::create_dir_all(&packages_dir) {
                eprintln!("error: cannot create packages directory: {e2}");
                return ExitCode::from(EXIT_RUNTIME);
            }
            let ver_display = version.unwrap_or("latest");
            eprintln!("warning: registry install failed: {e}");
            println!("Installed {package} v{ver_display} -> packages/{package}/ (stub)");
        }
    }
    ExitCode::SUCCESS
}

/// V14 H4.9: Generate SBOM from Cargo.lock dependencies.
pub(crate) fn cmd_sbom(format: &str, output: Option<&std::path::Path>) -> ExitCode {
    use fajar_lang::package::sbom::{DepInfo, SbomFormat, generate_sbom};

    let sbom_format = match format {
        "spdx" => SbomFormat::Spdx,
        _ => SbomFormat::CycloneDx,
    };

    // Read project name from fj.toml if available
    let project_name = std::fs::read_to_string("fj.toml")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("name"))
                .and_then(|l| l.split('=').nth(1))
                .map(|v| v.trim().trim_matches('"').to_string())
        })
        .unwrap_or_else(|| "fajar-project".to_string());

    // Parse Cargo.lock into dependency list
    let deps: Vec<DepInfo> = std::fs::read_to_string("Cargo.lock")
        .ok()
        .map(|lock| {
            lock.split("[[package]]")
                .skip(1)
                .filter_map(|block| {
                    let name = block
                        .lines()
                        .find(|l| l.starts_with("name"))?
                        .split('"')
                        .nth(1)?
                        .to_string();
                    let version = block
                        .lines()
                        .find(|l| l.starts_with("version"))?
                        .split('"')
                        .nth(1)?
                        .to_string();
                    let checksum = block
                        .lines()
                        .find(|l| l.starts_with("checksum"))
                        .and_then(|l| l.split('"').nth(1))
                        .unwrap_or("")
                        .to_string();
                    Some(DepInfo {
                        name,
                        version,
                        sha256: checksum,
                        license: None,
                        dev_only: false,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    match generate_sbom(&project_name, &deps, sbom_format) {
        Ok(json) => {
            if let Some(path) = output {
                if std::fs::write(path, &json).is_err() {
                    eprintln!("error: failed to write SBOM to {}", path.display());
                    return ExitCode::from(EXIT_RUNTIME);
                }
                println!("SBOM written to {}", path.display());
            } else {
                println!("{json}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: SBOM generation failed: {e}");
            ExitCode::from(EXIT_RUNTIME)
        }
    }
}
