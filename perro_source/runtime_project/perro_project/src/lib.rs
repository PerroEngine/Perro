//! Project config parsing, scaffold templates, and source override maintenance.

use std::{
    collections::BTreeSet,
    fmt::{Display, Formatter},
    fs,
    path::{Path, PathBuf},
};
use toml::Value;

fn parse_toml_document_value(src: impl AsRef<str>) -> Result<Value, toml::de::Error> {
    src.as_ref().parse::<toml::Table>().map(Value::Table)
}

include!("config.rs");
include!("error.rs");
include!("scaffold.rs");
include!("config_parse.rs");
include!("templates.rs");
include!("manifest.rs");
include!("tests.rs");
