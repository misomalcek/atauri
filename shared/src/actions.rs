use serde::{Deserialize, Serialize};

/// Proposed action that AI can suggest for user approval
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposedAction {
    pub id: String,
    pub kind: ActionKind,
    pub description: String,
    pub details: ActionDetails,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionKind {
    WriteFile,
    CreateFile,
    DeleteFile,
    GitCommit,
    GitPush,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionDetails {
    FileWrite { path: String, content: String },
    FileDelete { path: String },
    GitCommit { message: String },
    GitPush,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub id: String,
    pub success: bool,
    pub message: String,
}

/// Parse AI response for action blocks
///
/// Format:
/// ```action:write_file
/// path: some/file.md
/// ---
/// Content here...
/// ```
pub fn parse_actions(response: &str) -> Vec<ProposedAction> {
    let mut actions = Vec::new();
    let mut i = 0u32;

    // Find ```action: blocks
    let mut pos = 0;
    while let Some(start) = response[pos..].find("```action:") {
        let block_start = pos + start;
        let after_marker = block_start + 10; // len("```action:")

        // Find the end of the first line (action type)
        let line_end = response[after_marker..].find('\n')
            .map(|p| after_marker + p)
            .unwrap_or(response.len());

        let action_type = response[after_marker..line_end].trim();

        // Find closing ```
        let body_start = line_end + 1;
        let block_end = response[body_start..].find("\n```")
            .map(|p| body_start + p)
            .unwrap_or(response.len());

        let body = &response[body_start..block_end];

        if let Some(action) = parse_action_block(action_type, body, i) {
            actions.push(action);
            i += 1;
        }

        pos = block_end + 4; // skip past \n```
        if pos >= response.len() { break; }
    }

    actions
}

fn parse_action_block(action_type: &str, body: &str, index: u32) -> Option<ProposedAction> {
    match action_type {
        "write_file" | "create_file" => {
            // First line: path: <filepath>
            // Separator: ---
            // Rest: content
            let mut lines = body.lines();
            let path_line = lines.next()?;
            let path = path_line.strip_prefix("path:")?.trim().to_string();

            // Skip separator line if present
            let mut content_lines: Vec<&str> = Vec::new();
            let mut past_separator = false;
            for line in lines {
                if !past_separator && line.trim() == "---" {
                    past_separator = true;
                    continue;
                }
                if past_separator || !line.trim().starts_with("---") {
                    past_separator = true;
                    content_lines.push(line);
                }
            }
            let content = content_lines.join("\n");

            let kind = if action_type == "create_file" { ActionKind::CreateFile } else { ActionKind::WriteFile };
            let desc = if action_type == "create_file" {
                format!("Create {path}")
            } else {
                format!("Write to {path}")
            };

            Some(ProposedAction {
                id: format!("action-{index}"),
                kind,
                description: desc,
                details: ActionDetails::FileWrite { path, content },
            })
        }
        "delete_file" => {
            let path = body.lines().next()?.strip_prefix("path:")?.trim().to_string();
            Some(ProposedAction {
                id: format!("action-{index}"),
                kind: ActionKind::DeleteFile,
                description: format!("Delete {path}"),
                details: ActionDetails::FileDelete { path },
            })
        }
        "git_commit" => {
            let message = body.lines().next()?.strip_prefix("message:")?.trim().to_string();
            Some(ProposedAction {
                id: format!("action-{index}"),
                kind: ActionKind::GitCommit,
                description: format!("Commit: {message}"),
                details: ActionDetails::GitCommit { message },
            })
        }
        "git_push" => {
            Some(ProposedAction {
                id: format!("action-{index}"),
                kind: ActionKind::GitPush,
                description: "Push to remote".into(),
                details: ActionDetails::GitPush,
            })
        }
        _ => None,
    }
}

/// Instructions for AI about available actions (appended to system prompt)
pub fn action_instructions() -> &'static str {
    r#"
## Available Actions

You can propose file operations and git actions using special fenced code blocks.
The user will see action cards with Apply/Skip buttons.

### Write/Create a file:
```action:write_file
path: relative/path/to/file.md
---
File content goes here...
```

### Delete a file:
```action:delete_file
path: relative/path/to/file.md
```

### Git commit:
```action:git_commit
message: Your commit message here
```

### Git push:
```action:git_push
```

Use actions when the user asks you to create, modify, or manage files.
Always explain what you're doing before proposing actions.
Propose one action per code block. Multiple actions can be proposed in one response.
"#
}
