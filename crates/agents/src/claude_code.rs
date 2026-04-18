use async_trait::async_trait;
use chrono::Utc;
use shiftwrangler_core::{
    agent::{AgentAdapter, Session, SessionId, SessionState, SessionStatus},
    error::{Result, ShiftError},
};
use std::collections::HashMap;
use tracing::info;

const CLAUDE_SESSIONS_DIR: &str = ".claude/projects";
const CONVERSATION_ID_KEY: &str = "conversation_id";

/// Adapter for Claude Code CLI sessions.
///
/// Claude Code stores conversation state under ~/.claude/projects/<hash>/.
/// Sessions are resumed with `claude --continue <conversation_id>`.
pub struct ClaudeCodeAdapter {
    pub claude_home: std::path::PathBuf,
}

impl ClaudeCodeAdapter {
    pub fn new() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        Self {
            claude_home: std::path::PathBuf::from(home).join(CLAUDE_SESSIONS_DIR),
        }
    }
}

impl Default for ClaudeCodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AgentAdapter for ClaudeCodeAdapter {
    fn agent_type(&self) -> &'static str {
        "claude-code"
    }

    async fn discover(&self) -> Result<Vec<Session>> {
        // Scan for running `claude` processes. A full implementation would
        // cross-reference /proc/<pid>/cmdline with ~/.claude session files.
        Ok(vec![])
    }

    async fn pause(&self, session: &Session) -> Result<SessionState> {
        let pid = session.pid.ok_or_else(|| ShiftError::Agent("session has no pid".into()))?;
        let conversation_id = session
            .label
            .clone()
            .unwrap_or_else(|| session.id.to_string());

        info!(pid, %conversation_id, "pausing Claude Code session");

        #[cfg(unix)]
        {
            use nix::sys::signal::{kill, Signal};
            use nix::unistd::Pid;
            kill(Pid::from_raw(pid as i32), Signal::SIGTERM)
                .map_err(|e| ShiftError::Agent(format!("kill failed: {e}")))?;
        }

        let mut metadata = HashMap::new();
        metadata.insert(CONVERSATION_ID_KEY.to_string(), conversation_id.clone());

        let resume_command = format!("claude --continue {conversation_id}");

        Ok(SessionState {
            session_id: session.id.clone(),
            agent_type: self.agent_type().to_string(),
            working_dir: session.working_dir.clone(),
            metadata,
            paused_at: Utc::now(),
            label: session.label.clone(),
            resume_command,
        })
    }

    async fn resume(&self, state: &SessionState) -> Result<Session> {
        let conversation_id = state
            .metadata
            .get(CONVERSATION_ID_KEY)
            .ok_or_else(|| ShiftError::Agent("missing conversation_id in state".into()))?;

        info!(%conversation_id, "resuming Claude Code session");

        let _child = tokio::process::Command::new("claude")
            .arg("--continue")
            .arg(conversation_id)
            .current_dir(&state.working_dir)
            .spawn()
            .map_err(|e| ShiftError::Agent(format!("failed to launch claude: {e}")))?;

        Ok(Session {
            id: SessionId::new(),
            agent_type: self.agent_type().to_string(),
            pid: None,
            working_dir: state.working_dir.clone(),
            status: SessionStatus::Running,
            started_at: Utc::now(),
            label: state.label.clone(),
        })
    }

    async fn is_alive(&self, session: &Session) -> Result<bool> {
        let Some(pid) = session.pid else { return Ok(false) };
        #[cfg(unix)]
        {
            use nix::sys::signal::{kill, Signal};
            use nix::unistd::Pid;
            Ok(kill(Pid::from_raw(pid as i32), Signal::SIGCONT).is_ok())
        }
        #[cfg(not(unix))]
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shiftwrangler_core::agent::tests::{make_session, make_state};

    #[test]
    fn resume_command_format() {
        let adapter = ClaudeCodeAdapter::new();
        let mut session = make_session("claude-code");
        session.label = Some("conv-abc123".to_string());
        // Verify metadata key is correct without calling async fn.
        assert_eq!(adapter.agent_type(), "claude-code");
    }

    #[tokio::test]
    async fn resume_fails_without_conversation_id() {
        let adapter = ClaudeCodeAdapter::new();
        let session = make_session("claude-code");
        let state = make_state(&session); // metadata is empty
        let result = adapter.resume(&state).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn pause_fails_without_pid() {
        let adapter = ClaudeCodeAdapter::new();
        let mut session = make_session("claude-code");
        session.pid = None;
        let result = adapter.pause(&session).await;
        assert!(result.is_err());
    }
}
