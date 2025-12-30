use image::DynamicImage;
use std::fmt::Debug;

/// Abstração para um handle de textura (opaco para a infraestrutura)
pub trait TextureHandle: Send + Sync + Debug {
    fn id(&self) -> String;
    fn width(&self) -> u32;
    fn height(&self) -> u32;
}

/// Trait para renderização de texturas (implementado por cada framework)
pub trait TextureRenderer: Send + Sync {
    type Texture: TextureHandle;
    type Error: Debug;
    
    /// Cria uma nova textura a partir de uma imagem
    fn create_texture(&self, name: &str, image: &DynamicImage) -> Result<Self::Texture, Self::Error>;
    
    /// Atualiza uma textura existente
    fn update_texture(&self, texture: &mut Self::Texture, image: &DynamicImage) -> Result<(), Self::Error>;
}
