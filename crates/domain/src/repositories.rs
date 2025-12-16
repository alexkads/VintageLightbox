// Traits de repositórios (interfaces)
// Implementações ficam na camada de Infrastructure

// use crate::{entities::Photo, value_objects::PhotoId, DomainResult};

// /// Trait para repositório de fotos
// pub trait PhotoRepository {
//     fn save(&self, photo: &Photo) -> DomainResult<()>;
//     fn find_by_id(&self, id: PhotoId) -> DomainResult<Option<Photo>>;
//     fn find_all(&self) -> DomainResult<Vec<Photo>>;
//     fn delete(&self, id: PhotoId) -> DomainResult<()>;
// }
