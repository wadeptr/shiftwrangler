use chrono::{NaiveTime, Timelike};
use shiftwrangler_core::{
    error::Result,
    schedule::{ScheduleConfig, SuspendTrigger},
};
use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::info;

use crate::lifecycle::LifecycleManager;

pub struct Scheduler {
    inner: JobScheduler,
}

impl Scheduler {
    pub async fn new() -> Result<Self> {
        let inner = JobScheduler::new()
            .await
            .map_err(|e| anyhow::anyhow!("scheduler init failed: {e}"))?;
        Ok(Self { inner })
    }

    pub async fn register_config(
        &mut self,
        config: &ScheduleConfig,
        lifecycle: Arc<LifecycleManager>,
    ) -> Result<()> {
        for trigger in &config.triggers {
            match trigger {
                SuspendTrigger::Schedule(daily) => {
                    let suspend_cron = time_to_cron(&daily.suspend_at);
                    let wake_cron = time_to_cron(&daily.wake_at);
                    let lc_suspend = lifecycle.clone();
                    let lc_wake = lifecycle.clone();

                    info!(%suspend_cron, "registering suspend job");
                    self.inner
                        .add(Job::new_async(suspend_cron.as_str(), move |_, _| {
                            let lc = lc_suspend.clone();
                            Box::pin(async move {
                                if let Err(e) = lc.suspend().await {
                                    tracing::error!(err = %e, "scheduled suspend failed");
                                }
                            })
                        })
                        .map_err(|e| anyhow::anyhow!("job create failed: {e}"))?)
                        .await
                        .map_err(|e| anyhow::anyhow!("job add failed: {e}"))?;

                    info!(%wake_cron, "registering resume job");
                    self.inner
                        .add(Job::new_async(wake_cron.as_str(), move |_, _| {
                            let lc = lc_wake.clone();
                            Box::pin(async move {
                                if let Err(e) = lc.resume().await {
                                    tracing::error!(err = %e, "scheduled resume failed");
                                }
                            })
                        })
                        .map_err(|e| anyhow::anyhow!("job create failed: {e}"))?)
                        .await
                        .map_err(|e| anyhow::anyhow!("job add failed: {e}"))?;
                }
                SuspendTrigger::Manual => {}
                SuspendTrigger::Thermal { .. } => {
                    // Thermal monitoring is handled by the health module.
                }
            }
        }
        Ok(())
    }

    pub async fn start(&self) -> Result<()> {
        self.inner
            .start()
            .await
            .map_err(|e| anyhow::anyhow!("scheduler start failed: {e}").into())
    }
}

/// Convert a NaiveTime to a 6-field cron expression (sec min hour * * *).
fn time_to_cron(t: &NaiveTime) -> String {
    format!("0 {} {} * * *", t.minute(), t.hour())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;

    #[test]
    fn cron_expression_format() {
        let t = NaiveTime::from_hms_opt(23, 30, 0).unwrap();
        assert_eq!(time_to_cron(&t), "0 30 23 * * *");
    }

    #[test]
    fn cron_midnight() {
        let t = NaiveTime::from_hms_opt(0, 0, 0).unwrap();
        assert_eq!(time_to_cron(&t), "0 0 0 * * *");
    }
}
