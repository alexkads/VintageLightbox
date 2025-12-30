use crate::rendering::primitives::{RenderCommand, MeshData};
use egui::{Rect, Color32, TextureId};

/// Trait for rendering operations.
/// Allows decoupling components from specific rendering backends (like egui).
pub trait ImageRenderer {
    /// Execute a render command
    fn render(&mut self, command: RenderCommand);
    
    // Convenience methods
    
    fn draw_rect(&mut self, rect: Rect, color: Color32) {
        self.render(RenderCommand::DrawRect { rect, color });
    }
    
    fn draw_image(&mut self, rect: Rect, texture_id: TextureId, uv: Rect) {
        self.render(RenderCommand::DrawImage { rect, texture_id, uv });
    }
    
    fn draw_mesh(&mut self, mesh: MeshData) {
        self.render(RenderCommand::DrawMesh(mesh));
    }
}
