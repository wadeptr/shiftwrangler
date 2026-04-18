use async_trait::async_trait;
use shiftwrangler_core::{
    error::{Result, ShiftError},
    platform::{Platform, PlatformMode, Target},
};
use tracing::info;

/// Manages the local machine via systemctl and rtcwake.
pub struct LocalPlatform;

impl LocalPlatform {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LocalPlatform {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Platform for LocalPlatform {
    fn mode(&self) -> PlatformMode {
        PlatformMode::Local
    }

    async fn suspend(&self, _target: &Target) -> Result<()> {
        info!("suspending local machine via systemctl");
        let status = tokio::process::Command::new("systemctl")
            .arg("suspend")
            .status()
            .await
            .map_err(|e| ShiftError::Platform(format!("systemctl suspend failed: {e}")))?;

        if !status.success() {
            return Err(ShiftError::SuspendFailed("systemctl suspend returned non-zero".into()));
        }
        Ok(())
    }

    async fn wake(&self, _target: &Target) -> Result<()> {
        // RTC wake is set before suspend so the machine wakes itself.
        // This is a no-op post-wake; the daemon restores sessions on startup.
        info!("local wake: no action required (RTC alarm pre-set before suspend)");
        Ok(())
    }

    async fn is_alive(&self, _target: &Target) -> Result<bool> {
        Ok(true)
    }
}

/// Set an RTC alarm so the machine wakes at `wake_timestamp` (Unix seconds).
/// Must be called before `suspend()`.
pub async fn set_rtc_alarm(wake_timestamp: i64) -> Result<()> {
    info!(wake_timestamp, "setting RTC wake alarm via rtcwake");
    let status = tokio::process::Command::new("rtcwake")
        .args(["--mode", "no", "--time", &wake_timestamp.to_string()])
        .status()
        .await
        .map_err(|e| ShiftError::Platform(format!("rtcwake failed: {e}")))?;

    if !status.success() {
        return Err(ShiftError::Platform("rtcwake returned non-zero".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use shiftwrangler_core::platform::Target;

    #[tokio::test]
    async fn is_alive_always_true_locally() {
        let p = LocalPlatform::new();
        let target = Target::local();
        assert!(p.is_alive(&target).await.unwrap());
    }

    #[tokio::test]
    async fn wake_is_noop() {
        let p = LocalPlatform::new();
        let target = Target::local();
        assert!(p.wake(&target).await.is_ok());
    }
}
