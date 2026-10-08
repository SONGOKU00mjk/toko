use std::{env, fs, path::PathBuf};

use mlua::{Lua, LuaOptions, StdLib, Table};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendPreference {
    Auto,
    Awww,
    Swaybg,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub wallpaper_dir: PathBuf,
    pub backend: BackendPreference,
    pub preview_max_dim: u32,
}

#[derive(Debug)]
pub enum ConfigError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Lua {
        path: PathBuf,
        source: mlua::Error,
    },
    Invalid {
        path: PathBuf,
        message: String,
    },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io { path, source } => {
                write!(f, "failed to read config {}: {source}", path.display())
            }
            ConfigError::Lua { path, source } => {
                write!(f, "invalid Lua in config {}: {source}", path.display())
            }
            ConfigError::Invalid { path, message } => {
                write!(f, "invalid config {}: {message}", path.display())
            }
        }
    }
}

impl std::error::Error for ConfigError {}

impl Config {
    pub fn default() -> Self {
        let wallpaper_dir = match env::var("HOME") {
            Ok(home) => PathBuf::from(home).join("Pictures").join("wallpapers"),
            Err(_) => PathBuf::from("~/Pictures/wallpapers"),
        };
        Self {
            wallpaper_dir,
            backend: BackendPreference::Auto,
            preview_max_dim: 1280,
        }
    }

    pub fn load() -> Result<Self, ConfigError> {
        let path = match config_path() {
            Some(p) => p,
            None => return Ok(Config::default()),
        };

        let code = match fs::read_to_string(&path) {
            Ok(code) => code,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Config::default());
            }
            Err(e) => return Err(ConfigError::Io { path, source: e }),
        };

        let libs = StdLib::STRING | StdLib::TABLE | StdLib::MATH | StdLib::UTF8;
        let lua = Lua::new_with(libs, LuaOptions::default()).map_err(|e| ConfigError::Lua {
            path: path.clone(),
            source: e,
        })?;

        let table: Table = lua.create_table().map_err(|e| ConfigError::Lua {
            path: path.clone(),
            source: e,
        })?;
        table
            .set("wallpaper_dir", "~/Pictures/wallpapers")
            .map_err(|e| ConfigError::Lua {
                path: path.clone(),
                source: e,
            })?;
        table.set("backend", "auto").map_err(|e| ConfigError::Lua {
            path: path.clone(),
            source: e,
        })?;
        table
            .set("preview_max_dim", 1280)
            .map_err(|e| ConfigError::Lua {
                path: path.clone(),
                source: e,
            })?;

        lua.load(&code)
            .set_environment(table.clone())
            .exec()
            .map_err(|e| ConfigError::Lua {
                path: path.clone(),
                source: e,
            })?;
        let wallpaper_dir_raw: String =
            table
                .get("wallpaper_dir")
                .map_err(|e| ConfigError::Invalid {
                    path: path.clone(),
                    message: format!("wallpaper_dir must be a string: {e}"),
                })?;
        let backend_raw: String = table.get("backend").map_err(|e| ConfigError::Invalid {
            path: path.clone(),
            message: format!("backend must be a string: {e}"),
        })?;
        let preview_max_dim_raw: i64 =
            table
                .get("preview_max_dim")
                .map_err(|e| ConfigError::Invalid {
                    path: path.clone(),
                    message: format!("preview_max_dim must be an integer: {e}"),
                })?;

        if wallpaper_dir_raw.trim().is_empty() {
            return Err(ConfigError::Invalid {
                path,
                message: "wallpaper_dir must not be empty".to_string(),
            });
        }

        let wallpaper_dir =
            expand_tilde(&wallpaper_dir_raw).map_err(|message| ConfigError::Invalid {
                path: path.clone(),
                message,
            })?;
        let backend = parse_backend(&backend_raw).map_err(|message| ConfigError::Invalid {
            path: path.clone(),
            message,
        })?;
        let preview_max_dim =
            validate_dim(preview_max_dim_raw).map_err(|message| ConfigError::Invalid {
                path: path.clone(),
                message,
            })?;

        Ok(Config {
            wallpaper_dir,
            backend,
            preview_max_dim,
        })
    }
}

