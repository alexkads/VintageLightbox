use async_trait::async_trait;
use domain::ports::system_gateway::SystemGateway;

pub struct SystemGatewayImpl;

impl SystemGatewayImpl {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SystemGatewayImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SystemGateway for SystemGatewayImpl {
    async fn open_path(&self, path: &str) -> Result<(), String> {
        opener::open(path).map_err(|e| e.to_string())
    }
}
