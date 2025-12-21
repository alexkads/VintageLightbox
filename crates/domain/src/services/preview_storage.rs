use crate::value_objects::PhotoId;
use crate::DomainResult;
use async_trait::async_trait;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PreviewType {
    /// Small thumbnail for grid view (e.g., 300px)
    Thumbnail,
    /// Large preview for develop view (e.g., fit to screen/2560px)
    Large,
}

/// Service for managing photo previews in a cache (e.g., DB BLOBs)
#[async_trait]
pub trait PreviewStorage: Send + Sync {
    /// Save a preview to the storage
    fn save(&self, id: &PhotoId, preview_type: PreviewType, data: &[u8]) -> DomainResult<()>;
    
    /// Retrieve a preview from the storage
    fn get(&self, id: &PhotoId, preview_type: PreviewType) -> DomainResult<Option<Vec<u8>>>;
    
    /// Check if a preview exists
    fn has(&self, id: &PhotoId, preview_type: PreviewType) -> DomainResult<bool>;
    
    /// Delete all previews for a photo
    fn delete(&self, id: &PhotoId) -> DomainResult<()>;
}
