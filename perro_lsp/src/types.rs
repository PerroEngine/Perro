use perro_core::fur_ast::{FurElement, FurNode};
use perro_core::scripting::ast::{Function, Script, Type, Variable};
use std::collections::HashMap;
use std::path::PathBuf;

/// Represents a parsed document with its AST and metadata
#[derive(Debug, Clone)]
pub enum ParsedDocument {
    Pup {
        script: Script,
        source: String,
        uri: String,
    },
    Fur {
        ast: Vec<FurNode>,
        source: String,
        uri: String,
    },
}

impl ParsedDocument {
    pub fn uri(&self) -> &str {
        match self {
            ParsedDocument::Pup { uri, .. } => uri,
            ParsedDocument::Fur { uri, .. } => uri,
        }
    }

    pub fn source(&self) -> &str {
        match self {
            ParsedDocument::Pup { source, .. } => source,
            ParsedDocument::Fur { source, .. } => source,
        }
    }
}

/// Document cache that stores parsed documents
pub struct DocumentCache {
    documents: HashMap<String, ParsedDocument>,
}

impl DocumentCache {
    pub fn new() -> Self {
        Self {
            documents: HashMap::new(),
        }
    }

    pub fn get(&self, uri: &str) -> Option<&ParsedDocument> {
        self.documents.get(uri)
    }

    pub fn insert(&mut self, uri: String, doc: ParsedDocument) {
        self.documents.insert(uri, doc);
    }

    pub fn remove(&mut self, uri: &str) {
        self.documents.remove(uri);
    }

    pub fn clear(&mut self) {
        self.documents.clear();
    }
}

/// Helper to convert file URI to path
pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    url::Url::parse(uri)
        .ok()
        .and_then(|url| url.to_file_path().ok())
}

/// Helper to convert path to file URI
pub fn path_to_uri(path: &PathBuf) -> String {
    url::Url::from_file_path(path).unwrap().to_string()
}
