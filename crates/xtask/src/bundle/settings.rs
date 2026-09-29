use crate::error::{Result, XtaskError};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tauri_bundler::{
    AppCategory, BundleSettings, PackageSettings, WixLanguage, WixLanguageConfig, WixSettings,
};
use tauri_utils::config::DeepLinkProtocol;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BundleLocalization {
    pub(crate) locale_tag: &'static str,
    pub(crate) bundle_locale_tag: &'static str,
    pub(crate) source_lproj_dir: &'static str,
    pub(crate) bundle_lproj_dir: &'static str,
    pub(crate) wix_language: &'static str,
    pub(crate) wix_locale_file: &'static str,
}

const BUNDLE_LOCALIZATIONS: [BundleLocalization; 9] = [
    BundleLocalization {
        locale_tag: "en-US",
        bundle_locale_tag: "en",
        source_lproj_dir: "en-US.lproj",
        bundle_lproj_dir: "en.lproj",
        wix_language: "en-US",
        wix_locale_file: "en-US.wxl",
    },
    BundleLocalization {
        locale_tag: "zh-CN",
        bundle_locale_tag: "zh_CN",
        source_lproj_dir: "zh-Hans.lproj",
        bundle_lproj_dir: "zh_CN.lproj",
        wix_language: "zh-CN",
        wix_locale_file: "zh-CN.wxl",
    },
    BundleLocalization {
        locale_tag: "zh-TW",
        bundle_locale_tag: "zh_TW",
        source_lproj_dir: "zh-Hant.lproj",
        bundle_lproj_dir: "zh_TW.lproj",
        wix_language: "zh-TW",
        wix_locale_file: "zh-TW.wxl",
    },
    BundleLocalization {
        locale_tag: "ja",
        bundle_locale_tag: "ja",
        source_lproj_dir: "ja.lproj",
        bundle_lproj_dir: "ja.lproj",
        wix_language: "ja-JP",
        wix_locale_file: "ja-JP.wxl",
    },
    BundleLocalization {
        locale_tag: "ko",
        bundle_locale_tag: "ko",
        source_lproj_dir: "ko.lproj",
        bundle_lproj_dir: "ko.lproj",
        wix_language: "ko-KR",
        wix_locale_file: "ko-KR.wxl",
    },
    BundleLocalization {
        locale_tag: "de",
        bundle_locale_tag: "de",
        source_lproj_dir: "de.lproj",
        bundle_lproj_dir: "de.lproj",
        wix_language: "de-DE",
        wix_locale_file: "de-DE.wxl",
    },
    BundleLocalization {
        locale_tag: "fr",
        bundle_locale_tag: "fr",
        source_lproj_dir: "fr.lproj",
        bundle_lproj_dir: "fr.lproj",
        wix_language: "fr-FR",
        wix_locale_file: "fr-FR.wxl",
    },
    BundleLocalization {
        locale_tag: "es",
        bundle_locale_tag: "es",
        source_lproj_dir: "es.lproj",
        bundle_lproj_dir: "es.lproj",
        wix_language: "es-ES",
        wix_locale_file: "es-ES.wxl",
    },
    BundleLocalization {
        locale_tag: "pt-BR",
        bundle_locale_tag: "pt_BR",
        source_lproj_dir: "pt-BR.lproj",
        bundle_lproj_dir: "pt_BR.lproj",
        wix_language: "pt-BR",
        wix_locale_file: "pt-BR.wxl",
    },
];

const DEFAULT_MACOS_LOCALIZATION_TAGS: [&str; 2] = ["en-US", "zh-CN"];

pub(crate) fn resolve_bundle_localizations(
    declared_localizations: Option<&[String]>,
) -> Result<Vec<BundleLocalization>> {
    let requested = match declared_localizations {
        Some([]) => {
            return Err(XtaskError::msg(
                "bundle.localizations must contain at least one locale",
            ));
        }
        Some(localizations) => localizations.iter().map(String::as_str).collect::<Vec<_>>(),
        None => DEFAULT_MACOS_LOCALIZATION_TAGS.to_vec(),
    };
    let mut seen = std::collections::HashSet::new();
    requested
        .into_iter()
        .map(|tag| {
            if !seen.insert(tag) {
                return Err(XtaskError::msg(format!(
                    "duplicate bundle localization {tag}"
                )));
            }
            BUNDLE_LOCALIZATIONS
                .iter()
                .find(|localization| localization.locale_tag == tag)
                .copied()
                .ok_or_else(|| XtaskError::msg(format!("unsupported bundle localization {tag}")))
        })
        .collect()
}

#[derive(Deserialize)]
struct Manifest {
    package: ManifestPackage,
}