fn config_path() -> Option<PathBuf> {
    if let Ok(xdg) = env::var("XDG_CONFIG_HOME")
        && !xdg.is_empty()
    {
        return Some(PathBuf::from(xdg).join("toko").join("config.lua"));
    }
    env::var("HOME").ok().map(|h| {
        PathBuf::from(h)
            .join(".config")
            .join("toko")
            .join("config.lua")
    })
}

fn expand_tilde(raw: &str) -> Result<PathBuf, String> {
    if raw == "~" {
        return env::var("HOME")
            .map(PathBuf::from)
            .map_err(|_| "wallpaper_dir uses '~' but HOME is not set".to_string());
    }
    if let Some(rest) = raw.strip_prefix("~/") {
        let home = env::var("HOME")
            .map_err(|_| "wallpaper_dir uses '~' but HOME is not set".to_string())?;
        return Ok(PathBuf::from(home).join(rest));
    }
    Ok(PathBuf::from(raw))
}

fn parse_backend(raw: &str) -> Result<BackendPreference, String> {
    match raw {
        "auto" => Ok(BackendPreference::Auto),
        "awww" => Ok(BackendPreference::Awww),
        "swaybg" => Ok(BackendPreference::Swaybg),
        other => Err(format!(
            "backend must be \"auto\", \"awww\", or \"swaybg\" (got {other:?})"
        )),
    }
}

fn validate_dim(n: i64) -> Result<u32, String> {
    if n <= 0 {
        return Err(format!("preview_max_dim must be > 0 (got {n})"));
    }
    if n > 8192 {
        return Err(format!("preview_max_dim must be <= 8192 (got {n})"));
    }
    Ok(n as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_backend_accepts_valid_values() {
        assert_eq!(parse_backend("auto").unwrap(), BackendPreference::Auto);
        assert_eq!(parse_backend("awww").unwrap(), BackendPreference::Awww);
        assert_eq!(parse_backend("swaybg").unwrap(), BackendPreference::Swaybg);
    }

    #[test]
    fn parse_backend_rejects_invalid_values() {
        assert!(parse_backend("").is_err());
        assert!(parse_backend("AUTO").is_err());
        assert!(parse_backend("hyprpaper").is_err());
        assert!(parse_backend(" awww").is_err());
    }

    #[test]
    fn validate_dim_accepts_valid_range() {
        assert_eq!(validate_dim(1).unwrap(), 1);
        assert_eq!(validate_dim(1280).unwrap(), 1280);
        assert_eq!(validate_dim(8192).unwrap(), 8192);
    }

    #[test]
    fn validate_dim_rejects_out_of_range() {
        assert!(validate_dim(0).is_err());
        assert!(validate_dim(-1).is_err());
        assert!(validate_dim(8193).is_err());
        assert!(validate_dim(i64::MAX).is_err());
    }

    #[test]
    fn default_config_matches_documented_defaults() {
        let c = Config::default();
        assert_eq!(c.backend, BackendPreference::Auto);
        assert_eq!(c.preview_max_dim, 1280);
        let s = c.wallpaper_dir.to_string_lossy();
        assert!(
            s.ends_with("Pictures/wallpapers") || s == "~/Pictures/wallpapers",
            "unexpected default wallpaper_dir: {s}"
        );
    }

    #[test]
    fn expand_tilde_leaves_absolute_paths_alone() {
        let p = expand_tilde("/foo/bar").unwrap();
        assert_eq!(p, PathBuf::from("/foo/bar"));
    }

    #[test]
    fn expand_tilde_leaves_relative_paths_alone() {
        let p = expand_tilde("wallpapers/walls").unwrap();
        assert_eq!(p, PathBuf::from("wallpapers/walls"));
    }

    #[test]
    fn expand_tilde_expands_home_when_set() {
        if env::var("HOME").is_ok() {
            let p = expand_tilde("~/walls").unwrap();
            assert!(p.is_absolute());
            assert!(p.to_string_lossy().ends_with("walls"));
        }
    }
}
