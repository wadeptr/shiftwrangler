use shiftwrangler_core::agent::AgentAdapter;
use std::sync::Arc;
use tracing::warn;

/// Polls all registered sessions and logs any that have died unexpectedly.
pub struct HealthMonitor {
    agents: Vec<Arc<dyn AgentAdapter>>,
}

impl HealthMonitor {
    pub fn new(agents: Vec<Arc<dyn AgentAdapter>>) -> Self {
        Self { agents }
    }

    pub async fn check_all(&self) {
        for adapter in &self.agents {
            let sessions = match adapter.discover().await {
                Ok(s) => s,
                Err(e) => {
                    warn!(agent = adapter.agent_type(), err = %e, "health check discover failed");
                    continue;
                }
            };

            for session in &sessions {
                match adapter.is_alive(session).await {
                    Ok(true) => {}
                    Ok(false) => warn!(id = %session.id, "session appears dead"),
                    Err(e) => warn!(id = %session.id, err = %e, "health check failed"),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shiftwrangler_core::agent::tests::MockAgentAdapter;

    #[tokio::test]
    async fn check_all_handles_empty_adapters() {
        let monitor = HealthMonitor::new(vec![]);
        monitor.check_all().await; // must not panic
    }

    #[tokio::test]
    async fn check_all_handles_discover_error() {
        let mut agent = MockAgentAdapter::new();
        agent.expect_agent_type().return_const("test");
        agent
            .expect_discover()
            .returning(|| Err(shiftwrangler_core::ShiftError::Agent("err".into())));

        let monitor = HealthMonitor::new(vec![Arc::new(agent)]);
        monitor.check_all().await; // must not panic
    }
}
