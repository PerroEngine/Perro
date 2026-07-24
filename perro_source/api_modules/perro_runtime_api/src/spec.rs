#[cfg(feature = "spec")]
use std::fs::OpenOptions;
#[cfg(feature = "spec")]
use std::io::Write;
#[cfg(feature = "spec")]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(feature = "spec")]
#[doc(hidden)]
pub fn __spec_marker(kind: &str, label: &str) {
    let Ok(path) = std::env::var("PERRO_SPEC_MARKERS") else {
        return;
    };
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let label = label
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r");
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(
            file,
            "{{\"timestamp_ms\":{timestamp_ms},\"kind\":\"{kind}\",\"label\":\"{label}\"}}"
        );
    }
}

#[macro_export]
macro_rules! spec_begin {
    ($label:expr) => {{
        #[cfg(feature = "perro-spec")]
        {
            $crate::spec::__spec_marker("begin", $label)
        }
        #[cfg(not(feature = "perro-spec"))]
        {
            ()
        }
    }};
}

#[macro_export]
macro_rules! spec_end {
    ($label:expr) => {{
        #[cfg(feature = "perro-spec")]
        {
            $crate::spec::__spec_marker("end", $label)
        }
        #[cfg(not(feature = "perro-spec"))]
        {
            ()
        }
    }};
}

#[macro_export]
macro_rules! spec_point {
    ($label:expr) => {{
        #[cfg(feature = "perro-spec")]
        {
            $crate::spec::__spec_marker("point", $label)
        }
        #[cfg(not(feature = "perro-spec"))]
        {
            ()
        }
    }};
}
