//! GitHub Tool — interact with GitHub API: list PRs, issues, check CI status.
//!
//! Uses the GitHub REST API v3. Requires `GITHUB_TOKEN` environment variable
//! for authentication (or works with reduced rate limits without it).
//!
//! # Actions
//!
//! - `list_prs`     — List open pull requests for a repository
//! - `list_issues`  — List open issues for a repository
//! - `check_ci`     — Check latest CI workflow run status
//! - `repo_info`    — Get repository metadata and stats

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::{json, Value};
use std::collections::HashMap;

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(GitHubTool));
}

struct GitHubTool;

#[async_trait::async_trait]
impl Tool for GitHubTool {
    fn name(&self) -> &str {
        "github_tool"
    }

    fn description(&self) -> &str {
        "Interact with GitHub API: list PRs, issues, check CI status, get repo info. \
         Requires GITHUB_TOKEN env var for authenticated requests."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                parameter_type: "string".to_string(),
                description: "Action: list_prs, list_issues, check_ci, repo_info".to_string(),
                required: true,
            },
            ToolParameter {
                name: "owner".to_string(),
                parameter_type: "string".to_string(),
                description: "Repository owner (e.g. 'rust-lang')".to_string(),
                required: true,
            },
            ToolParameter {
                name: "repo".to_string(),
                parameter_type: "string".to_string(),
                description: "Repository name (e.g. 'rust')".to_string(),
                required: true,
            },
            ToolParameter {
                name: "state".to_string(),
                parameter_type: "string".to_string(),
                description: "Filter by state: open, closed, all (default: open)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "limit".to_string(),
                parameter_type: "number".to_string(),
                description: "Maximum results to return (default: 10, max: 100)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "branch".to_string(),
                parameter_type: "string".to_string(),
                description: "Filter PRs by head branch (for list_prs)".to_string(),
                required: false,
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: action")?;

        let owner = params
            .get("owner")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: owner")?;

        let repo = params
            .get("repo")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: repo")?;

        let state = params
            .get("state")
            .and_then(|v| v.as_str())
            .unwrap_or("open");

        let limit = params
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(10)
            .min(100) as usize;

        match action {
            "list_prs" => list_prs(owner, repo, state, limit, params).await,
            "list_issues" => list_issues(owner, repo, state, limit).await,
            "check_ci" => check_ci(owner, repo, limit).await,
            "repo_info" => repo_info(owner, repo).await,
            _ => Err(format!(
                "Unknown action: {action}. Supported: list_prs, list_issues, check_ci, repo_info"
            )),
        }
    }
}

// ============================================================================
// GitHub API Client
// ============================================================================

/// Build headers with optional auth token.
fn build_headers() -> Vec<(&'static str, String)> {
    let mut headers = vec![
        ("Accept", "application/vnd.github.v3+json".to_string()),
        ("User-Agent", "cargo-agent/0.1.0".to_string()),
    ];
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        if !token.is_empty() {
            headers.push(("Authorization", format!("Bearer {token}")));
        }
    }
    headers
}

/// Make a GitHub API GET request.
async fn github_get(path: &str) -> Result<Value, String> {
    let url = format!("https://api.github.com{path}");
    let resp = reqwest::Client::new()
        .get(&url)
        .headers(build_headers().into_iter().map(|(k, v)| {
            (reqwest::header::HeaderName::from_bytes(k.as_bytes()).unwrap(), reqwest::header::HeaderValue::from_str(&v).unwrap())
        }).collect::<reqwest::header::HeaderMap>())
        .send()
        .await
        .map_err(|e| format!("GitHub API request failed: {e}"))?;

    let status = resp.status();
    let body: Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse GitHub response: {e}"))?;

    if !status.is_success() {
        let msg = body
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error");
        return Err(format!("GitHub API error (HTTP {status}): {msg}"));
    }

    Ok(body)
}

// ============================================================================
// Actions
// ============================================================================

