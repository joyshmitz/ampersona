use std::path::Path;

use anyhow::{Context, Result};
use serde_json::Value;

/// Summary row for a single persona file.
pub struct PersonaRow {
    pub file: String,
    pub name: String,
    pub mbti: String,
    pub role: String,
    pub skills: usize,
}

/// Scan a directory for .json files and produce summary rows.
pub fn scan_dir(dir: &str) -> Result<Vec<PersonaRow>> {
    let mut rows = Vec::new();
    let entries = std::fs::read_dir(dir).with_context(|| format!("cannot read directory {dir}"))?;

    let mut paths: Vec<_> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .map(|e| e.path())
        .collect();
    paths.sort();

    for path in paths {
        match load_row(&path) {
            Ok(row) => rows.push(row),
            Err(e) => {
                eprintln!("  skip {}: {e}", path.display());
            }
        }
    }
    Ok(rows)
}

fn load_row(path: &Path) -> Result<PersonaRow> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    let data: Value = serde_json::from_str(&content)
        .with_context(|| format!("{}: invalid JSON", path.display()))?;

    let file = path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();

    let name = data
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("-")
        .to_string();
    let mbti = data
        .pointer("/psychology/traits/mbti")
        .and_then(Value::as_str)
        .unwrap_or("-")
        .to_string();
    let role = data
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or("-")
        .to_string();
    let skills = data
        .pointer("/capabilities/skills")
        .and_then(Value::as_array)
        .map(|a| a.len())
        .unwrap_or(0);

    Ok(PersonaRow {
        file,
        name,
        mbti,
        role,
        skills,
    })
}

/// Print rows as an aligned table to stdout.
pub fn print_table(rows: &[PersonaRow]) {
    if rows.is_empty() {
        println!("(no personas found)");
        return;
    }
    let w_file = rows.iter().map(|r| r.file.len()).max().unwrap_or(4).max(4);
    let w_name = rows.iter().map(|r| r.name.len()).max().unwrap_or(4).max(4);
    let w_role = rows.iter().map(|r| r.role.len()).max().unwrap_or(4).max(4);

    let header = "SKILLS";
    let separator = "------";
    println!(
        "{:<w_file$}  {:<w_name$}  {:<4}  {:<w_role$}  {header}",
        "FILE", "NAME", "MBTI", "ROLE"
    );
    println!(
        "{:<w_file$}  {:<w_name$}  {:<4}  {:<w_role$}  {separator}",
        "-".repeat(w_file),
        "-".repeat(w_name),
        "----",
        "-".repeat(w_role)
    );
    for r in rows {
        println!(
            "{:<w_file$}  {:<w_name$}  {:<4}  {:<w_role$}  {}",
            r.file, r.name, r.mbti, r.role, r.skills
        );
    }
}
