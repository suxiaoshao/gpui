use crate::{
    AgentRuntimeError, LocalTool, Result, ToolDefinition, ToolExecutor, ToolRunPolicy,
    builtin_tools::{
        approval::{normalize_for_access, path_access_request, resolve_tool_path},
        types::{
            BuiltinToolContext, BuiltinToolName, FileEntryKind, FindPathInput, FindPathOutput,
            GrepContextLineOutput, GrepInput, GrepMatchOutput, GrepOutput, PathSearchMatchOutput,
            TextRangeOutput, find_path_schema, grep_schema, output_with_structured,
        },
    },
};
use async_trait::async_trait;
use globset::Glob;
use grep_matcher::Matcher;
use grep_regex::RegexMatcherBuilder;
use grep_searcher::{Searcher, SearcherBuilder, Sink, SinkContext, SinkContextKind, SinkMatch};
use ignore::WalkBuilder;
use jaco_core::{ToolAccessKind, ToolExecutionPolicy, ToolInvocationOutput, ToolSource};
use std::{
    fs,
    path::{Path, PathBuf},
};

const DEFAULT_MAX_RESULTS: usize = 200;

#[derive(Clone, Debug)]
pub struct FindPathTool {
    context: BuiltinToolContext,
}

#[derive(Clone, Debug)]
pub struct GrepTool {
    context: BuiltinToolContext,
}

impl FindPathTool {
    pub fn new(context: BuiltinToolContext) -> Self {
        Self { context }
    }
}

impl GrepTool {
    pub fn new(context: BuiltinToolContext) -> Self {
        Self { context }
    }
}

#[async_trait]
impl ToolExecutor for FindPathTool {
    async fn execute(&self, arguments: serde_json::Value) -> Result<ToolInvocationOutput> {
        let input: FindPathInput = serde_json::from_value(arguments)?;
        let root = search_root(input.path.as_deref(), &self.context)?;
        let max_results = input.max_results.unwrap_or(DEFAULT_MAX_RESULTS);
        let (matches, truncated) =
            find_path_matches(&root, &input.query, input.include_hidden, max_results)?;
        let output = FindPathOutput {
            query: input.query,
            matches,
            truncated,
        };
        output_with_structured(
            format!(
                "Found {} path match(es){}",
                output.matches.len(),
                if output.truncated { " (truncated)" } else { "" }
            ),
            output,
        )
    }
}

impl LocalTool for FindPathTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            source: ToolSource::Local,
            namespace: None,
            name: BuiltinToolName::FindPath.as_str().to_string(),
            description: "Find files or directories by path name under the project.".to_string(),
            parameters: find_path_schema(),
            policy: local_tool_policy(),
        }
    }
}

#[async_trait]
impl ToolExecutor for GrepTool {
    async fn execute(&self, arguments: serde_json::Value) -> Result<ToolInvocationOutput> {
        let input: GrepInput = serde_json::from_value(arguments)?;
        let root = search_root(input.path.as_deref(), &self.context)?;
        let max_results = input.max_results.unwrap_or(DEFAULT_MAX_RESULTS);
        let context_lines = input.context_lines.unwrap_or(0);
        let (matches, truncated) = grep_matches(&root, &input, max_results, context_lines)?;
        let output = GrepOutput {
            pattern: input.pattern,
            matches,
            truncated,
        };
        output_with_structured(
            format!(
                "Found {} code match(es){}",
                output.matches.len(),
                if output.truncated { " (truncated)" } else { "" }
            ),
            output,
        )
    }
}

impl LocalTool for GrepTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            source: ToolSource::Local,
            namespace: None,
            name: BuiltinToolName::Grep.as_str().to_string(),
            description: "Search file contents with a regular expression.".to_string(),
            parameters: grep_schema(),
            policy: local_tool_policy(),
        }
    }
}

pub fn access_requests(
    tool: BuiltinToolName,
    arguments: &serde_json::Value,
    context: &BuiltinToolContext,
) -> Result<Vec<crate::builtin_tools::types::PathAccessRequest>> {
    match tool {
        BuiltinToolName::FindPath => {
            let input: FindPathInput = serde_json::from_value(arguments.clone())?;
            Ok(vec![path_access_request(
                ToolAccessKind::Read,
                input.path.unwrap_or_else(|| ".".to_string()),
                context.project_root.as_deref(),
                Some("find_path"),
            )?])
        }
        BuiltinToolName::Grep => {
            let input: GrepInput = serde_json::from_value(arguments.clone())?;
            Ok(vec![path_access_request(
                ToolAccessKind::Read,
                input.path.unwrap_or_else(|| ".".to_string()),
                context.project_root.as_deref(),
                Some("grep"),
            )?])
        }
        BuiltinToolName::ReadFile
        | BuiltinToolName::ListDirectory
        | BuiltinToolName::WriteFile
        | BuiltinToolName::EditFile => Ok(Vec::new()),
    }
}

