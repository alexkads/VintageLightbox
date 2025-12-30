use crate::rendering::traits::ImageRenderer;
use crate::rendering::primitives::{RenderCommand};
use egui::{Ui, Shape, Mesh, Color32};

/// Implementation of ImageRenderer for egui::Ui
pub struct EguiRenderer<'a> {
    pub ui: &'a mut Ui,
}

impl<'a> EguiRenderer<'a> {
    pub fn new(ui: &'a mut Ui) -> Self {
        Self { ui }
    }
}

impl<'a> ImageRenderer for EguiRenderer<'a> {
    fn render(&mut self, command: RenderCommand) {
        match command {
            RenderCommand::DrawRect { rect, color } => {
                self.ui.painter().rect_filled(rect, 0.0, color);
            }
            RenderCommand::DrawImage { rect, texture_id, uv } => {
                let mut mesh = Mesh::with_texture(texture_id);
                mesh.add_rect_with_uv(rect, uv, Color32::WHITE);
                self.ui.painter().add(Shape::mesh(mesh));
            }
            RenderCommand::DrawMesh(mesh_data) => {
                let mut mesh = Mesh::with_texture(mesh_data.texture_id);
                for v in mesh_data.vertices {
                    mesh.vertices.push(egui::epaint::Vertex {
                        pos: v.pos,
                        uv: v.uv,
                        color: v.color,
                    });
                }
                mesh.indices = mesh_data.indices;
                self.ui.painter().add(Shape::mesh(mesh));
            }
        }
    }
}
