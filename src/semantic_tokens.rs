use crate::db::CommandDb;
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

fn tokenize_line(db: &CommandDb, trimmed: &str, line: &str) -> Vec<(u32, u32, usize)> {
    let mut tokens = Vec::new();

    if trimmed.is_empty() {
        return tokens;
    }

    if let Some(comment_pos) = trimmed.find('#') {
        let start = line.find('#').unwrap_or(0);
        let length = (line.len() - start) as u32;
        let prefix = &trimmed[..comment_pos].trim();
        if let Some(var_tokens) = scan_variables_in_prefix(line, prefix) {
            tokens.extend(var_tokens);
        }
        tokens.push((start as u32, length, T_COMMENT));
        return tokens;
    }

    let words: Vec<&str> = trimmed.split_whitespace().collect();
    if words.is_empty() {
        return tokens;
    }

    let first_word = words[0];
    let is_known_cmd = db.lookup(first_word).is_some();

    for (i, abs_pos) in word_positions(line, &words).iter().enumerate() {
        if abs_pos.is_empty() {
            continue;
        }
        let (start, len) = abs_pos[0];
        let word = words[i];

        if i == 0 && is_known_cmd {
            tokens.push((start, len, T_KEYWORD));
            continue;
        }

        if is_known_cmd && i == 3 && ["fix", "compute", "dump"].contains(&first_word) {
            let style_cmd = format!("{} {}", first_word, word);
            if db.lookup(&style_cmd).is_some() {
                tokens.push((start, len, T_STYLE));
                continue;
            }
        }

        if is_known_cmd && i == 1 && first_word.ends_with("_style") {
            let style_cmd = format!("{} {}", first_word, word);
            if db.lookup(&style_cmd).is_some() {
                tokens.push((start, len, T_STYLE));
                continue;
            }
        }

        if word.parse::<f64>().is_ok() {
            tokens.push((start, len, T_NUMBER));
            continue;
        }

        if word.starts_with('"') && word.ends_with('"') && word.len() > 1 {
            tokens.push((start, len, T_STRING));
            continue;
        }
    }

    if let Some(var_tokens) = scan_variables(line, trimmed) {
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

fn scan_variables_in_prefix(line: &str, prefix: &str) -> Option<Vec<(u32, u32, usize)>> {
    let line_chars: Vec<char> = line.chars().collect();
    let prefix_end = prefix.len().min(line.len());
    scan_vars_in_range(&line_chars, 0, prefix_end)
}

fn scan_variables(line: &str, _trimmed: &str) -> Option<Vec<(u32, u32, usize)>> {
    let line_chars: Vec<char> = line.chars().collect();
    scan_vars_in_range(&line_chars, 0, line.len())
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
