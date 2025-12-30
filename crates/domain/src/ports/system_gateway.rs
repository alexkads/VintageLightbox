use async_trait::async_trait;

#[async_trait]
pub trait SystemGateway: Send + Sync {
    /// Open a path in the system's default file manager or application
    async fn open_path(&self, path: &str) -> Result<(), String>;
}
