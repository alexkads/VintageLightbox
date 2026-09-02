use thiserror::Error;

/// Erros que podem ocorrer na camada de domínio
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("Rating inválido: deve estar entre 0 e 5")]
    InvalidRating,

    #[error("ID inválido: {id} - {reason}")]
    InvalidId { id: String, reason: String },

    #[error("Photo ID inválido")]
    InvalidPhotoId,

    #[error("File path inválido: {0}")]
    InvalidFilePath(String),

    #[error("Color label inválido")]
    InvalidColorLabel,

    #[error("Adjustment value fora dos limites permitidos")]
    InvalidAdjustmentValue,

    #[error("Print settings inválido: {0}")]
    InvalidPrintSettings(String),

    #[error("Foto não encontrada")]
    PhotoNotFound,

    /// O site recusou a credencial — e-mail, senha ou sessão vencida.
    ///
    /// Variante própria, e não `InfrastructureError`, porque o remédio é outro:
    /// "sem rede" é esperar; isto é entrar de novo. A tela precisa distinguir.
    #[error("o site recusou o acesso: confira e-mail e senha")]
    AcessoRecusado,

    #[error("Collection não encontrada")]
    CollectionNotFound,

    #[error("Operação inválida: {0}")]
    InvalidOperation(String),

    #[error("Erro de infraestrutura: {0}")]
    InfrastructureError(String),
}

/// Tipo Result padrão para operações de domínio
pub type DomainResult<T> = Result<T, DomainError>;