fn local_tool_policy() -> ToolRunPolicy {
    ToolRunPolicy {
        approval_policy: jaco_core::ToolApprovalPolicy::Never,
        execution_policy: ToolExecutionPolicy::Foreground,
        timeout_ms: None,
    }
}

fn search_root(path: Option<&str>, context: &BuiltinToolContext) -> Result<PathBuf> {
    let path = path.unwrap_or(".");
    let resolved = resolve_tool_path(path, context.project_root.as_deref())?;
    normalize_for_access(&resolved)
}

fn find_path_matches(
    root: &Path,
    query: &str,
    include_hidden: bool,
    max_results: usize,
) -> Result<(Vec<PathSearchMatchOutput>, bool)> {
    let query_lower = query.to_lowercase();
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(!include_hidden)
        .git_ignore(true)
        .parents(true);
    let mut matches = Vec::new();
    for result in builder.build() {
        let entry = result.map_err(|err| std::io::Error::other(err.to_string()))?;
        let path = entry.path();
        let path_text = path.to_string_lossy();
        let path_lower = path_text.to_lowercase();
        let Some(score) = path_score(&path_lower, &query_lower) else {
            continue;
        };
        matches.push(PathSearchMatchOutput {
            path: path_text.into_owned(),
            kind: file_entry_kind(path)?,
            score: Some(score),
        });
    }
    Ok(sort_and_truncate_path_matches(matches, max_results))
}

fn sort_and_truncate_path_matches(
    mut matches: Vec<PathSearchMatchOutput>,
    max_results: usize,
) -> (Vec<PathSearchMatchOutput>, bool) {
    matches.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.path.cmp(&right.path))
    });
    let truncated = matches.len() > max_results;
    matches.truncate(max_results);
    (matches, truncated)
}

fn grep_matches(
    root: &Path,
    input: &GrepInput,
    max_results: usize,
    context_lines: u32,
) -> Result<(Vec<GrepMatchOutput>, bool)> {
    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(!input.case_sensitive.unwrap_or(true))
        .build(&input.pattern)
        .map_err(|err| AgentRuntimeError::Unsupported(format!("invalid grep pattern: {err}")))?;
    let glob = match input.glob.as_deref() {
        Some(glob) => Some(
            Glob::new(glob)
                .map_err(|err| AgentRuntimeError::Unsupported(format!("invalid glob: {err}")))?
                .compile_matcher(),
        ),
        None => None,
    };
    let mut matches = Vec::new();
    let collection_limit = max_results.saturating_add(1);
    if root.is_file() {
        search_file(
            root,
            &matcher,
            collection_limit,
            context_lines,
            &mut matches,
        )?;
        let truncated = matches.len() > max_results;
        matches.truncate(max_results);
        return Ok((matches, truncated));
    }

    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(!input.include_hidden)
        .git_ignore(true)
        .parents(true);
    for result in builder.build() {
        if matches.len() >= collection_limit {
            break;
        }
        let entry = result.map_err(|err| std::io::Error::other(err.to_string()))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if let Some(glob) = &glob
            && !glob.is_match(path)
        {
            continue;
        }
        search_file(
            path,
            &matcher,
            collection_limit,
            context_lines,
            &mut matches,
        )?;
    }
    let truncated = matches.len() > max_results;
    matches.truncate(max_results);
    Ok((matches, truncated))
}

fn search_file(
    path: &Path,
    matcher: &grep_regex::RegexMatcher,
    max_results: usize,
    context_lines: u32,
    matches: &mut Vec<GrepMatchOutput>,
) -> Result<()> {
    let context_lines = context_lines as usize;
    let mut searcher = SearcherBuilder::new()
        .line_number(true)
        .before_context(context_lines)
        .after_context(context_lines)
        .build();
    let mut sink = CollectSink {
        path,
        matcher,
        max_results,
        context_lines,
        pending_context_before: Vec::new(),
        last_match_index: None,
        matches,
    };
    searcher
        .search_path(matcher, path, &mut sink)
        .map_err(|err| std::io::Error::other(err.to_string()))?;
    Ok(())
}

