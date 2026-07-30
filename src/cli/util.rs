//! Shared source-loading helpers for CLI commands.
//!
//! Extracted from `main.rs` (REFACTOR_2026_07 Phase 2); pure code motion.

use crate::EXIT_USAGE;
use std::path::PathBuf;
use std::process::ExitCode;

/// Reads a source file, returning its contents or printing an error.
pub(crate) fn read_source(path: &PathBuf) -> Result<String, ExitCode> {
    if path.is_dir() {
        // Directory mode: concatenate all .fj files in the directory
        read_source_dir(path)
    } else {
        std::fs::read_to_string(path).map_err(|e| {
            eprintln!("error: cannot read '{}': {e}", path.display());
            ExitCode::from(EXIT_USAGE)
        })
    }
}

/// Reads all .fj files in a directory and concatenates them.
/// Files are sorted alphabetically, except main.fj is always last.
/// Orders files by their `use` dependencies (topological sort).
///
/// Files that are depended on by others come first.
/// Falls back to alphabetical if no dependencies detected or cycle exists.
pub(crate) fn order_by_dependencies(files: &[PathBuf]) -> Vec<PathBuf> {
    use std::collections::{HashMap, HashSet, VecDeque};

    if files.len() <= 1 {
        return files.to_vec();
    }

    // Extract module name from file path: kernel/mm/frames.fj → "frames"
    let file_modules: HashMap<String, usize> = files
        .iter()
        .enumerate()
        .filter_map(|(i, f)| {
            f.file_stem()
                .and_then(|s| s.to_str())
                .map(|name| (name.to_string(), i))
        })
        .collect();

    // Parse `use` statements from each file to find dependencies
    let mut deps: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut in_degree: HashMap<usize, usize> = HashMap::new();

    for (i, file) in files.iter().enumerate() {
        in_degree.entry(i).or_insert(0);
        if let Ok(content) = std::fs::read_to_string(file) {
            for line in content.lines() {
                let trimmed = line.trim();
                // Match: use module_name, use path::module_name
                if let Some(rest) = trimmed.strip_prefix("use ") {
                    let module_path = rest.trim_end_matches(';').trim();
                    // Get last segment: use kernel::mm::frames → "frames"
                    let module_name = module_path
                        .rsplit("::")
                        .next()
                        .unwrap_or(module_path)
                        .trim();
                    // Also try full path segments
                    let segments: Vec<&str> = module_path.split("::").collect();

                    // Check if any file matches this dependency
                    for seg in &segments {
                        if let Some(&dep_idx) = file_modules.get(*seg) {
                            if dep_idx != i {
                                deps.entry(dep_idx).or_default().push(i);
                                *in_degree.entry(i).or_insert(0) += 1;
                            }
                        }
                    }
                    if let Some(&dep_idx) = file_modules.get(module_name) {
                        if dep_idx != i && !deps.get(&dep_idx).is_some_and(|d| d.contains(&i)) {
                            deps.entry(dep_idx).or_default().push(i);
                            *in_degree.entry(i).or_insert(0) += 1;
                        }
                    }
                }
            }
        }
    }

    // Kahn's algorithm for topological sort
    let mut queue: VecDeque<usize> = in_degree
        .iter()
        .filter(|(_, deg)| **deg == 0)
        .map(|(&idx, _)| idx)
        .collect();

    // Sort initial queue for determinism
    let mut sorted_queue: Vec<usize> = queue.drain(..).collect();
    sorted_queue.sort();
    queue.extend(sorted_queue);

    let mut result: Vec<PathBuf> = Vec::new();
    let mut visited = HashSet::new();

    while let Some(idx) = queue.pop_front() {
        if !visited.insert(idx) {
            continue;
        }
        result.push(files[idx].clone());

        if let Some(dependents) = deps.get(&idx) {
            for &dep in dependents {
                if let Some(deg) = in_degree.get_mut(&dep) {
                    *deg = deg.saturating_sub(1);
                    if *deg == 0 && !visited.contains(&dep) {
                        queue.push_back(dep);
                    }
                }
            }
        }
    }

    // Add any files not in the dependency graph (standalone)
    for (i, file) in files.iter().enumerate() {
        if !visited.contains(&i) {
            result.push(file.clone());
        }
    }

    // If topological sort produced fewer files (cycle), fallback to alphabetical
    if result.len() < files.len() {
        let mut fallback = files.to_vec();
        fallback.sort();
        eprintln!("warning: circular dependency detected, using alphabetical order");
        return fallback;
    }

    result
}

pub(crate) fn read_source_dir(dir: &std::path::Path) -> Result<String, ExitCode> {
    let mut files: Vec<PathBuf> = Vec::new();
    let mut main_file: Option<PathBuf> = None;

    // Collect all .fj files recursively
    fn collect_fj_files(
        dir: &std::path::Path,
        files: &mut Vec<PathBuf>,
        main_file: &mut Option<PathBuf>,
    ) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    collect_fj_files(&path, files, main_file);
                } else if path.extension().is_some_and(|e| e == "fj") {
                    if path.file_name().is_some_and(|n| n == "main.fj") {
                        *main_file = Some(path);
                    } else {
                        files.push(path);
                    }
                }
            }
        }
    }

    collect_fj_files(dir, &mut files, &mut main_file);

    // E8: Auto-include shared/ directory for cross-service type definitions
    let mut shared_files: Vec<PathBuf> = Vec::new();
    let mut shared_main: Option<PathBuf> = None;
    for candidate in [dir.join("../shared"), dir.join("../../shared")] {
        if candidate.is_dir() {
            collect_fj_files(&candidate, &mut shared_files, &mut shared_main);
            if !shared_files.is_empty() {
                eprintln!(
                    "info: including {} shared type files from '{}'",
                    shared_files.len(),
                    candidate.display()
                );
                break;
            }
        }
    }

    // Build final file list: shared first, then service files in dependency order, main.fj last
    shared_files.sort();

    // Dependency-based ordering: parse `use` statements to determine file order
    let service_files = order_by_dependencies(&files);

    let mut final_files = shared_files;
    final_files.extend(service_files);
    if let Some(main) = main_file {
        final_files.push(main);
    }
    let files = final_files;

    if files.is_empty() {
        eprintln!("error: no .fj files found in '{}'", dir.display());
        return Err(ExitCode::from(EXIT_USAGE));
    }

    let mut combined = String::new();
    for f in &files {
        match std::fs::read_to_string(f) {
            Ok(content) => {
                combined.push_str(&format!("\n// ── Source: {} ──\n", f.display()));
                combined.push_str(&content);
                combined.push('\n');
            }
            Err(e) => {
                eprintln!("error: cannot read '{}': {e}", f.display());
                return Err(ExitCode::from(EXIT_USAGE));
            }
        }
    }

    eprintln!(
        "info: concatenated {} .fj files from '{}'",
        files.len(),
        dir.display()
    );
    Ok(combined)
}