/// List pull requests for a repository.
async fn list_prs(
    owner: &str,
    repo: &str,
    state: &str,
    limit: usize,
    params: &HashMap<String, Value>,
) -> Result<Value, String> {
    let mut path = format!("/repos/{owner}/{repo}/pulls?state={state}&per_page={limit}&sort=updated&direction=desc");

    if let Some(branch) = params.get("branch").and_then(|v| v.as_str()) {
        path.push_str(&format!("&head={owner}:{branch}"));
    }

    let body = github_get(&path).await?;

    let prs = match &body {
        Value::Array(arr) => arr,
        _ => return Err("Unexpected response format from GitHub API".to_string()),
    };

    let results: Vec<Value> = prs
        .iter()
        .take(limit)
        .map(|pr| {
            json!({
                "number": pr.get("number"),
                "title": pr.get("title").and_then(|v| v.as_str()).unwrap_or(""),
                "state": pr.get("state").and_then(|v| v.as_str()).unwrap_or(""),
                "user": pr.get("user").and_then(|u| u.get("login")).and_then(|v| v.as_str()).unwrap_or(""),
                "created_at": pr.get("created_at").and_then(|v| v.as_str()).unwrap_or(""),
                "updated_at": pr.get("updated_at").and_then(|v| v.as_str()).unwrap_or(""),
                "draft": pr.get("draft").and_then(|v| v.as_bool()).unwrap_or(false),
                "html_url": pr.get("html_url").and_then(|v| v.as_str()).unwrap_or(""),
                "labels": pr.get("labels").map(|labels| {
                    labels.as_array().map(|arr| {
                        arr.iter().filter_map(|l| l.get("name").and_then(|v| v.as_str()).map(|s| s.to_string())).collect::<Vec<_>>()
                    }).unwrap_or_default()
                }).unwrap_or_default(),
            })
        })
        .collect();

    Ok(json!({
        "action": "list_prs",
        "owner": owner,
        "repo": repo,
        "state": state,
        "count": results.len(),
        "pull_requests": results,
    }))
}

/// List issues for a repository.
async fn list_issues(owner: &str, repo: &str, state: &str, limit: usize) -> Result<Value, String> {
    let path = format!("/repos/{owner}/{repo}/issues?state={state}&per_page={limit}&sort=updated&direction=desc");
    let body = github_get(&path).await?;

    let issues = match &body {
        Value::Array(arr) => arr,
        _ => return Err("Unexpected response format from GitHub API".to_string()),
    };

    // Filter out PRs (GitHub's /issues endpoint also returns PRs)
    let results: Vec<Value> = issues
        .iter()
        .filter(|item| !item.get("pull_request").is_some())
        .take(limit)
        .map(|issue| {
            json!({
                "number": issue.get("number"),
                "title": issue.get("title").and_then(|v| v.as_str()).unwrap_or(""),
                "state": issue.get("state").and_then(|v| v.as_str()).unwrap_or(""),
                "user": issue.get("user").and_then(|u| u.get("login")).and_then(|v| v.as_str()).unwrap_or(""),
                "created_at": issue.get("created_at").and_then(|v| v.as_str()).unwrap_or(""),
                "updated_at": issue.get("updated_at").and_then(|v| v.as_str()).unwrap_or(""),
                "comments": issue.get("comments").and_then(|v| v.as_u64()).unwrap_or(0),
                "html_url": issue.get("html_url").and_then(|v| v.as_str()).unwrap_or(""),
                "labels": issue.get("labels").map(|labels| {
                    labels.as_array().map(|arr| {
                        arr.iter().filter_map(|l| l.get("name").and_then(|v| v.as_str()).map(|s| s.to_string())).collect::<Vec<_>>()
                    }).unwrap_or_default()
                }).unwrap_or_default(),
            })
        })
        .collect();

    Ok(json!({
        "action": "list_issues",
        "owner": owner,
        "repo": repo,
        "state": state,
        "count": results.len(),
        "issues": results,
    }))
}

