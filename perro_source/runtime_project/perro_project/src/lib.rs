//! Project config parsing, scaffold templates, and source override maintenance.

use std::{
    collections::BTreeSet,
    fmt::{Display, Formatter},
    fs,
    path::{Path, PathBuf},
};
use toml::Value;

include!("config.rs");
include!("error.rs");
include!("scaffold.rs");
include!("config_parse.rs");
include!("templates.rs");
include!("manifest.rs");
include!("tests.rs");
