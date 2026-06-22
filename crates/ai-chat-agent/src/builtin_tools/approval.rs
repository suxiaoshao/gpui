use crate::{AgentRuntimeError, Result, builtin_tools::types::PathAccessRequest};
use ai_chat_core::{
    ToolAccessKind, ToolAccessRequestPayload, ToolApprovalMode, ToolPermissionScopeSnapshot,
    ToolPolicySnapshot,
};
use std::{
    path::{Component, Path, PathBuf},
    sync::Arc,
};

#[derive(Clone, Debug)]
pub struct ToolPermissionEvaluator {
    project_roots: Arc<Vec<PathBuf>>,
    approval_mode: ToolApprovalMode,
    external_read_requires_approval: bool,
    external_write_requires_approval: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ToolPermissionDecision {
    Allow {
        auto_approved: Vec<ToolAccessRequestPayload>,
    },
    Ask {
        reason: String,
        access_requests: Vec<ToolAccessRequestPayload>,
    },
    Deny {
        reason: String,
    },
}

impl ToolPermissionEvaluator {
    pub fn from_policy(
        policy: &ToolPolicySnapshot,
        fallback_project_root: Option<&Path>,
    ) -> Result<Self> {
        let scope = policy.permission_scope.as_ref();
        let project_roots = project_roots(scope, fallback_project_root)?;
        Ok(Self {
            project_roots: Arc::new(project_roots),
            approval_mode: policy.approval_mode,
            external_read_requires_approval: scope
                .map(|scope| scope.external_read_requires_approval)
                .unwrap_or(false),
            external_write_requires_approval: scope
                .map(|scope| scope.external_write_requires_approval)
                .unwrap_or(true),
        })
    }

    pub fn evaluate(&self, requests: &[PathAccessRequest]) -> ToolPermissionDecision {
        if self.approval_mode == ToolApprovalMode::FullAccess {
            return ToolPermissionDecision::Allow {
                auto_approved: Vec::new(),
            };
        }

        if self.project_roots.is_empty()
            && requests
                .iter()
                .any(|request| matches!(request.kind, ToolAccessKind::Write))
        {
            return ToolPermissionDecision::Deny {
                reason: "Write tools require a project root unless Full Access is selected"
                    .to_string(),
            };
        }

        let mut approval_required = Vec::new();
        for request in requests {
            let within_project = self.is_within_project(&request.normalized_path);
            let payload = ToolAccessRequestPayload {
                kind: request.kind,
                target: request.target.clone(),
                normalized_path: Some(request.normalized_path.to_string_lossy().into_owned()),
                within_project,
                reason_key: request.reason_key.clone(),
            };
            if !within_project && self.requires_external_approval(request.kind) {
                approval_required.push(payload);
            }
        }

        if approval_required.is_empty() {
            return ToolPermissionDecision::Allow {
                auto_approved: Vec::new(),
            };
        }

        match self.approval_mode {
            ToolApprovalMode::AutoApprove => ToolPermissionDecision::Allow {
                auto_approved: approval_required,
            },
            ToolApprovalMode::RequestApproval => ToolPermissionDecision::Ask {
                reason: approval_reason(&approval_required),
                access_requests: approval_required,
            },
            ToolApprovalMode::FullAccess => unreachable!(),
        }
    }

    pub fn is_within_project(&self, path: &Path) -> bool {
        self.project_roots
            .iter()
            .any(|project_root| path.starts_with(project_root))
    }

    fn requires_external_approval(&self, kind: ToolAccessKind) -> bool {
        match kind {
            ToolAccessKind::Read => self.external_read_requires_approval,
            ToolAccessKind::Write => self.external_write_requires_approval,
            ToolAccessKind::Execute | ToolAccessKind::Network => true,
        }
    }
}

pub fn path_access_request(
    kind: ToolAccessKind,
    path: impl AsRef<str>,
    project_root: Option<&Path>,
    reason_key: Option<&str>,
) -> Result<PathAccessRequest> {
    let target = path.as_ref().to_string();
    let resolved = resolve_tool_path(&target, project_root)?;
    let normalized_path = normalize_for_access(&resolved)?;
    Ok(PathAccessRequest {
        kind,
        target,
        normalized_path,
        within_project: false,
        reason_key: reason_key.map(str::to_string),
    })
}

pub fn resolve_tool_path(path: &str, project_root: Option<&Path>) -> Result<PathBuf> {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        return Ok(path);
    }
    let Some(project_root) = project_root else {
        return Err(AgentRuntimeError::Invariant(
            "relative tool path requires a project root".to_string(),
        ));
    };
    Ok(project_root.join(path))
}

