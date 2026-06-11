use crate::db::{CommandDb, DocEntry};
use std::collections::HashSet;
use tower_lsp::lsp_types::{
    SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokens,
    SemanticTokensFullOptions, SemanticTokensLegend, SemanticTokensOptions, SemanticTokensResult,
};

const T_COMMENT: usize = 0;
const T_KEYWORD: usize = 1;
const T_STYLE: usize = 2;
const T_NUMBER: usize = 3;
const T_VARIABLE: usize = 4;
const T_STRING: usize = 5;

pub fn legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: vec![
            SemanticTokenType::COMMENT,
            SemanticTokenType::KEYWORD,
            SemanticTokenType::FUNCTION,
            SemanticTokenType::NUMBER,
            SemanticTokenType::VARIABLE,
            SemanticTokenType::STRING,
        ],
        token_modifiers: vec![SemanticTokenModifier::STATIC],
    }
}

pub fn get_tokens(db: &CommandDb, text: &str) -> SemanticTokensResult {
    let lines: Vec<&str> = text.lines().collect();
    let mut tokens: Vec<SemanticToken> = Vec::new();
    let mut prev_line: u32 = 0;
    let mut prev_col: u32 = 0;

    for (line_idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let line_tokens = tokenize_line(db, trimmed, line);

        for (start, length, typ) in &line_tokens {
            let delta_line = (line_idx as u32) - prev_line;
            let delta_start = if delta_line == 0 {
                *start - prev_col
            } else {
                *start
            };

            tokens.push(SemanticToken {
                delta_line,
                delta_start,
                length: *length,
                token_type: *typ as u32,
                token_modifiers_bitset: 0,
            });

            prev_line = line_idx as u32;
            prev_col = start + length;
        }
    }

    SemanticTokensResult::Tokens(SemanticTokens {
        result_id: None,
        data: tokens,
    })
}

fn find_command_entry<'a>(db: &'a CommandDb, words: &[&str]) -> Option<(&'a DocEntry, usize)> {
    // Try longest prefix first
    for len in (2..=words.len()).rev() {
        let candidate = words[..len].join(" ");
        if let Some(entry) = db.lookup(&candidate) {
            let variant_idx = entry
                .command
                .iter()
                .position(|c| c == &candidate)
                .map(|i| i.min(entry.args.len().saturating_sub(1)))
                .unwrap_or(0);
            return Some((entry, variant_idx));
        }
    }

    // Fix/compute/dump: skip ID and group-ID (positions 1, 2)
    if words.len() >= 4 && ["fix", "compute", "dump"].contains(&words[0]) {
        for len in (1..=words.len() - 3).rev() {
            let candidate = format!("{} {}", words[0], words[3..3 + len].join(" "));
            if let Some(entry) = db.lookup(&candidate) {
                let variant_idx = entry
                    .command
                    .iter()
                    .position(|c| c == &candidate)
                    .map(|i| i.min(entry.args.len().saturating_sub(1)))
                    .unwrap_or(0);
                return Some((entry, variant_idx));
            }
        }
    }

    // Single word fallback
    if let Some(entry) = db.lookup(words[0]) {
        let variant_idx = entry
            .command
            .iter()
            .position(|c| c == words[0])
            .map(|i| i.min(entry.args.len().saturating_sub(1)))
            .unwrap_or(0);
        return Some((entry, variant_idx));
    }

    None
}

