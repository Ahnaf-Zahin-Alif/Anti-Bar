use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotlightEntry {
    pub id: String,
    pub name: String,
    pub subtitle: String,
    pub path: String,
    pub kind: String, // "app" | "system" | "math"
    pub icon_data_url: Option<String>,
}

fn collect_lnk_recursive(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    if depth > 4 || !dir.exists() {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_lnk_recursive(&p, out, depth + 1);
        } else if p
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.eq_ignore_ascii_case("lnk"))
            .unwrap_or(false)
        {
            out.push(p);
        }
    }
}

pub fn index_installed_apps() -> Vec<SpotlightEntry> {
    let mut results = Vec::new();
    let mut seen = HashSet::new();

    let mut dirs_to_scan = Vec::new();
    if let Ok(appdata) = std::env::var("APPDATA") {
        dirs_to_scan.push(
            PathBuf::from(appdata)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs"),
        );
    }
    if let Ok(progdata) = std::env::var("ProgramData") {
        dirs_to_scan.push(
            PathBuf::from(progdata)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs"),
        );
    }

    let mut shortcuts = Vec::new();
    for d in &dirs_to_scan {
        collect_lnk_recursive(d, &mut shortcuts, 0);
    }

    for lnk in shortcuts {
        let Some(stem) = lnk.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let lower = stem.to_ascii_lowercase();
        if lower.contains("uninstall") || lower.contains("readme") || lower.contains("help") {
            continue;
        }
        if !seen.insert(lower) {
            continue;
        }
        let path_str = lnk.to_string_lossy().to_string();
        results.push(SpotlightEntry {
            id: format!("lnk:{}", path_str),
            name: stem.to_string(),
            subtitle: "Application".into(),
            path: path_str,
            kind: "app".into(),
            icon_data_url: None,
        });
    }

    let builtins = [
        ("Settings", "Windows Settings", "ms-settings:"),
        ("Task Manager", "System Monitor", "taskmgr.exe"),
        ("Control Panel", "Windows Control Panel", "control.exe"),
        ("Calculator", "Windows Calculator", "calc.exe"),
        ("Command Prompt", "Windows Terminal / CMD", "cmd.exe"),
        ("File Explorer", "Browse Files", "explorer.exe"),
        ("Paint", "Image Editor", "mspaint.exe"),
        ("Snipping Tool", "Screen Capture", "snippingtool.exe"),
    ];
    for (name, sub, cmd) in builtins {
        if seen.insert(name.to_ascii_lowercase()) {
            results.push(SpotlightEntry {
                id: format!("sys:{}", cmd),
                name: name.into(),
                subtitle: sub.into(),
                path: cmd.into(),
                kind: "system".into(),
                icon_data_url: None,
            });
        }
    }

    results.sort_by_key(|a| a.name.to_lowercase());
    results
}

pub fn filter_spotlight_entries(
    entries: &[SpotlightEntry],
    query_lower: &str,
    limit: usize,
) -> Vec<SpotlightEntry> {
    if query_lower.is_empty() {
        return entries.iter().take(limit).cloned().collect();
    }
    entries
        .iter()
        .filter(|e| {
            e.name.to_ascii_lowercase().contains(query_lower)
                || e.subtitle.to_ascii_lowercase().contains(query_lower)
        })
        .take(limit)
        .cloned()
        .collect()
}

/// Safe recursive-descent arithmetic evaluator supporting +, -, *, /, %, ^, and parentheses.
pub fn eval_math_expr(input: &str) -> Option<f64> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Must contain at least one operator or parens so plain numbers aren't treated as calculations
    if !trimmed
        .chars()
        .any(|c| matches!(c, '+' | '-' | '*' | '/' | '%' | '^' | '('))
    {
        return None;
    }
    let tokens = tokenize(trimmed)?;
    let mut pos = 0;
    let val = parse_expr(&tokens, &mut pos)?;
    if pos != tokens.len() || !val.is_finite() {
        return None;
    }
    Some(val)
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Op(char),
    LParen,
    RParen,
}