struct CollectSink<'a, 'm> {
    path: &'a Path,
    matcher: &'m grep_regex::RegexMatcher,
    max_results: usize,
    context_lines: usize,
    pending_context_before: Vec<GrepContextLineOutput>,
    last_match_index: Option<usize>,
    matches: &'a mut Vec<GrepMatchOutput>,
}

impl Sink for CollectSink<'_, '_> {
    type Error = std::io::Error;

    fn matched(&mut self, _searcher: &Searcher, mat: &SinkMatch<'_>) -> std::io::Result<bool> {
        if self.matches.len() >= self.max_results {
            return Ok(false);
        }

        let bytes = mat.bytes();
        let line = grep_line_text(bytes);
        let mut ranges = Vec::new();
        self.matcher
            .find_iter(bytes, |range| {
                ranges.push(TextRangeOutput {
                    start: range.start(),
                    end: range.end(),
                });
                true
            })
            .map_err(|err| std::io::Error::other(err.to_string()))?;
        self.matches.push(GrepMatchOutput {
            path: self.path.to_string_lossy().into_owned(),
            line_number: mat.line_number().unwrap_or(0) as u32,
            line,
            ranges,
            context_before: std::mem::take(&mut self.pending_context_before),
            context_after: Vec::new(),
        });
        self.last_match_index = Some(self.matches.len() - 1);
        Ok(self.context_lines > 0 || self.matches.len() < self.max_results)
    }

    fn context(
        &mut self,
        _searcher: &Searcher,
        context: &SinkContext<'_>,
    ) -> std::io::Result<bool> {
        let context_line = GrepContextLineOutput {
            line_number: context.line_number().unwrap_or(0) as u32,
            line: grep_line_text(context.bytes()),
        };
        match context.kind() {
            SinkContextKind::Before => {
                self.pending_context_before.push(context_line);
                Ok(true)
            }
            SinkContextKind::After => {
                let Some(last_match_index) = self.last_match_index else {
                    return Ok(true);
                };
                if let Some(last_match) = self.matches.get_mut(last_match_index) {
                    last_match.context_after.push(context_line);
                }
                Ok(self.matches.len() < self.max_results
                    || self
                        .matches
                        .get(last_match_index)
                        .map(|last_match| last_match.context_after.len() < self.context_lines)
                        .unwrap_or(false))
            }
            SinkContextKind::Other => Ok(true),
        }
    }

    fn context_break(&mut self, _searcher: &Searcher) -> std::io::Result<bool> {
        self.pending_context_before.clear();
        Ok(self.matches.len() < self.max_results)
    }
}

fn grep_line_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .trim_end_matches(['\r', '\n'])
        .to_string()
}

fn file_entry_kind(path: &Path) -> Result<FileEntryKind> {
    let metadata = fs::symlink_metadata(path)?;
    let file_type = metadata.file_type();
    Ok(if file_type.is_symlink() {
        FileEntryKind::Symlink
    } else if file_type.is_dir() {
        FileEntryKind::Directory
    } else if file_type.is_file() {
        FileEntryKind::File
    } else {
        FileEntryKind::Other
    })
}

