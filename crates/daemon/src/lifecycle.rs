use shiftwrangler_core::{
    agent::{AgentAdapter, SessionState},
    error::{Result, ShiftError},
    manifest::Manifest,
    platform::{Platform, Target},
};
use shiftwrangler_state::StateBackend;
use std::sync::Arc;
use tracing::{error, info, warn};

pub struct LifecycleManager {
    pub agents: Vec<Arc<dyn AgentAdapter>>,
    pub platform: Arc<dyn Platform>,
    pub state: Arc<dyn StateBackend>,
    pub target: Target,
}

impl LifecycleManager {
    pub fn new(
        agents: Vec<Arc<dyn AgentAdapter>>,
        platform: Arc<dyn Platform>,
        state: Arc<dyn StateBackend>,
        target: Target,
    ) -> Self {
        Self { agents, platform, state, target }
    }

    /// Pause all sessions, save manifest, then suspend the machine.
    pub async fn suspend(&self) -> Result<()> {
        info!("starting suspend sequence");

        let states = self.pause_all_sessions().await?;
        let manifest = Manifest::new(states);

        self.state.save_manifest(&manifest).await?;
        info!(count = manifest.sessions.len(), "manifest saved");

        self.platform.suspend(&self.target).await?;
        Ok(())
    }

    /// Load manifest and resume all sessions.
    pub async fn resume(&self) -> Result<()> {
        info!("starting resume sequence");

        let manifest = self
            .state
            .load_manifest()
            .await?
            .ok_or_else(|| ShiftError::State("no manifest found on wake".into()))?;

        if manifest.is_empty() {
            info!("manifest is empty; nothing to resume");
            return Ok(());
        }

        self.resume_all_sessions(&manifest.sessions).await?;
        self.state.clear_manifest().await?;
        Ok(())
    }

    async fn pause_all_sessions(&self) -> Result<Vec<SessionState>> {
        let mut all_states = Vec::new();

        for adapter in &self.agents {
            let sessions = adapter.discover().await.unwrap_or_else(|e| {
                warn!(agent = adapter.agent_type(), err = %e, "discover failed");
                vec![]
            });

            for session in &sessions {
                match adapter.pause(session).await {
                    Ok(state) => {
                        info!(id = %session.id, agent = adapter.agent_type(), "session paused");
                        all_states.push(state);
                    }
                    Err(e) => {
                        error!(id = %session.id, err = %e, "failed to pause session");
                    }
                }
            }
        }

        Ok(all_states)
    }

    async fn resume_all_sessions(&self, states: &[SessionState]) -> Result<()> {
        for state in states {
            let adapter = self
                .agents
                .iter()
                .find(|a| a.agent_type() == state.agent_type)
                .ok_or_else(|| {
                    ShiftError::Agent(format!("no adapter for agent type '{}'", state.agent_type))
                })?;

            match adapter.resume(state).await {
                Ok(session) => info!(id = %session.id, agent = state.agent_type, "session resumed"),
                Err(e) => error!(agent = %state.agent_type, err = %e, "failed to resume session"),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shiftwrangler_core::{
        agent::tests::{make_session, make_state, MockAgentAdapter},
        platform::{tests::MockPlatform, Target},
    };
    use shiftwrangler_state::local_fs::LocalFsBackend;
    use tempfile::tempdir;

    fn make_lifecycle(
        agent: MockAgentAdapter,
        platform: MockPlatform,
        dir: &std::path::Path,
    ) -> LifecycleManager {
        LifecycleManager::new(
            vec![Arc::new(agent)],
            Arc::new(platform),
            Arc::new(LocalFsBackend::new(dir.join("manifest.json"))),
            Target::local(),
        )
    }

    #[tokio::test]
    async fn resume_fails_without_manifest() {
        let dir = tempdir().unwrap();
        let agent = MockAgentAdapter::new();
        let platform = MockPlatform::new();
        let lm = make_lifecycle(agent, platform, dir.path());
        let result = lm.resume().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn suspend_saves_manifest() {
        let dir = tempdir().unwrap();
        let session = make_session("test-agent");
        let state = make_state(&session);

        let mut agent = MockAgentAdapter::new();
        agent
            .expect_discover()
            .returning(move || Ok(vec![session.clone()]));
        agent
            .expect_agent_type()
            .return_const("test-agent");
        agent
            .expect_pause()
            .returning(move |s| Ok(make_state(s)));

        let mut platform = MockPlatform::new();
        platform
            .expect_suspend()
            .returning(|_| Ok(()));

        let lm = make_lifecycle(agent, platform, dir.path());
        lm.suspend().await.unwrap();

        let manifest_path = dir.path().join("manifest.json");
        assert!(manifest_path.exists());
        let _ = state;
    }
}
