use std::path::PathBuf;

/// Get the user's data directory (roaming on Windows)
/// - Windows: `{FOLDERID_RoamingAppData}` e.g. `C:\Users\Alice\AppData\Roaming`
/// - macOS: `$HOME/Library/Application Support` e.g. `/Users/Alice/Library/Application Support`
/// - Linux: `$XDG_DATA_HOME` or `$HOME/.local/share` e.g. `/home/alice/.local/share`
pub fn data_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA").map(PathBuf::from)
    }

    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
    }
}

/// Get the user's local data directory (non-roaming on Windows)
/// - Windows: `{FOLDERID_LocalAppData}` e.g. `C:\Users\Alice\AppData\Local`
/// - macOS: `$HOME/Library/Application Support` e.g. `/Users/Alice/Library/Application Support`
/// - Linux: `$XDG_DATA_HOME` or `$HOME/.local/share` e.g. `/home/alice/.local/share`
pub fn data_local_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    }

    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
    }
}

/// Get the user's config directory
/// - Windows: `{FOLDERID_RoamingAppData}` e.g. `C:\Users\Alice\AppData\Roaming`
/// - macOS: `$HOME/Library/Application Support` e.g. `/Users/Alice/Library/Application Support`
/// - Linux: `$XDG_CONFIG_HOME` or `$HOME/.config` e.g. `/home/alice/.config`
pub fn config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA").map(PathBuf::from)
    }

    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    }
}

/// Get the user's cache directory
/// - Windows: `{FOLDERID_LocalAppData}` e.g. `C:\Users\Alice\AppData\Local`
/// - macOS: `$HOME/Library/Caches` e.g. `/Users/Alice/Library/Caches`
/// - Linux: `$XDG_CACHE_HOME` or `$HOME/.cache` e.g. `/home/alice/.cache`
pub fn cache_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    }

    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Caches"))
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
    }
}

/// Get the user's home directory
/// - Windows: `{FOLDERID_Profile}` e.g. `C:\Users\Alice`
/// - macOS: `$HOME` e.g. `/Users/Alice`
/// - Linux: `$HOME` e.g. `/home/alice`
pub fn home_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    }

    #[cfg(not(target_os = "windows"))]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

/// Get the user's document directory
/// - Windows: `{FOLDERID_Documents}` e.g. `C:\Users\Alice\Documents`
/// - macOS: `$HOME/Documents` e.g. `/Users/Alice/Documents`
/// - Linux: `$XDG_DOCUMENTS_DIR` or `$HOME/Documents` e.g. `/home/alice/Documents`
pub fn document_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("USERPROFILE").map(|p| PathBuf::from(p).join("Documents"))
    }

    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Documents"))
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::env::var_os("XDG_DOCUMENTS_DIR")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Documents")))
    }
}

/// Get the user's download directory
/// - Windows: `{FOLDERID_Downloads}` e.g. `C:\Users\Alice\Downloads`
/// - macOS: `$HOME/Downloads` e.g. `/Users/Alice/Downloads`
/// - Linux: `$XDG_DOWNLOAD_DIR` or `$HOME/Downloads` e.g. `/home/alice/Downloads`
pub fn download_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("USERPROFILE").map(|p| PathBuf::from(p).join("Downloads"))
    }

    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Downloads"))
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::env::var_os("XDG_DOWNLOAD_DIR")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Downloads")))
    }
}

/// Get the user's desktop directory
/// - Windows: `{FOLDERID_Desktop}` e.g. `C:\Users\Alice\Desktop`
/// - macOS: `$HOME/Desktop` e.g. `/Users/Alice/Desktop`
/// - Linux: `$XDG_DESKTOP_DIR` or `$HOME/Desktop` e.g. `/home/alice/Desktop`
pub fn desktop_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("USERPROFILE").map(|p| PathBuf::from(p).join("Desktop"))
    }

    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Desktop"))
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::env::var_os("XDG_DESKTOP_DIR")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Desktop")))
    }
}

/// Get the system temporary directory
/// - All platforms: Uses `std::env::temp_dir()`
pub fn temp_dir() -> PathBuf {
    std::env::temp_dir()
}