pub fn normalize_for_access(path: &Path) -> Result<PathBuf> {
    if let Ok(canonical) = path.canonicalize() {
        return Ok(canonical);
    }

    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };

    let mut missing = Vec::new();
    let mut cursor = absolute.as_path();
    while !cursor.exists() {
        let Some(file_name) = cursor.file_name() else {
            return Ok(normalize_lexically(&absolute));
        };
        missing.push(file_name.to_os_string());
        let Some(parent) = cursor.parent() else {
            return Ok(normalize_lexically(&absolute));
        };
        cursor = parent;
    }

    let mut normalized = cursor.canonicalize()?;
    for component in missing.iter().rev() {
        normalized.push(component);
    }
    Ok(normalize_lexically(&normalized))
}

fn project_roots(
    scope: Option<&ToolPermissionScopeSnapshot>,
    fallback_project_root: Option<&Path>,
) -> Result<Vec<PathBuf>> {
    let mut roots = Vec::new();
    if let Some(scope) = scope {
        for root in &scope.project_roots {
            roots.push(normalize_for_access(Path::new(root))?);
        }
    }
    if roots.is_empty()
        && let Some(root) = fallback_project_root
    {
        roots.push(normalize_for_access(root)?);
    }
    Ok(roots)
}

fn approval_reason(requests: &[ToolAccessRequestPayload]) -> String {
    let Some(first) = requests.first() else {
        return "Tool call requires approval".to_string();
    };
    let action = match first.kind {
        ToolAccessKind::Read => "read",
        ToolAccessKind::Write => "write",
        ToolAccessKind::Execute => "execute",
        ToolAccessKind::Network => "access network",
    };
    if requests.len() == 1 {
        format!(
            "Tool call wants to {action} outside the project: {}",
            first.target
        )
    } else {
        format!(
            "Tool call wants to {action} {} paths outside the project",
            requests.len()
        )
    }
}

fn normalize_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;
    use ai_chat_core::{ToolApprovalPolicy, ToolSource};
    use tempfile::tempdir;

    #[test]
    fn request_approval_allows_project_write_and_external_read() {
        let project = tempdir().unwrap();
        let external = tempdir().unwrap();
        let evaluator = evaluator(project.path(), ToolApprovalMode::RequestApproval);
        let project_write = path_access_request(
            ToolAccessKind::Write,
            "src/main.rs",
            Some(project.path()),
            None,
        )
        .unwrap();
        let external_read = path_access_request(
            ToolAccessKind::Read,
            external.path().join("notes.txt").to_string_lossy(),
            Some(project.path()),
            None,
        )
        .unwrap();

        assert!(matches!(
            evaluator.evaluate(&[project_write, external_read]),
            ToolPermissionDecision::Allow { .. }
        ));
    }

    #[test]
    fn request_approval_asks_for_external_write() {
        let project = tempdir().unwrap();
        let external = tempdir().unwrap();
        let evaluator = evaluator(project.path(), ToolApprovalMode::RequestApproval);
        let external_write = path_access_request(
            ToolAccessKind::Write,
            external.path().join("notes.txt").to_string_lossy(),
            Some(project.path()),
            None,
        )
        .unwrap();

        let ToolPermissionDecision::Ask {
            access_requests, ..
        } = evaluator.evaluate(&[external_write])
        else {
            panic!("expected approval request");
        };
        assert_eq!(access_requests.len(), 1);
        assert!(!access_requests[0].within_project);
    }

    #[test]
    fn auto_approve_allows_external_write_with_audit_payload() {
        let project = tempdir().unwrap();
        let external = tempdir().unwrap();
        let evaluator = evaluator(project.path(), ToolApprovalMode::AutoApprove);
        let external_write = path_access_request(
            ToolAccessKind::Write,
            external.path().join("notes.txt").to_string_lossy(),
            Some(project.path()),
            None,
        )
        .unwrap();

        let ToolPermissionDecision::Allow { auto_approved } = evaluator.evaluate(&[external_write])
        else {
            panic!("expected auto allow");
        };
        assert_eq!(auto_approved.len(), 1);
    }

    fn evaluator(project_root: &Path, approval_mode: ToolApprovalMode) -> ToolPermissionEvaluator {
        ToolPermissionEvaluator::from_policy(
            &ToolPolicySnapshot {
                approval_policy: ToolApprovalPolicy::OnRequest,
                enabled_sources: vec![ToolSource::Local],
                max_steps: 8,
                approval_mode,
                permission_scope: Some(ToolPermissionScopeSnapshot {
                    project_roots: vec![project_root.to_string_lossy().into_owned()],
                    external_read_requires_approval: false,
                    external_write_requires_approval: true,
                }),
            },
            Some(project_root),
        )
        .unwrap()
    }
}