fn tokenize_line(db: &CommandDb, trimmed: &str, line: &str) -> Vec<(u32, u32, usize)> {
    let mut tokens = Vec::new();

    if trimmed.is_empty() {
        return tokens;
    }

    if trimmed.contains('#') {
        let start = line.find('#').unwrap_or(0);
        let length = (line.len() - start) as u32;
        if let Some(var_tokens) = scan_variables_in_prefix(line) {
            tokens.extend(var_tokens);
        }
        tokens.push((start as u32, length, T_COMMENT));
        return tokens;
    }

    let words: Vec<&str> = trimmed.split_whitespace().collect();
    if words.is_empty() {
        return tokens;
    }

    let entry_info: Option<(&CommandDb, &DocEntry, usize)> =
        find_command_entry(db, &words).map(|(e, vi)| (db, e, vi));

    let style_choices: HashSet<&str> = entry_info
        .iter()
        .flat_map(|(_, entry, variant_idx)| {
            entry
                .args
                .get(*variant_idx)
                .into_iter()
                .flat_map(|slots| slots.iter())
                .filter(|s| s.arg_type == 3)
                .flat_map(|s| s.choices.iter().map(|c| c.as_str()))
        })
        .collect();

    let first_word = words[0];
    let is_known_cmd = db.lookup(first_word).is_some();

    for (i, abs_pos) in word_positions(line, &words).iter().enumerate() {
        if abs_pos.is_empty() {
            continue;
        }
        let (start, len) = abs_pos[0];
        let word = words[i];

        // Data-driven: type=1 literal keyword at matching position
        if let Some((_, entry, variant_idx)) = &entry_info {
            if let Some(slot) = entry.args.get(*variant_idx).and_then(|s| s.get(i)) {
                if slot.arg_type == 1 && slot.arg == word {
                    tokens.push((start, len, T_KEYWORD));
                    continue;
                }
            }
        }

        // Data-driven: type=3 choice (style/keyword arg from args metadata)
        if style_choices.contains(word) {
            tokens.push((start, len, T_STYLE));
            continue;
        }

        // Position-based: fix/compute/dump style at position 3
        if is_known_cmd && i == 3 && ["fix", "compute", "dump"].contains(&first_word) {
            let style_cmd = format!("{} {}", first_word, word);
            if db.lookup(&style_cmd).is_some() {
                tokens.push((start, len, T_STYLE));
                continue;
            }
        }

        // Position-based: *_style commands — style at position 1
        if is_known_cmd && i == 1 && first_word.ends_with("_style") {
            let style_cmd = format!("{} {}", first_word, word);
            if db.lookup(&style_cmd).is_some() {
                tokens.push((start, len, T_STYLE));
                continue;
            }
        }

        // Position-based: base command at position 0
        if i == 0 && is_known_cmd {
            tokens.push((start, len, T_KEYWORD));
            continue;
        }

        // Heuristics: number
        if word.parse::<f64>().is_ok() {
            tokens.push((start, len, T_NUMBER));
            continue;
        }

        // Heuristics: quoted string
        if word.starts_with('"') && word.ends_with('"') && word.len() > 1 {
            tokens.push((start, len, T_STRING));
            continue;
        }
    }

    if let Some(var_tokens) = scan_variables(line) {
        tokens.extend(var_tokens);
    }

    tokens
}

fn word_positions<'a>(line: &str, words: &[&str]) -> Vec<Vec<(u32, u32)>> {
    let mut result = Vec::new();
    let mut search_pos = 0;
    for word in words {
        if let Some(pos) = line[search_pos..].find(word) {
            let abs_start = search_pos + pos;
            let len = word.len() as u32;
            result.push(vec![(abs_start as u32, len)]);
            search_pos = abs_start + len as usize;
        } else {
            result.push(vec![]);
        }
    }
    result
}

fn scan_variables_in_prefix(line: &str) -> Option<Vec<(u32, u32, usize)>> {
    let line_chars: Vec<char> = line.chars().collect();
    let end = line
        .chars()
        .position(|c| c == '#')
        .unwrap_or(line_chars.len());
    scan_vars_in_range(&line_chars, 0, end)
}

fn scan_variables(line: &str) -> Option<Vec<(u32, u32, usize)>> {
    let line_chars: Vec<char> = line.chars().collect();
    scan_vars_in_range(&line_chars, 0, line_chars.len())
}

