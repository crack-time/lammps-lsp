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
        let first_word = trimmed.split_whitespace().next().unwrap_or("");
        if first_word.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '/') {
            if trimmed.split_whitespace().count() <= 1 {
                return complete_command_name(db, first_word, position);
            }
        }
        return CompletionList {
            is_incomplete: false,
            items: vec![],
        };
    }

    complete_command_name(db, "", position)
}

fn complete_command_name(
    db: &CommandDb,
    prefix: &str,
    _position: &Position,
) -> CompletionList {
    let prefix_lower = prefix.to_lowercase();
    let matching: Vec<CompletionItem> = db
        .all_commands()
        .filter(|cmd| cmd.to_lowercase().starts_with(&prefix_lower))
        .take(100)
        .map(|cmd| {
            let detail = db
                .lookup(cmd)
                .and_then(|e| e.syntax.first())
                .cloned()
                .unwrap_or_default();

            let insert_text = cmd.clone();

            CompletionItem {
                label: cmd.clone(),
                detail: Some(detail),
                insert_text: Some(insert_text),
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
