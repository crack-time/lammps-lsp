use crate::db::CommandDb;
use tower_lsp::lsp_types::{
    Hover, HoverContents, MarkupContent, MarkupKind, Position, Range,
};

pub fn get_hover(db: &CommandDb, line: &str) -> Option<Hover> {
    if line.trim().starts_with('#') {
        return None;
    }

    let first_word = line.trim().split_whitespace().next()?;
    let doc = db.lookup(first_word)?;

    let mut md = String::new();

    if !doc.short_description.is_empty() {
        md.push_str(&doc.short_description);
        md.push_str("\n\n---\n\n");
    }

    if !doc.syntax.is_empty() {
        md.push_str("### Syntax\n```lammps\n");
        for syn in &doc.syntax {
            md.push_str(syn);
            md.push('\n');
        }
        md.push_str("```\n\n");
    }

    if !doc.parameters.is_empty() {
        md.push_str(&doc.parameters);
        md.push('\n');
    }

    if !doc.examples.is_empty() {
        md.push_str("### Examples\n");
        md.push_str(&doc.examples);
        md.push('\n');
    }

    if !doc.restrictions.is_empty() {
        md.push_str("### Restrictions\n");
        md.push_str(&doc.restrictions);
        md.push('\n');
    }

    if !doc.related.is_empty() {
        md.push_str("### Related\n");
        md.push_str(&doc.related);
        md.push('\n');
    }

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: md,
        }),
        range: Some(Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: line.len() as u32,
            },
        }),
    })
}