fn scan_vars_in_range(chars: &[char], start: usize, end: usize) -> Option<Vec<(u32, u32, usize)>> {
    let mut tokens = Vec::new();
    let mut i = start;
    while i < end.min(chars.len()) {
        if chars[i] == '$' && i + 1 < end && chars[i + 1] == '{' {
            let var_start = i;
            let mut depth = 0u32;
            let mut j = i;
            while j < end.min(chars.len()) {
                if chars[j] == '{' {
                    depth += 1;
                } else if chars[j] == '}' {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        break;
                    }
                }
                j += 1;
            }
            if depth == 0 && j > i {
                let len = (j - i + 1) as u32;
                tokens.push((var_start as u32, len, T_VARIABLE));
                i = j + 1;
                continue;
            }
        } else if chars[i] == '$' && i + 1 < end && !chars[i + 1].is_whitespace() {
            let var_start = i;
            let mut j = i + 1;
            while j < end.min(chars.len()) && (chars[j].is_alphanumeric() || chars[j] == '_') {
                j += 1;
            }
            if j > i + 1 {
                let len = (j - i) as u32;
                tokens.push((var_start as u32, len, T_VARIABLE));
                i = j;
                continue;
            }
        }
        i += 1;
    }
    if tokens.is_empty() {
        None
    } else {
        Some(tokens)
    }
}

