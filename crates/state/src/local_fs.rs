use async_trait::async_trait;
use shiftwrangler_core::{
    error::{Result, ShiftError},
    manifest::Manifest,
};
use std::path::PathBuf;
use tracing::info;

#[async_trait]
pub trait StateBackend: Send + Sync {
    async fn save_manifest(&self, manifest: &Manifest) -> Result<()>;
    async fn load_manifest(&self) -> Result<Option<Manifest>>;
    async fn clear_manifest(&self) -> Result<()>;
}

/// Persists the session manifest as a JSON file on the local filesystem.
pub struct LocalFsBackend {
    path: PathBuf,
}

impl LocalFsBackend {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn default_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("shiftwrangler")
            .join("manifest.json")
    }
}

#[async_trait]
impl StateBackend for LocalFsBackend {
    async fn save_manifest(&self, manifest: &Manifest) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(ShiftError::Io)?;
        }
        let json = manifest.serialize()?;
        tokio::fs::write(&self.path, json).await.map_err(ShiftError::Io)?;
        info!(path = %self.path.display(), "manifest saved");
        Ok(())
    }

    async fn load_manifest(&self) -> Result<Option<Manifest>> {
        if !self.path.exists() {
            return Ok(None);
        }
        let json = tokio::fs::read_to_string(&self.path)
            .await
            .map_err(ShiftError::Io)?;
        let manifest = Manifest::deserialize(&json)?;
        Ok(Some(manifest))
    }

    async fn clear_manifest(&self) -> Result<()> {
        if self.path.exists() {
            tokio::fs::remove_file(&self.path).await.map_err(ShiftError::Io)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shiftwrangler_core::{
        agent::tests::{make_session, make_state},
        manifest::Manifest,
    };
    use tempfile::tempdir;

    #[tokio::test]
    async fn round_trip_manifest() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("manifest.json");
        let backend = LocalFsBackend::new(&path);

        let session = make_session("claude-code");
        let state = make_state(&session);
        let manifest = Manifest::new(vec![state]);

        backend.save_manifest(&manifest).await.unwrap();
        let loaded = backend.load_manifest().await.unwrap().unwrap();

        assert_eq!(loaded.sessions.len(), 1);
        assert_eq!(loaded.sessions[0].agent_type, "claude-code");
    }

    #[tokio::test]
    async fn load_returns_none_when_no_file() {
        let dir = tempdir().unwrap();
        let backend = LocalFsBackend::new(dir.path().join("nonexistent.json"));
        let result = backend.load_manifest().await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn clear_removes_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("manifest.json");
        let backend = LocalFsBackend::new(&path);

        let manifest = Manifest::new(vec![]);
        backend.save_manifest(&manifest).await.unwrap();
        assert!(path.exists());

        backend.clear_manifest().await.unwrap();
        assert!(!path.exists());
    }
}
