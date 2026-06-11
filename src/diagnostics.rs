use tower_lsp::lsp_types::Diagnostic;

const MAX_GROUPS: usize = 32;

pub fn check_line_brackets(line: &str, line_idx: u32) -> Option<Diagnostic> {
    let mut stack: Vec<char> = Vec::new();
    let mut first_bracket_pos: Option<usize> = None;
    let mut last_bracket_pos: usize = 0;

    for (i, ch) in line.char_indices() {
        match ch {
            '(' | '[' | '{' => {
                if stack.is_empty() {
                    first_bracket_pos = Some(i);
                }
                stack.push(ch);
                last_bracket_pos = i;
            }
            ')' | ']' | '}' => {
                let expected = match ch {
                    ')' => '(',
                    ']' => '[',
                    '}' => '{',
                    _ => unreachable!(),
                };
                if stack.pop() != Some(expected) {
                    let start = first_bracket_pos.unwrap_or(i);
                    return Some(Diagnostic {
                        range: tower_lsp::lsp_types::Range {
                            start: tower_lsp::lsp_types::Position {
                                line: line_idx,
                                character: start as u32,
                            },
                            end: tower_lsp::lsp_types::Position {
                                line: line_idx,
                                character: (i + 1) as u32,
                            },
                        },
                        severity: Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR),
                        source: Some("lammps-lsp".to_string()),
                        message: "Unbalanced parenthesis/bracket".to_string(),
                        ..Default::default()
                    });
                }
                last_bracket_pos = i;
            }
            _ => {}
        }
    }

    if !stack.is_empty() {
        let start = first_bracket_pos.unwrap_or(0);
        return Some(Diagnostic {
            range: tower_lsp::lsp_types::Range {
                start: tower_lsp::lsp_types::Position {
                    line: line_idx,
                    character: start as u32,
                },
                end: tower_lsp::lsp_types::Position {
                    line: line_idx,
                    character: (last_bracket_pos + 1) as u32,
                },
            },
            severity: Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR),
            source: Some("lammps-lsp".to_string()),
            message: "Unclosed parenthesis/bracket".to_string(),
            ..Default::default()
        });
    }

    None
}

pub fn check_group_count(lines: &[String]) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let mut group_count = 0u32;

    let group_pattern = regex_lite::Regex::new(
        r"^\s*group\s+\S+\s+(?:delete|clear|empty|region|type|id|molecule|variable|include|subtract|union|intersect|dynamic|static)",
    )
    .unwrap();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        if group_pattern.is_match(trimmed) {
            group_count += 1;
            if group_count > MAX_GROUPS as u32 {
                let pos = line.find("group").unwrap_or(0);
                diags.push(Diagnostic {
                    range: tower_lsp::lsp_types::Range {
                        start: tower_lsp::lsp_types::Position {
                            line: i as u32,
                            character: pos as u32,
                        },
                        end: tower_lsp::lsp_types::Position {
                            line: i as u32,
                            character: (pos + "group".len()) as u32,
                        },
                    },
                    severity: Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR),
                    source: Some("lammps-lsp".to_string()),
                    message: "There can be no more than 32 groups defined at one time, including \"all\""
                        .to_string(),
                    ..Default::default()
                });
            }
        }
    }

    diags
}

pub fn check_file_paths(lines: &[String]) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    let read_commands = ["read_data", "read_restart", "read_dump", "include"];

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }

        let first_word = trimmed.split_whitespace().next().unwrap_or("");
        if read_commands.contains(&first_word) {
            let args: Vec<&str> = trimmed.split_whitespace().collect();
            if args.len() > 1 {
                let file_path = args[1].trim_matches('"').trim_matches('\'');
                if !file_path.contains('*') && !file_path.contains('$') {
                    let pos = line.find(file_path).unwrap_or(0);
                    diags.push(Diagnostic {
                        range: tower_lsp::lsp_types::Range {
                            start: tower_lsp::lsp_types::Position {
                                line: i as u32,
                                character: pos as u32,
                            },
                            end: tower_lsp::lsp_types::Position {
                                line: i as u32,
                                character: (pos + file_path.len()) as u32,
                            },
                        },
                        severity: Some(tower_lsp::lsp_types::DiagnosticSeverity::WARNING),
                        source: Some("lammps-lsp".to_string()),
                        message: format!("Cannot verify if file '{}' exists (offline LSP)", file_path),
                        ..Default::default()
                    });
                }
            }
        }
    }

    diags
}
