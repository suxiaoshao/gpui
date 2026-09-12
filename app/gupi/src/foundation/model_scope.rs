//! Read-only projection of Pi's configured model scope. RPC returns the full catalog.
use glob::{MatchOptions, Pattern};
use pi_rpc::protocol::Model;
use serde::Deserialize;
use std::{fs, io, path::Path};

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Settings {
    enabled_models: Option<Vec<String>>,
    default_project_trust: Option<String>,
}

fn read_json<T: serde::de::DeserializeOwned + Default>(path: &Path) -> io::Result<T> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(text.trim_start_matches('\u{feff}'))
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(T::default()),
        Err(error) => Err(error),
    }
}

pub(crate) fn load(agent: &Path, cwd: &Path) -> io::Result<Option<Vec<String>>> {
    let global: Settings = read_json(&agent.join("settings.json"))?;
    let project_path = cwd.join(".pi/settings.json");
    if !project_path.exists() {
        return Ok(global.enabled_models);
    }
    // Pi RPC has no trust prompt. Use its persisted nearest-parent decision,
    // then the global default; an unanswered "ask" does not load project settings.
    let trust: std::collections::BTreeMap<String, Option<bool>> =
        read_json(&agent.join("trust.json"))?;
    let canonical = fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_owned());
    let trusted = canonical
        .ancestors()
        .find_map(|path| {
            trust
                .get(path.to_string_lossy().as_ref())
                .copied()
                .flatten()
        })
        .unwrap_or(global.default_project_trust.as_deref() == Some("always"));
    if trusted {
        let project: Settings = read_json(&project_path)?;
        Ok(project.enabled_models.or(global.enabled_models))
    } else {
        Ok(global.enabled_models)
    }
}

fn exact<'a>(pattern: &str, models: &'a [Model]) -> Option<&'a Model> {
    let pattern = pattern.trim();
    let unique = |matches: Vec<&'a Model>| {
        if matches.len() == 1 {
            Some(matches[0])
        } else {
            None
        }
    };
    let canonical: Vec<_> = models
        .iter()
        .filter(|model| {
            format!("{}/{}", model.provider, model.id).eq_ignore_ascii_case(pattern)
                || pattern.split_once('/').is_some_and(|(provider, id)| {
                    model.provider.eq_ignore_ascii_case(provider.trim())
                        && model.id.eq_ignore_ascii_case(id.trim())
                })
        })
        .collect();
    if !canonical.is_empty() {
        return unique(canonical);
    }
    unique(
        models
            .iter()
            .filter(|m| m.id.eq_ignore_ascii_case(pattern))
            .collect(),
    )
}

fn alias(id: &str) -> bool {
    !id.rsplit_once('-')
        .is_some_and(|(_, suffix)| suffix.len() == 8 && suffix.bytes().all(|c| c.is_ascii_digit()))
}

fn partial<'a>(pattern: &str, models: &'a [Model]) -> Option<&'a Model> {
    let mut pattern = pattern;
    loop {
        if let Some(model) = exact(pattern, models) {
            return Some(model);
        }
        let query = pattern.to_lowercase();
        let mut matches: Vec<_> = models
            .iter()
            .filter(|m| {
                m.id.to_lowercase().contains(&query) || m.name.to_lowercase().contains(&query)
            })
            .collect();
        // Pi prefers an undated alias, then the highest model id.
        matches.sort_by(|a, b| {
            alias(&b.id)
                .cmp(&alias(&a.id))
                .then_with(|| b.id.cmp(&a.id))
        });
        if let Some(model) = matches.first() {
            return Some(model);
        }
        // Try the full id before removing a thinking suffix, preserving :exacto ids.
        pattern = pattern.rsplit_once(':')?.0;
    }
}

fn is_thinking_level(value: &str) -> bool {
    matches!(
        value,
        "off" | "minimal" | "low" | "medium" | "high" | "xhigh" | "max"
    )
}

