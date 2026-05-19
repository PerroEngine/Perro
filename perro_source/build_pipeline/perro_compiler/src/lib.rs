//! Project build pipeline, script crate generation, and bundle export helpers.

use perro_assets::{build_compressed_perro_archive_from_entries, build_perro_assets_archive};
use perro_io::walkdir::walk_dir;
use perro_project::{ensure_source_overrides, load_project_toml};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    env,
    fmt::{Display, Formatter},
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::Command,
    thread,
};

include!("error.rs");
include!("scripts.rs");
include!("dlc.rs");
include!("static_modules.rs");
include!("project_bundle.rs");
include!("script_writer.rs");
include!("script_codegen.rs");
include!("script_fields.rs");
include!("script_methods.rs");
include!("tests.rs");
