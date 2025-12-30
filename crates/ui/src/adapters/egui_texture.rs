use image::DynamicImage;
use infrastructure::services::texture_service::{TextureHandle, TextureRenderer};
use eframe::egui::{ColorImage, Context, TextureOptions, TextureHandle as EguiTextureHandle};

/// Wrapper para texture handle do egui
pub struct EguiTextureWrapper {
    handle: EguiTextureHandle,
}

impl std::fmt::Debug for EguiTextureWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EguiTextureWrapper")
            .field("name", &self.handle.name())
            .field("size", &self.handle.size())
            .finish()
    }
}

impl TextureHandle for EguiTextureWrapper {
    fn id(&self) -> String {
        self.handle.name().to_string()
    }

    fn width(&self) -> u32 {
        self.handle.size()[0] as u32
    }

    fn height(&self) -> u32 {
        self.handle.size()[1] as u32
    }
}

/// Implementação do renderizador para egui
pub struct EguiTextureRenderer {
    ctx: Context,
}

impl EguiTextureRenderer {
    pub fn new(ctx: Context) -> Self {
        Self { ctx }
    }
}

#[derive(Debug)]
pub enum EguiRenderError {
    AllocationFailed,
}

impl TextureRenderer for EguiTextureRenderer {
    type Texture = EguiTextureWrapper;
    type Error = EguiRenderError;

    fn create_texture(&self, name: &str, image: &DynamicImage) -> Result<Self::Texture, Self::Error> {
        let size = [image.width() as usize, image.height() as usize];
        let color_image = ColorImage::from_rgba_unmultiplied(size, image.as_bytes());
        
        let handle = self.ctx.load_texture(
            name,
            color_image,
            TextureOptions::default()
        );

        Ok(EguiTextureWrapper { handle })
    }

    fn update_texture(&self, texture: &mut Self::Texture, image: &DynamicImage) -> Result<(), Self::Error> {
         let size = [image.width() as usize, image.height() as usize];
         let color_image = ColorImage::from_rgba_unmultiplied(size, image.as_bytes());
         texture.handle.set(color_image, TextureOptions::default());
         Ok(())
    }
}