fn tokenize(s: &str) -> Option<Vec<Tok>> {
    let mut toks = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let ch = bytes[i] as char;
        if ch.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if ch.is_ascii_digit() || ch == '.' {
            let start = i;
            let mut dots = 0;
            while i < bytes.len() {
                let c = bytes[i] as char;
                if c.is_ascii_digit() {
                    i += 1;
                } else if c == '.' && dots == 0 {
                    dots += 1;
                    i += 1;
                } else {
                    break;
                }
            }
            let num: f64 = s[start..i].parse().ok()?;
            toks.push(Tok::Num(num));
            continue;
        }
        match ch {
            '+' | '-' | '*' | '/' | '%' | '^' => {
                toks.push(Tok::Op(ch));
                i += 1;
            }
            '(' => {
                toks.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                toks.push(Tok::RParen);
                i += 1;
            }
            _ => return None,
        }
    }
    Some(toks)
}

fn parse_expr(toks: &[Tok], pos: &mut usize) -> Option<f64> {
    let mut left = parse_term(toks, pos)?;
    while *pos < toks.len() {
        match toks[*pos] {
            Tok::Op('+') => {
                *pos += 1;
                left += parse_term(toks, pos)?;
            }
            Tok::Op('-') => {
                *pos += 1;
                left -= parse_term(toks, pos)?;
            }
            _ => break,
        }
    }
    Some(left)
}

fn parse_term(toks: &[Tok], pos: &mut usize) -> Option<f64> {
    let mut left = parse_power(toks, pos)?;
    while *pos < toks.len() {
        match toks[*pos] {
            Tok::Op('*') => {
                *pos += 1;
                left *= parse_power(toks, pos)?;
            }
            Tok::Op('/') => {
                *pos += 1;
                let rhs = parse_power(toks, pos)?;
                if rhs == 0.0 {
                    return None;
                }
                left /= rhs;
            }
            Tok::Op('%') => {
                *pos += 1;
                let rhs = parse_power(toks, pos)?;
                if rhs == 0.0 {
                    return None;
                }
                left %= rhs;
            }
            _ => break,
        }
    }
    Some(left)
}

fn parse_power(toks: &[Tok], pos: &mut usize) -> Option<f64> {
    let base = parse_unary(toks, pos)?;
    if *pos < toks.len() && toks[*pos] == Tok::Op('^') {
        *pos += 1;
        let exp = parse_power(toks, pos)?;
        Some(base.powf(exp))
    } else {
        Some(base)
    }
}

fn parse_unary(toks: &[Tok], pos: &mut usize) -> Option<f64> {
    if *pos >= toks.len() {
        return None;
    }
    match toks[*pos] {
        Tok::Op('-') => {
            *pos += 1;
            Some(-parse_unary(toks, pos)?)
        }
        Tok::Op('+') => {
            *pos += 1;
            parse_unary(toks, pos)
        }
        _ => parse_primary(toks, pos),
    }
}

fn parse_primary(toks: &[Tok], pos: &mut usize) -> Option<f64> {
    if *pos >= toks.len() {
        return None;
    }
    match &toks[*pos] {
        Tok::Num(n) => {
            *pos += 1;
            Some(*n)
        }
        Tok::LParen => {
            *pos += 1;
            let val = parse_expr(toks, pos)?;
            if *pos >= toks.len() || toks[*pos] != Tok::RParen {
                return None;
            }
            *pos += 1;
            Some(val)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn math_precedence_and_parens() {
        assert_eq!(eval_math_expr("2 + 3 * 4"), Some(14.0));
        assert_eq!(eval_math_expr("(2 + 3) * 4"), Some(20.0));
        assert_eq!(eval_math_expr("2^3^2"), Some(512.0));
        assert_eq!(eval_math_expr("-5 + 12.5"), Some(7.5));
        assert_eq!(eval_math_expr("10 / 0"), None);
        assert_eq!(eval_math_expr("42"), None);
        assert_eq!(eval_math_expr("notepad"), None);
    }
}