/// Check latest CI workflow run status.
async fn check_ci(owner: &str, repo: &str, limit: usize) -> Result<Value, String> {
    let path = format!("/repos/{owner}/{repo}/actions/runs?per_page={limit}&status=completed&page=1");
    let body = github_get(&path).await?;

    let workflow_runs = body
        .get("workflow_runs")
        .and_then(|v| v.as_array())
        .ok_or("Unexpected response format from GitHub Actions API")?;

    let total = body
        .get("total_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let runs: Vec<Value> = workflow_runs
        .iter()
        .take(limit)
        .map(|run| {
            json!({
                "id": run.get("id"),
                "name": run.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                "head_branch": run.get("head_branch").and_then(|v| v.as_str()).unwrap_or(""),
                "status": run.get("status").and_then(|v| v.as_str()).unwrap_or(""),
                "conclusion": run.get("conclusion").and_then(|v| v.as_str()).unwrap_or(""),
                "created_at": run.get("created_at").and_then(|v| v.as_str()).unwrap_or(""),
                "updated_at": run.get("updated_at").and_then(|v| v.as_str()).unwrap_or(""),
                "html_url": run.get("html_url").and_then(|v| v.as_str()).unwrap_or(""),
                "event": run.get("event").and_then(|v| v.as_str()).unwrap_or(""),
            })
        })
        .collect();

    // Summary stats
    let success_count = runs.iter().filter(|r| r["conclusion"] == "success").count();
    let failure_count = runs.iter().filter(|r| r["conclusion"] == "failure").count();
    let cancelled_count = runs.iter().filter(|r| r["conclusion"] == "cancelled").count();

    Ok(json!({
        "action": "check_ci",
        "owner": owner,
        "repo": repo,
        "total_runs": total,
        "returned": runs.len(),
        "summary": {
            "success": success_count,
            "failure": failure_count,
            "cancelled": cancelled_count,
        },
        "workflow_runs": runs,
    }))
}

/// Get repository information.
async fn repo_info(owner: &str, repo: &str) -> Result<Value, String> {
    let path = format!("/repos/{owner}/{repo}");
    let body = github_get(&path).await?;

    Ok(json!({
        "action": "repo_info",
        "owner": owner,
        "repo": repo,
        "full_name": body.get("full_name").and_then(|v| v.as_str()).unwrap_or(""),
        "description": body.get("description").and_then(|v| v.as_str()).unwrap_or(""),
        "language": body.get("language").and_then(|v| v.as_str()).unwrap_or(""),
        "stars": body.get("stargazers_count").and_then(|v| v.as_u64()).unwrap_or(0),
        "forks": body.get("forks_count").and_then(|v| v.as_u64()).unwrap_or(0),
        "open_issues": body.get("open_issues_count").and_then(|v| v.as_u64()).unwrap_or(0),
        "watchers": body.get("subscribers_count").and_then(|v| v.as_u64()).unwrap_or(0),
        "default_branch": body.get("default_branch").and_then(|v| v.as_str()).unwrap_or(""),
        "license": body.get("license").and_then(|l| l.get("spdx_id")).and_then(|v| v.as_str()).unwrap_or(""),
        "created_at": body.get("created_at").and_then(|v| v.as_str()).unwrap_or(""),
        "updated_at": body.get("updated_at").and_then(|v| v.as_str()).unwrap_or(""),
        "topics": body.get("topics").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|t| t.as_str().map(|s| s.to_string())).collect::<Vec<_>>()).unwrap_or_default(),
        "html_url": body.get("html_url").and_then(|v| v.as_str()).unwrap_or(""),
    }))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_tool_metadata() {
        let tool = GitHubTool;
        assert_eq!(tool.name(), "github_tool");
        assert!(tool.description().contains("GitHub"));
        let params = tool.parameters();
        assert!(params.iter().any(|p| p.name == "action" && p.required));
        assert!(params.iter().any(|p| p.name == "owner" && p.required));
        assert!(params.iter().any(|p| p.name == "repo" && p.required));
    }

    #[test]
    fn test_build_headers_without_token() {
        // Temporarily remove GITHUB_TOKEN for this test
        std::env::remove_var("GITHUB_TOKEN");
        let headers = build_headers();
        assert!(headers.iter().any(|(k, _)| *k == "Accept"));
        assert!(headers.iter().any(|(k, _)| *k == "User-Agent"));
        // No auth header without token
        assert!(!headers.iter().any(|(k, _)| *k == "Authorization"));
    }

    #[tokio::test]
    async fn test_missing_params() {
        let tool = GitHubTool;
        let params = HashMap::new();
        let result = tool.execute(&params).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing required parameter"));
    }

    #[tokio::test]
    async fn test_missing_owner() {
        let mut params = HashMap::new();
        params.insert("action".to_string(), json!("list_prs"));
        params.insert("repo".to_string(), json!("rust"));
        let result = GitHubTool.execute(&params).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing required parameter: owner"));
    }

    #[tokio::test]
    async fn test_invalid_action() {
        let mut params = HashMap::new();
        params.insert("action".to_string(), json!("invalid_action"));
        params.insert("owner".to_string(), json!("test"));
        params.insert("repo".to_string(), json!("test"));
        let result = GitHubTool.execute(&params).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown action"));
    }

    #[tokio::test]
    async fn test_repo_info_bad_repo() {
        let mut params = HashMap::new();
        params.insert("action".to_string(), json!("repo_info"));
        params.insert("owner".to_string(), json!("this-repo-does-not-exist-12345"));
        params.insert("repo".to_string(), json!("also-fake-99999"));
        let result = GitHubTool.execute(&params).await;
        // Should fail with API error, not a panic or crash
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("GitHub API error") || err.contains("request failed"));
    }

    #[test]
    fn test_limit_clamping() {
        let tool = GitHubTool;
        let params = tool.parameters();
        let limit_param = params.iter().find(|p| p.name == "limit").unwrap();
        assert_eq!(limit_param.parameter_type, "number");
    }
}
