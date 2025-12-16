//! # Domain Layer - VintageLightbox
//!
//! Esta camada contém as regras de negócio centrais da aplicação,
//! independentes de qualquer framework ou tecnologia externa.
//!
//! ## Estrutura
//! - `entities`: Entidades de domínio (Photo, Collection, etc.)
//! - `value_objects`: Objetos de valor imutáveis (Rating, PhotoId, etc.)
//! - `services`: Serviços de domínio
//! - `repositories`: Traits de repositórios (interfaces)
//! - `errors`: Erros específicos do domínio

pub mod entities;
pub mod errors;
pub mod repositories;
pub mod services;
pub mod value_objects;

// Re-exports públicos
pub use errors::{DomainError, DomainResult};