pub(crate) fn filter(models: Vec<Model>, patterns: Option<&[String]>) -> Vec<Model> {
    let Some(patterns) = patterns.filter(|patterns| !patterns.is_empty()) else {
        return models;
    };
    let mut selected = std::collections::HashSet::new();
    for pattern in patterns {
        if pattern.contains(['*', '?', '[']) {
            let pattern = pattern
                .rsplit_once(':')
                .filter(|(_, suffix)| is_thinking_level(suffix))
                .map_or(pattern.as_str(), |(pattern, _)| pattern);
            if let Some(model) = exact(pattern, &models) {
                selected.insert((model.provider.clone(), model.id.clone()));
                continue;
            }
            let Ok(pattern) = Pattern::new(pattern) else {
                continue;
            };
            let options = MatchOptions {
                case_sensitive: false,
                require_literal_separator: true,
                require_literal_leading_dot: true,
            };
            for model in &models {
                if pattern.matches_with(&format!("{}/{}", model.provider, model.id), options)
                    || pattern.matches_with(&model.id, options)
                {
                    selected.insert((model.provider.clone(), model.id.clone()));
                }
            }
        } else if let Some(model) = partial(pattern, &models) {
            selected.insert((model.provider.clone(), model.id.clone()));
        }
    }
    // A configured range with no matches stays empty: never reveal excluded models.
    models
        .into_iter()
        .filter(|m| selected.contains(&(m.provider.clone(), m.id.clone())))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn model(provider: &str, id: &str) -> Model {
        Model {
            provider: provider.into(),
            id: id.into(),
            name: id.into(),
            reasoning: true,
            context_window: 0,
            extra: Default::default(),
        }
    }
    fn ids(models: Vec<Model>, patterns: &[&str]) -> Vec<String> {
        filter(
            models,
            Some(&patterns.iter().map(|p| p.to_string()).collect::<Vec<_>>()),
        )
        .into_iter()
        .map(|m| format!("{}/{}", m.provider, m.id))
        .collect()
    }
    #[test]
    fn scope_excludes_models_and_matches_provider_id_globs_and_thinking_suffixes() {
        let models = vec![
            model("deepseek", "v4"),
            model("zai-coding-cn", "glm-5.3"),
            model("zai-coding-cn", "glm-4.7"),
            model("openai-codex", "gpt-5.5"),
            model("other", "hidden"),
        ];
        assert_eq!(
            ids(
                models.clone(),
                &[
                    "DEEPSEEK/*:high",
                    "zai-coding-cn/glm-5.3*",
                    "gpt-?.[45]",
                    "deepseek/*"
                ]
            ),
            [
                "deepseek/v4",
                "zai-coding-cn/glm-5.3",
                "openai-codex/gpt-5.5"
            ]
        );
        assert!(ids(models.clone(), &["missing/*"]).is_empty());
        assert_eq!(filter(models.clone(), None).len(), 5);
        assert_eq!(ids(models, &[]).len(), 5);
    }
    #[test]
    fn exact_colon_ids_precede_suffixes_and_aliases_precede_dates() {
        let models = vec![
            model("p", "sonnet-20260910"),
            model("p", "sonnet"),
            model("p", "m:exacto"),
        ];
        assert_eq!(
            ids(models, &["sonnet:high", "p/m:exacto"]),
            ["p/sonnet", "p/m:exacto"]
        );
    }
    #[test]
    fn project_scope_requires_trust_and_replaces_global_including_empty_array() {
        let tmp = tempfile::tempdir().unwrap();
        let agent = tmp.path().join("agent");
        let project = tmp.path().join("project");
        fs::create_dir_all(&agent).unwrap();
        fs::create_dir_all(project.join(".pi")).unwrap();
        fs::write(
            agent.join("settings.json"),
            r#"{"enabledModels":["global/*"]}"#,
        )
        .unwrap();
        fs::write(
            project.join(".pi/settings.json"),
            r#"{"enabledModels":["project/*"]}"#,
        )
        .unwrap();
        assert_eq!(load(&agent, &project).unwrap().unwrap(), ["global/*"]);
        fs::write(
            agent.join("trust.json"),
            serde_json::to_vec(&serde_json::json!({
                fs::canonicalize(tmp.path()).unwrap().to_str().unwrap(): true
            }))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(load(&agent, &project).unwrap().unwrap(), ["project/*"]);
        fs::write(project.join(".pi/settings.json"), r#"{"enabledModels":[]}"#).unwrap();
        assert!(load(&agent, &project).unwrap().unwrap().is_empty());
        fs::write(project.join(".pi/settings.json"), "invalid").unwrap();
        assert!(load(&agent, &project).is_err());
    }
}
