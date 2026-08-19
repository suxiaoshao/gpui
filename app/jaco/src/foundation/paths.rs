use std::{ffi::OsString, path::PathBuf};

use crate::{
    app::APP_NAME,
    errors::{JacoError, JacoResult},
};

pub(crate) const CONFIG_DIR_ENV: &str = "JACO_CONFIG_DIR";
pub(crate) const CONFIG_FILE_NAME: &str = "config.toml";
pub(crate) const STATE_FILE_NAME: &str = "state.toml";

pub(crate) fn config_dir() -> JacoResult<PathBuf> {
    config_dir_from(std::env::var_os(CONFIG_DIR_ENV), dirs_next::config_dir())
}

pub(crate) fn config_file() -> JacoResult<PathBuf> {
    Ok(config_dir()?.join(CONFIG_FILE_NAME))
}

pub(crate) fn state_file() -> JacoResult<PathBuf> {
    Ok(config_dir()?.join(STATE_FILE_NAME))
}

pub(crate) fn data_dir() -> JacoResult<PathBuf> {
    data_dir_from(std::env::var_os(CONFIG_DIR_ENV), dirs_next::data_dir())
}

pub(crate) fn database_file() -> JacoResult<PathBuf> {
    Ok(data_dir()?.join(jaco_db::DATABASE_FILE))
}

pub(crate) fn normalize_lexically(path: PathBuf) -> PathBuf {
    let is_absolute = path.is_absolute();
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if normalized
                    .components()
                    .next_back()
                    .is_some_and(|component| matches!(component, std::path::Component::Normal(_)))
                {
                    normalized.pop();
                } else if !is_absolute {
                    normalized.push(component.as_os_str());
                }
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

#[cfg(test)]
fn roots_from(
    override_dir: Option<OsString>,
    config_base: Option<PathBuf>,
    data_base: Option<PathBuf>,
) -> JacoResult<(PathBuf, PathBuf)> {
    let config_dir = config_dir_from(override_dir.clone(), config_base)?;
    let data_dir = data_dir_from(override_dir, data_base)?;
    Ok((config_dir, data_dir))
}

fn config_dir_from(override_dir: Option<OsString>, base: Option<PathBuf>) -> JacoResult<PathBuf> {
    root_from(override_dir, base, JacoError::ConfigDirUnavailable)
}

fn data_dir_from(override_dir: Option<OsString>, base: Option<PathBuf>) -> JacoResult<PathBuf> {
    root_from(override_dir, base, JacoError::DataDirUnavailable)
}

fn root_from(
    override_dir: Option<OsString>,
    base: Option<PathBuf>,
    unavailable: JacoError,
) -> JacoResult<PathBuf> {
    if let Some(override_dir) = override_dir.filter(|value| !value.is_empty()) {
        return std::path::absolute(PathBuf::from(override_dir))
            .map(normalize_lexically)
            .map_err(JacoError::from);
    }
    base.ok_or(unavailable)
        .map(|base| normalize_lexically(base.join(APP_NAME)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_paths_share_root() {
        let current_dir = std::env::current_dir().expect("resolve current directory");
        let root = current_dir.join("tmp/jaco/../isolated");
        let expected = normalize_lexically(current_dir.join("tmp/isolated"));
        let (config, data) = roots_from(
            Some(root.into_os_string()),
            Some(PathBuf::from("/config")),
            Some(PathBuf::from("/data")),
        )
        .expect("resolve override roots");

        assert_eq!(config, expected);
        assert_eq!(data, config);
        assert_eq!(
            config.join(CONFIG_FILE_NAME),
            expected.join(CONFIG_FILE_NAME)
        );
        assert_eq!(config.join(STATE_FILE_NAME), expected.join(STATE_FILE_NAME));
        assert_eq!(
            data.join(jaco_db::DATABASE_FILE),
            expected.join(jaco_db::DATABASE_FILE)
        );
    }

    #[test]
    fn override_does_not_require_platform_bases() {
        let expected = std::env::current_dir()
            .expect("resolve current directory")
            .join("isolated");
        let (config, data) = roots_from(Some(expected.clone().into_os_string()), None, None)
            .expect("override replaces both platform bases");

        assert_eq!(config, expected);
        assert_eq!(data, config);
    }

    #[test]
    fn relative_override_paths_resolve_from_current_directory() {
        let current_dir = std::env::current_dir().expect("resolve current directory");
        let expected = normalize_lexically(current_dir.join("isolated"));
        let (config, data) = roots_from(
            Some(OsString::from("./isolated")),
            Some(PathBuf::from("/config")),
            Some(PathBuf::from("/data")),
        )
        .expect("resolve relative override roots");

        assert_eq!(config, expected);
        assert_eq!(data, expected);
        assert!(config.is_absolute());
    }

    #[test]
    fn parent_relative_override_paths_preserve_parent_directory() {
        let current_dir = std::env::current_dir().expect("resolve current directory");
        let expected = normalize_lexically(current_dir.join("../isolated"));
        let (config, data) = roots_from(
            Some(OsString::from("../isolated")),
            Some(PathBuf::from("/config")),
            Some(PathBuf::from("/data")),
        )
        .expect("resolve parent-relative override roots");

        assert_eq!(config, expected);
        assert_eq!(data, expected);
        assert!(config.is_absolute());
    }

    #[test]
    fn relative_lexical_normalization_preserves_unresolved_parent() {
        assert_eq!(
            normalize_lexically(PathBuf::from("../../isolated")),
            PathBuf::from("../../isolated")
        );
    }

    #[cfg(windows)]
    #[test]
    fn drive_relative_override_becomes_absolute() {
        let (config, data) = roots_from(Some(OsString::from(r"C:isolated")), None, None)
            .expect("resolve drive-relative override root");

        assert!(config.is_absolute());
        assert!(config.ends_with("isolated"));
        assert_eq!(data, config);
    }

    #[test]
    fn production_paths_use_distinct_config_and_data_bases() {
        let (config, data) = roots_from(
            None,
            Some(PathBuf::from("/config-base")),
            Some(PathBuf::from("/data-base")),
        )
        .expect("resolve production roots");

        assert_eq!(config, PathBuf::from("/config-base").join(APP_NAME));
        assert_eq!(data, PathBuf::from("/data-base").join(APP_NAME));
        assert_ne!(config, data);
    }

    #[test]
    fn empty_override_uses_platform_bases() {
        let (config, data) = roots_from(
            Some(OsString::new()),
            Some(PathBuf::from("/config-base")),
            Some(PathBuf::from("/data-base")),
        )
        .expect("resolve roots for empty override");

        assert_eq!(config, PathBuf::from("/config-base").join(APP_NAME));
        assert_eq!(data, PathBuf::from("/data-base").join(APP_NAME));
    }

    #[test]
    fn unavailable_platform_base_identifies_the_missing_root() {
        assert!(matches!(
            roots_from(None, None, Some(PathBuf::from("/data"))),
            Err(JacoError::ConfigDirUnavailable)
        ));
        assert!(matches!(
            roots_from(None, Some(PathBuf::from("/config")), None),
            Err(JacoError::DataDirUnavailable)
        ));
    }
}