pub fn options() -> SemanticTokensOptions {
    SemanticTokensOptions {
        legend: legend(),
        full: Some(SemanticTokensFullOptions::Bool(true)),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> &'static CommandDb {
        use std::sync::OnceLock;
        static DB: OnceLock<CommandDb> = OnceLock::new();
        DB.get_or_init(CommandDb::from_embedded)
    }

    fn token_types(tokens: &[(u32, u32, usize)]) -> Vec<usize> {
        tokens.iter().map(|t| t.2).collect()
    }

    #[test]
    fn test_comment_line() {
        let db = test_db();
        let tokens = tokenize_line(db, "# comment", "# comment");
        assert_eq!(token_types(&tokens), vec![T_COMMENT]);
        assert_eq!(tokens[0].0, 0); // start at pos 0
        assert_eq!(tokens[0].1, 9); // length of "# comment"
    }

    #[test]
    fn test_comment_with_leading_space() {
        let db = test_db();
        let tokens = tokenize_line(db, "# comment", "  # comment");
        assert_eq!(token_types(&tokens), vec![T_COMMENT]);
        assert_eq!(tokens[0].0, 2); // start at pos 2
        assert_eq!(tokens[0].1, 9); // length of "# comment"
    }

    #[test]
    fn test_comment_with_variable_in_prefix() {
        let db = test_db();
        let tokens = tokenize_line(db, "fix ${x} # comment", "fix ${x} # comment");
        // Variable + comment
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].2, T_VARIABLE); // ${x}
        assert_eq!(tokens[1].2, T_COMMENT); // # comment
    }

    #[test]
    fn test_simple_command() {
        let db = test_db();
        let tokens = tokenize_line(db, "run 1000", "run 1000");
        assert_eq!(token_types(&tokens), vec![T_KEYWORD, T_NUMBER]);
        assert_eq!(tokens[0].0, 0); // run
        assert_eq!(tokens[0].1, 3);
        assert_eq!(tokens[1].0, 4); // 1000
        assert_eq!(tokens[1].1, 4);
    }

    #[test]
    fn test_fix_style() {
        let db = test_db();
        let tokens = tokenize_line(db, "fix 1 all nve", "fix 1 all nve");
        assert_eq!(token_types(&tokens), vec![T_KEYWORD, T_NUMBER, T_STYLE]);
    }

    #[test]
    fn test_fix_with_trailing_args() {
        let db = test_db();
        let tokens = tokenize_line(db, "fix 1 all nve 1.0 100", "fix 1 all nve 1.0 100");
        // fix=KEYWORD, 1=NUMBER, nve=STYLE, 1.0=NUMBER, 100=NUMBER
        assert!(tokens[0].2 == T_KEYWORD);
        let style_tokens: Vec<_> = tokens.iter().filter(|t| t.2 == T_STYLE).collect();
        assert_eq!(style_tokens.len(), 1);
    }

    #[test]
    fn test_pair_style() {
        let db = test_db();
        let tokens = tokenize_line(db, "pair_style lj/cut 2.5", "pair_style lj/cut 2.5");
        // pair_style=KEYWORD (type=1 slot 0), lj/cut=STYLE (via _style check), 2.5=NUMBER
        assert_eq!(token_types(&tokens), vec![T_KEYWORD, T_STYLE, T_NUMBER]);
        assert_eq!(tokens[0].0, 0); // pair_style at start
        assert_eq!(tokens[0].1, 10);
        assert_eq!(tokens[1].0, 11); // lj/cut
        assert_eq!(tokens[2].0, 18); // 2.5
    }

    #[test]
    fn test_variable() {
        let db = test_db();
        let tokens = tokenize_line(db, "fix ${x} all nve", "fix ${x} all nve");
        // fix=KEYWORD, ${x}=VARIABLE, nve=STYLE
        let types = token_types(&tokens);
        assert!(types.contains(&T_KEYWORD));
        assert!(types.contains(&T_VARIABLE));
        assert!(types.contains(&T_STYLE));
    }

    #[test]
    fn test_dollar_variable() {
        let db = test_db();
        let tokens = tokenize_line(db, "variable x equal $x", "variable x equal $x");
        let types = token_types(&tokens);
        assert!(types.contains(&T_VARIABLE));
    }

    #[test]
    fn test_blank_line_skipped() {
        let db = test_db();
        let lines = "run 1000\n\nrun 2000";
        let result = get_tokens(db, lines);
        let data = match result {
            SemanticTokensResult::Tokens(t) => t.data,
            _ => panic!("expected Tokens variant"),
        };
        assert_eq!(data.len(), 4);
        // "run" at line 0, col 0: first token, abs positions
        assert_eq!(data[0].delta_line, 0);
        assert_eq!(data[0].delta_start, 0);
        assert_eq!(data[0].length, 3);
        assert_eq!(data[0].token_type, T_KEYWORD as u32);
        // "1000" at line 0, col 4: relative to prev end (0+3=3), so ds=4-3=1
        assert_eq!(data[1].delta_line, 0);
        assert_eq!(data[1].delta_start, 1);
        assert_eq!(data[1].length, 4);
        assert_eq!(data[1].token_type, T_NUMBER as u32);
        // "run" at line 2, col 0: blank line skipped, dl=2, ds=abs=0
        assert_eq!(data[2].delta_line, 2);
        assert_eq!(data[2].delta_start, 0);
        assert_eq!(data[2].length, 3);
        assert_eq!(data[2].token_type, T_KEYWORD as u32);
        // "2000" relative to prev end: ds=4-3=1
        assert_eq!(data[3].delta_line, 0);
        assert_eq!(data[3].delta_start, 1);
        assert_eq!(data[3].length, 4);
        assert_eq!(data[3].token_type, T_NUMBER as u32);
    }

    #[test]
    fn test_unknown_command() {
        let db = test_db();
        let tokens = tokenize_line(db, "foobar 1 2", "foobar 1 2");
        // No known base command, only numbers should have tokens
        let types = token_types(&tokens);
        assert_eq!(types, vec![T_NUMBER, T_NUMBER]);
    }

    #[test]
    fn test_empty_line() {
        let db = test_db();
        let tokens = tokenize_line(db, "", "  ");
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_quoted_string() {
        let db = test_db();
        // Single-word quoted string (multi-word quotes get split by split_whitespace)
        let tokens = tokenize_line(db, "print \"hello\"", "print \"hello\"");
        let types = token_types(&tokens);
        assert!(types.contains(&T_STRING));
    }

    #[test]
    fn test_scan_vars_in_prefix_bugfix() {
        // Regression test for scan_variables_in_prefix with leading whitespace
        let db = test_db();
        let tokens = tokenize_line(db, "fix ${x} # comment", "  fix ${x} # comment");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].2, T_VARIABLE); // full ${x} captured
        assert_eq!(tokens[0].1, 4); // len=4 for "${x}"
        assert_eq!(tokens[1].2, T_COMMENT);
    }

    #[test]
    fn test_compute_style() {
        let db = test_db();
        let tokens = tokenize_line(db, "compute 1 all temp", "compute 1 all temp");
        let types = token_types(&tokens);
        assert!(types.contains(&T_KEYWORD));
        assert!(types.contains(&T_STYLE));
    }

    #[test]
    fn test_incomplete_fix() {
        let db = test_db();
        // User types just "fix nve" — should detect nve as style via type=3 choices
        let tokens = tokenize_line(db, "fix nve", "fix nve");
        let types = token_types(&tokens);
        assert!(types.contains(&T_KEYWORD));
        assert!(types.contains(&T_STYLE));
    }
}