fn path_score(path: &str, query: &str) -> Option<f64> {
    if query.is_empty() {
        return Some(0.0);
    }
    if path.contains(query) {
        return Some(1.0 + query.len() as f64 / path.len().max(1) as f64);
    }
    let mut chars = query.chars();
    let mut current = chars.next()?;
    for path_char in path.chars() {
        if path_char == current {
            match chars.next() {
                Some(next) => current = next,
                None => return Some(0.5),
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ToolExecutor;
    use serde_json::json;
    use tempfile::tempdir;

    #[tokio::test]
    async fn find_path_and_grep_use_project_root() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(
            src.join("main.rs"),
            "fn main() {\n    println!(\"hello\");\n}\n",
        )
        .unwrap();
        fs::write(dir.path().join("README.md"), "hello docs\n").unwrap();
        let context = BuiltinToolContext {
            project_root: Some(dir.path().to_path_buf()),
        };

        let found = FindPathTool::new(context.clone())
            .execute(json!({
                "query": "main.rs",
                "maxResults": 5,
            }))
            .await
            .unwrap();
        let found_output: FindPathOutput =
            serde_json::from_value(found.structured_output.unwrap().value).unwrap();
        assert!(
            found_output
                .matches
                .iter()
                .any(|entry| entry.path.ends_with("main.rs"))
        );

        let grep = GrepTool::new(context)
            .execute(json!({
                "pattern": "println",
                "glob": "*.rs",
                "maxResults": 5,
            }))
            .await
            .unwrap();
        let grep_output: GrepOutput =
            serde_json::from_value(grep.structured_output.unwrap().value).unwrap();
        assert_eq!(grep_output.matches.len(), 1);
        assert_eq!(grep_output.matches[0].line_number, 2);
        assert!(grep_output.matches[0].context_before.is_empty());
        assert!(grep_output.matches[0].context_after.is_empty());
    }

    #[tokio::test]
    async fn grep_honors_context_lines() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("main.rs"),
            [
                "fn main() {",
                "    let message = \"hello\";",
                "    println!(\"{message}\");",
                "    finish();",
                "}",
            ]
            .join("\n"),
        )
        .unwrap();
        let context = BuiltinToolContext {
            project_root: Some(dir.path().to_path_buf()),
        };

        let grep = GrepTool::new(context)
            .execute(json!({
                "pattern": "println",
                "glob": "*.rs",
                "contextLines": 1,
                "maxResults": 5,
            }))
            .await
            .unwrap();
        let grep_output: GrepOutput =
            serde_json::from_value(grep.structured_output.unwrap().value).unwrap();

        assert_eq!(grep_output.matches.len(), 1);
        let grep_match = &grep_output.matches[0];
        assert_eq!(grep_match.line_number, 3);
        assert_eq!(grep_match.line, "    println!(\"{message}\");");
        assert_eq!(grep_match.context_before.len(), 1);
        assert_eq!(grep_match.context_before[0].line_number, 2);
        assert_eq!(
            grep_match.context_before[0].line,
            "    let message = \"hello\";"
        );
        assert_eq!(grep_match.context_after.len(), 1);
        assert_eq!(grep_match.context_after[0].line_number, 4);
        assert_eq!(grep_match.context_after[0].line, "    finish();");
    }

    #[tokio::test]
    async fn grep_marks_truncated_only_after_omitting_a_match() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("main.rs"),
            ["alpha one", "beta", "alpha two"].join("\n"),
        )
        .unwrap();
        let context = BuiltinToolContext {
            project_root: Some(dir.path().to_path_buf()),
        };

        let exact = GrepTool::new(context.clone())
            .execute(json!({
                "pattern": "alpha one",
                "glob": "*.rs",
                "maxResults": 1,
            }))
            .await
            .unwrap();
        let exact_output: GrepOutput =
            serde_json::from_value(exact.structured_output.unwrap().value).unwrap();
        assert_eq!(exact_output.matches.len(), 1);
        assert!(!exact_output.truncated);

        let truncated = GrepTool::new(context)
            .execute(json!({
                "pattern": "alpha",
                "glob": "*.rs",
                "maxResults": 1,
            }))
            .await
            .unwrap();
        let truncated_output: GrepOutput =
            serde_json::from_value(truncated.structured_output.unwrap().value).unwrap();
        assert_eq!(truncated_output.matches.len(), 1);
        assert!(truncated_output.truncated);
    }

    #[test]
    fn find_path_sorts_matches_before_truncating() {
        let (matches, truncated) = sort_and_truncate_path_matches(
            vec![
                PathSearchMatchOutput {
                    path: "/repo/src/generated/current_user_profile.rs".to_string(),
                    kind: FileEntryKind::File,
                    score: Some(1.1),
                },
                PathSearchMatchOutput {
                    path: "/repo/src/user.rs".to_string(),
                    kind: FileEntryKind::File,
                    score: Some(1.5),
                },
                PathSearchMatchOutput {
                    path: "/repo/tests/user_flow.rs".to_string(),
                    kind: FileEntryKind::File,
                    score: Some(1.3),
                },
            ],
            1,
        );

        assert!(truncated);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].path, "/repo/src/user.rs");
    }

    #[tokio::test]
    async fn grep_rejects_invalid_regex_as_structured_error() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();
        let context = BuiltinToolContext {
            project_root: Some(dir.path().to_path_buf()),
        };

        let err = GrepTool::new(context)
            .execute(json!({
                "pattern": "[",
            }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("invalid grep pattern"));
    }
}
