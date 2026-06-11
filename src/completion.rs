use crate::db::CommandDb;
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionList, InsertTextFormat, Position,
};

pub fn get_completions(db: &CommandDb, position: &Position, line: &str) -> CompletionList {
    let col = position.character as usize;
    let before_cursor = if col <= line.len() {
        &line[..col]
    } else {
        line
    };

    let trimmed = before_cursor.trim();
    let has_text_on_line = trimmed.contains(|c: char| !c.is_whitespace());

    if has_text_on_line {
        let words: Vec<&str> = trimmed.split_whitespace().collect();
        let first_word = words[0];

        if first_word
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '/')
        {
            if words.len() == 1 {
                return complete_command_name(db, first_word, position);
            } else {
                let sub_prefix = words[1..].join(" ");
                return complete_sub_command(db, first_word, &sub_prefix, position);
            }
        }
        return CompletionList {
            is_incomplete: false,
            items: vec![],
        };
    }

    complete_command_name(db, "", position)
}

fn complete_command_name(db: &CommandDb, prefix: &str, _position: &Position) -> CompletionList {
    let prefix_lower = prefix.to_lowercase();
    let matching: Vec<CompletionItem> = db
        .all_commands()
        .filter(|cmd| !cmd.contains(' ') && cmd.to_lowercase().starts_with(&prefix_lower))
        .take(100)
        .map(|cmd| {
            let detail = db
                .lookup(cmd)
                .map(|e| e.short_description.clone())
                .unwrap_or_default();

            CompletionItem {
                label: cmd.clone(),
                detail: Some(detail),
                insert_text: Some(cmd.clone()),
                insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                kind: Some(CompletionItemKind::FUNCTION),
                ..Default::default()
            }
        })
        .collect();

    CompletionList {
        is_incomplete: false,
        items: matching,
    }
}

fn complete_sub_command(
    db: &CommandDb,
    base_cmd: &str,
    sub_prefix: &str,
    _position: &Position,
) -> CompletionList {
    let sub_prefix_lower = sub_prefix.to_lowercase();
    let matching: Vec<CompletionItem> = db
        .sub_commands(base_cmd)
        .iter()
        .filter(|cmd| {
            let sub = cmd.strip_prefix(&format!("{} ", base_cmd)).unwrap_or(cmd);
            sub.to_lowercase().starts_with(&sub_prefix_lower)
        })
        .take(100)
        .map(|cmd| {
            let sub = cmd.strip_prefix(&format!("{} ", base_cmd)).unwrap_or(cmd);
            let detail = db
                .lookup(cmd)
                .map(|e| e.short_description.clone())
                .unwrap_or_default();

            CompletionItem {
                label: sub.to_string(),
                detail: Some(detail),
                insert_text: Some(sub.to_string()),
                insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                kind: Some(CompletionItemKind::VALUE),
                ..Default::default()
            }
        })
        .collect();

    CompletionList {
        is_incomplete: false,
        items: matching,
    }
}
