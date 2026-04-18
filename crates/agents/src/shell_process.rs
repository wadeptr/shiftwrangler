use async_trait::async_trait;
use chrono::Utc;
use shiftwrangler_core::{
    agent::{AgentAdapter, Session, SessionId, SessionState, SessionStatus},
    error::{Result, ShiftError},
};
use std::collections::HashMap;
use tracing::info;

/// Generic adapter for any shell process. Pauses via SIGTERM and records
/// the command line so it can be re-launched on resume.
pub struct ShellProcessAdapter {
    pub label_prefix: String,
}

impl ShellProcessAdapter {
    pub fn new() -> Self {
        Self {
            label_prefix: "shell".to_string(),
        }
    }
}

impl Default for ShellProcessAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AgentAdapter for ShellProcessAdapter {
    fn agent_type(&self) -> &'static str {
        "shell-process"
    }

    async fn discover(&self) -> Result<Vec<Session>> {
        // In a full implementation this would scan /proc or use sysinfo crate.
        // Returning empty for now — sessions are registered explicitly via CLI.
        Ok(vec![])
    }

    async fn pause(&self, session: &Session) -> Result<SessionState> {
        let pid = session
            .pid
            .ok_or_else(|| ShiftError::Agent("session has no pid".into()))?;

        info!(pid, "sending SIGTERM to shell process");

        #[cfg(unix)]
        {
            use nix::sys::signal::{kill, Signal};
            use nix::unistd::Pid;
            kill(Pid::from_raw(pid as i32), Signal::SIGTERM)
                .map_err(|e| ShiftError::Agent(format!("kill failed: {e}")))?;
        }

        #[cfg(not(unix))]
        warn!("SIGTERM not supported on this platform; session may not stop cleanly");

        let resume_cmd = session
            .label
            .clone()
            .unwrap_or_else(|| format!("# no resume command recorded for session {}", session.id));

        Ok(SessionState {
            session_id: session.id.clone(),
            agent_type: self.agent_type().to_string(),
            working_dir: session.working_dir.clone(),
            metadata: HashMap::new(),
            paused_at: Utc::now(),
            label: session.label.clone(),
            resume_command: resume_cmd,
        })
    }

    async fn resume(&self, state: &SessionState) -> Result<Session> {
        info!(cmd = %state.resume_command, "resuming shell process");

        // Spawn detached. A full implementation would track the new PID.
        let _child = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&state.resume_command)
            .current_dir(&state.working_dir)
            .spawn()
            .map_err(|e| ShiftError::Agent(format!("spawn failed: {e}")))?;

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
        let Some(pid) = session.pid else {
            return Ok(false);
        };
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
    use shiftwrangler_core::agent::tests::make_session;

    #[tokio::test]
    async fn discover_returns_empty_by_default() {
        let adapter = ShellProcessAdapter::new();
        let sessions = adapter.discover().await.unwrap();
        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn pause_fails_without_pid() {
        let adapter = ShellProcessAdapter::new();
        let mut session = make_session("shell-process");
        session.pid = None;
        let result = adapter.pause(&session).await;
        assert!(result.is_err());
    }
}
