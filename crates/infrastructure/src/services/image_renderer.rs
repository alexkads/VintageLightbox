use image::DynamicImage;
use crate::services::texture_service::TextureRenderer;

/// Serviço de alto nível para renderização de imagens
pub struct ImageRenderer<R: TextureRenderer> {
    renderer: R,
}

impl<R: TextureRenderer> ImageRenderer<R> {
    pub fn new(renderer: R) -> Self {
        Self { renderer }
    }

    pub fn create_preview(&self, photo_id: &str, image: &DynamicImage) -> Result<R::Texture, R::Error> {
        let name = format!("preview_{}", photo_id);
        self.renderer.create_texture(&name, image)
    }

    pub fn create_thumbnail(&self, photo_id: &str, image: &DynamicImage) -> Result<R::Texture, R::Error> {
        let name = format!("thumb_{}", photo_id);
        self.renderer.create_texture(&name, image)
    }
}
