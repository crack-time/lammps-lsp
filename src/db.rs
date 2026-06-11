use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct DocEntry {
    pub command: Vec<String>,
    pub syntax: Vec<String>,
    pub parameters: String,
    pub examples: String,
    pub html_filename: String,
    pub short_description: String,
    pub description: String,
    pub restrictions: String,
    pub related: String,
}

#[derive(Debug)]
pub struct CommandDb {
    entries: Vec<DocEntry>,
    index: HashMap<String, usize>,
}

impl CommandDb {
    pub fn from_embedded() -> Self {
        let json = include_str!("../data/commands.json");
        let entries: Vec<DocEntry> =
            serde_json::from_str(json).expect("Failed to parse commands.json");
        let mut index = HashMap::new();
        for (i, entry) in entries.iter().enumerate() {
            for cmd in &entry.command {
                index.insert(cmd.clone(), i);
            }
        }
        Self { entries, index }
    }

    pub fn lookup(&self, command: &str) -> Option<&DocEntry> {
        self.index.get(command).map(|&i| &self.entries[i])
    }

    #[allow(dead_code)]
    pub fn find_by_prefix(&self, prefix: &str) -> Vec<&DocEntry> {
        let prefix_lower = prefix.to_lowercase();
        let mut results: Vec<&DocEntry> = self
            .index
            .keys()
            .filter(|k| k.to_lowercase().starts_with(&prefix_lower))
            .filter_map(|k| self.index.get(k))
            .map(|&i| &self.entries[i])
            .collect();
        results.dedup();
        results.truncate(50);
        results
    }

    pub fn all_commands(&self) -> impl Iterator<Item = &String> {
        self.index.keys()
    }

    pub fn lookup_with_prefix(&self, cmd_line: &str) -> Option<&DocEntry> {
        let trimmed = cmd_line.trim();
        let words: Vec<&str> = trimmed.split_whitespace().collect();
        if words.is_empty() {
            return None;
        }
        for len in (1..=words.len()).rev() {
            let candidate = words[..len].join(" ");
            if let Some(entry) = self.lookup(&candidate) {
                return Some(entry);
            }
        }
        None
    }

    pub fn sub_commands(&self, base_cmd: &str) -> Vec<&String> {
        let prefix = format!("{} ", base_cmd);
        let mut results: Vec<&String> = self
            .index
            .keys()
            .filter(|k| k.starts_with(&prefix))
            .collect();
        results.sort();
        results.dedup();
        results
    }
}