#[derive(Deserialize)]
struct ManifestPackage {
    name: String,
    version: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    homepage: Option<String>,
    #[serde(default)]
    authors: Option<Vec<String>>,
    #[serde(rename = "default-run", default)]
    default_run: Option<String>,
    #[serde(default)]
    license: Option<String>,
    #[serde(rename = "license-file", default)]
    license_file: Option<String>,
    #[serde(default)]
    metadata: Option<ManifestMetadata>,
}

#[derive(Deserialize)]
struct ManifestMetadata {
    #[serde(default)]
    bundle: Option<ManifestBundle>,
}

#[derive(Deserialize)]
struct ManifestBundle {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    identifier: Option<String>,
    #[serde(default)]
    publisher: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    homepage: Option<String>,
    #[serde(default)]
    short_description: Option<String>,
    #[serde(default)]
    long_description: Option<String>,
    #[serde(default)]
    deep_link_protocols: Option<Vec<DeepLinkProtocol>>,
    #[serde(default)]
    localizations: Option<Vec<String>>,
    #[serde(default)]
    deb: Option<ManifestDeb>,
}

#[derive(Deserialize)]
struct ManifestDeb {
    #[serde(default)]
    depends: Option<Vec<String>>,
}

pub fn read_bundle_settings(
    manifest_path: &Path,
) -> Result<(PackageSettings, BundleSettings, Vec<BundleLocalization>)> {
    let content = fs::read_to_string(manifest_path).map_err(|err| {
        XtaskError::msg(format!("failed to read {}: {err}", manifest_path.display()))
    })?;
    let manifest_dir = manifest_path.parent().ok_or_else(|| {
        XtaskError::msg(format!(
            "failed to resolve manifest dir for {}",
            manifest_path.display()
        ))
    })?;
    let manifest: Manifest = toml::from_str(&content).map_err(|err| {
        XtaskError::msg(format!(
            "failed to parse {}: {err}",
            manifest_path.display()
        ))
    })?;

    let Manifest { package } = manifest;
    let ManifestPackage {
        name,
        version,
        description,
        homepage,
        authors,
        default_run,
        license,
        license_file,
        metadata,
    } = package;
    let declared_localizations = metadata
        .as_ref()
        .and_then(|metadata| metadata.bundle.as_ref())
        .and_then(|bundle| bundle.localizations.clone());
    let localizations = resolve_bundle_localizations(declared_localizations.as_deref())?;
    let bundle = metadata.and_then(|metadata| metadata.bundle);

    let product_name = bundle
        .as_ref()
        .and_then(|bundle| bundle.name.clone())
        .unwrap_or(name);
    let description = description.unwrap_or_default();

    let mut bundle_settings = BundleSettings::default();
    if let Some(bundle) = bundle {
        bundle_settings.identifier = bundle.identifier;
        bundle_settings.publisher = bundle.publisher.or_else(|| {
            bundle_settings
                .identifier
                .as_deref()
                .and_then(infer_publisher_from_identifier)
        });
        bundle_settings.category = bundle
            .category
            .as_deref()
            .map(parse_app_category)
            .transpose()?;
        bundle_settings.short_description = bundle.short_description;
        bundle_settings.long_description = bundle.long_description;
        bundle_settings.homepage = bundle.homepage.or_else(|| homepage.clone());
        bundle_settings.deep_link_protocols = bundle.deep_link_protocols;
        if let Some(deb) = bundle.deb {
            bundle_settings.deb.depends = deb.depends;
        }
    }

    if bundle_settings.homepage.is_none() {
        bundle_settings.homepage = homepage.clone();
    }

    bundle_settings.license = license;
    bundle_settings.license_file =
        license_file.map(|path| resolve_manifest_path(manifest_dir, &path));
    bundle_settings.resources_map = Some(macos_bundle_localization_resources(
        manifest_dir,
        &localizations,
    )?);

    if declared_localizations.is_some() {
        let wix_languages = localizations
            .iter()
            .map(|localization| {
                let relative_path =
                    Path::new("build-assets/locales/wix").join(localization.wix_locale_file);
                let locale_path =
                    resolve_manifest_path(manifest_dir, &relative_path.to_string_lossy());
                if !locale_path.is_file() {
                    return Err(XtaskError::msg(format!(
                        "missing WiX localization file {}",
                        locale_path.display()
                    )));
                }
                Ok((
                    localization.wix_language.to_string(),
                    WixLanguageConfig {
                        locale_path: Some(locale_path),
                    },
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        bundle_settings.windows.wix = Some(WixSettings {
            language: WixLanguage(wix_languages),
            ..WixSettings::default()
        });
    }

    let package_settings = PackageSettings {
        product_name,
        version,
        description,
        homepage,
        authors,
        default_run,
    };

    Ok((package_settings, bundle_settings, localizations))
}

fn resolve_manifest_path(manifest_dir: &Path, path: &str) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        manifest_dir.join(path)
    }
}

fn macos_bundle_localization_resources(
    manifest_dir: &Path,
    localizations: &[BundleLocalization],
) -> Result<HashMap<String, String>> {
    let resources_root = fs::canonicalize(manifest_dir.join("locales/macos")).map_err(|err| {
        XtaskError::msg(format!(
            "failed to resolve macOS localization root relative to {}: {err}",
            manifest_dir.display()
        ))
    })?;
    let mut resources_map = HashMap::new();

    for localization in localizations {
        let lproj_dir = resources_root.join(localization.source_lproj_dir);
        if !lproj_dir.exists() {
            return Err(XtaskError::msg(format!(
                "missing macOS localization directory {}",
                lproj_dir.display()
            )));
        }

        for entry in walkdir::WalkDir::new(&lproj_dir) {
            let entry = entry.map_err(|err| {
                XtaskError::msg(format!(
                    "failed to walk macOS localization resources under {}: {err}",
                    lproj_dir.display()
                ))
            })?;

            if !entry.file_type().is_file() {
                continue;
            }

            let path = fs::canonicalize(entry.path()).map_err(|err| {
                XtaskError::msg(format!(
                    "failed to resolve macOS localization resource {}: {err}",
                    entry.path().display()
                ))
            })?;
            let relative = path.strip_prefix(&resources_root).map_err(|err| {
                XtaskError::msg(format!(
                    "failed to strip localization root {} from {}: {err}",
                    resources_root.display(),
                    path.display()
                ))
            })?;
            let destination = Path::new(localization.bundle_lproj_dir).join(
                relative
                    .strip_prefix(localization.source_lproj_dir)
                    .map_err(|err| {
                        XtaskError::msg(format!(
                            "failed to strip localization dir {} from {}: {err}",
                            localization.source_lproj_dir,
                            relative.display()
                        ))
                    })?,
            );

            resources_map.insert(
                path.to_string_lossy().into_owned(),
                destination.to_string_lossy().into_owned(),
            );
        }
    }

    Ok(resources_map)
}

fn infer_publisher_from_identifier(identifier: &str) -> Option<String> {
    let mut parts = identifier.split('.');
    parts.next()?;
    parts
        .next()
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn parse_app_category(category: &str) -> Result<AppCategory> {
    category.parse().map_err(|suggestion| {
        let message = match suggestion {
            Some(suggestion) => {
                format!("invalid bundle category `{category}`, did you mean `{suggestion}`?")
            }
            None => format!("invalid bundle category `{category}`"),
        };
        XtaskError::msg(message)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new() -> Result<Self> {
            let suffix = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
            let path = std::env::temp_dir().join(format!(
                "xtask-bundle-settings-{suffix}-{}",
                std::process::id()
            ));
            fs::create_dir_all(&path)?;
            Ok(Self { path })
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn contains_localization_resource(
        resources_map: &HashMap<String, String>,
        source_suffix: &Path,
        destination: &Path,
    ) -> bool {
        resources_map.iter().any(|(source, mapped_destination)| {
            Path::new(source).ends_with(source_suffix)
                && Path::new(mapped_destination) == destination
        })
    }

    #[test]
    fn declared_localizations_are_per_app_and_configure_wix_languages() -> Result<()> {
        let temp_dir = TestDir::new()?;
        let locales = [
            ("en-US", "en-US.lproj", "en.lproj", "en-US"),
            ("zh-CN", "zh-Hans.lproj", "zh_CN.lproj", "zh-CN"),
            ("zh-TW", "zh-Hant.lproj", "zh_TW.lproj", "zh-TW"),
            ("ja", "ja.lproj", "ja.lproj", "ja-JP"),
            ("ko", "ko.lproj", "ko.lproj", "ko-KR"),
            ("de", "de.lproj", "de.lproj", "de-DE"),
            ("fr", "fr.lproj", "fr.lproj", "fr-FR"),
            ("es", "es.lproj", "es.lproj", "es-ES"),
            ("pt-BR", "pt-BR.lproj", "pt_BR.lproj", "pt-BR"),
        ];
        let macos_root = temp_dir.path.join("locales/macos");
        let wix_root = temp_dir.path.join("build-assets/locales/wix");
        for (_, source_dir, _, wix_locale) in locales {
            fs::create_dir_all(macos_root.join(source_dir))?;
            fs::write(
                macos_root.join(source_dir).join("InfoPlist.strings"),
                "\"CFBundleName\" = \"Gupi\";\n",
            )?;
            fs::create_dir_all(&wix_root)?;
            fs::write(
                wix_root.join(format!("{wix_locale}.wxl")),
                "<WixLocalization></WixLocalization>",
            )?;
        }

        let manifest_path = temp_dir.path.join("Cargo.toml");
        fs::write(
            &manifest_path,
            r#"[package]
name = "gupi"
version = "0.1.0"

[package.metadata.bundle]
name = "Gupi"
localizations = ["en-US", "zh-CN", "zh-TW", "ja", "ko", "de", "fr", "es", "pt-BR"]
"#,
        )?;

        let (_, bundle_settings, resolved) = read_bundle_settings(&manifest_path)?;
        assert_eq!(resolved.len(), 9);
        let resources = bundle_settings.resources_map.as_ref().unwrap();
        assert_eq!(resources.len(), 9);
        assert!(contains_localization_resource(
            resources,
            Path::new("locales/macos/zh-Hant.lproj/InfoPlist.strings"),
            Path::new("zh_TW.lproj/InfoPlist.strings"),
        ));

        let wix_languages = &bundle_settings.windows.wix.as_ref().unwrap().language.0;
        assert_eq!(
            wix_languages
                .iter()
                .map(|(language, _)| language.as_str())
                .collect::<Vec<_>>(),
            vec![
                "en-US", "zh-CN", "zh-TW", "ja-JP", "ko-KR", "de-DE", "fr-FR", "es-ES", "pt-BR",
            ]
        );
        assert!(wix_languages.iter().all(|(_, config)| {
            config
                .locale_path
                .as_ref()
                .is_some_and(|path| path.is_file())
        }));
        Ok(())
    }

    #[test]
    fn unsupported_or_duplicate_bundle_localizations_fail() {
        assert!(resolve_bundle_localizations(Some(&["en-US".into(), "en-US".into()])).is_err());
        assert!(resolve_bundle_localizations(Some(&["en-GB".into()])).is_err());
        assert!(resolve_bundle_localizations(Some(&[])).is_err());
    }

    #[allow(deprecated)]
    #[test]
    fn read_bundle_settings_resolves_relative_bundle_paths() -> Result<()> {
        let temp_dir = TestDir::new()?;
        let i18n_dir = temp_dir.path.join("locales/macos");
        fs::create_dir_all(i18n_dir.join("en-US.lproj"))?;
        fs::create_dir_all(i18n_dir.join("zh-Hans.lproj"))?;
        fs::write(
            i18n_dir.join("en-US.lproj/InfoPlist.strings"),
            "\"CFBundleName\" = \"Jaco\";\n",
        )?;
        fs::write(
            i18n_dir.join("zh-Hans.lproj/InfoPlist.strings"),
            "\"CFBundleName\" = \"Jaco\";\n",
        )?;

        let manifest_path = temp_dir.path.join("Cargo.toml");
        fs::write(
            &manifest_path,
            r#"[package]
name = "jaco"
version = "0.1.0"
license-file = "LICENSE"

[package.metadata.bundle]
name = "Jaco"
identifier = "top.sushao.jaco"
category = "DeveloperTool"
deep_link_protocols = [{ schemes = ["jaco-screenclip"] }]

[package.metadata.bundle.deb]
depends = ["libasound2"]
"#,
        )?;

        let (_, bundle_settings, localizations) = read_bundle_settings(&manifest_path)?;
        assert_eq!(localizations.len(), 2);
        assert!(bundle_settings.windows.wix.is_none());

        assert!(bundle_settings.icon.is_none());
        assert_eq!(
            bundle_settings.license_file,
            Some(temp_dir.path.join("LICENSE"))
        );
        assert_eq!(bundle_settings.category, Some(AppCategory::DeveloperTool));
        assert_eq!(
            bundle_settings.deb.depends,
            Some(vec!["libasound2".to_string()])
        );
        assert_eq!(
            bundle_settings
                .deep_link_protocols
                .as_ref()
                .expect("deep link protocols should be present")[0]
                .schemes,
            vec!["jaco-screenclip".to_string()]
        );
        let resources_map = bundle_settings
            .resources_map
            .as_ref()
            .expect("macOS localization resources should be present");
        assert!(contains_localization_resource(
            resources_map,
            &Path::new("locales").join("macos/en-US.lproj/InfoPlist.strings"),
            &Path::new("en.lproj").join("InfoPlist.strings"),
        ));
        assert!(contains_localization_resource(
            resources_map,
            &Path::new("locales").join("macos/zh-Hans.lproj/InfoPlist.strings"),
            &Path::new("zh_CN.lproj").join("InfoPlist.strings"),
        ));

        Ok(())
    }
}
