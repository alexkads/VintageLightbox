use egui::{Color32, Rect, TextureHandle, TextureId, Pos2};

/// Abstraction for a texture identifier.
/// Currently wraps egui data, but can be abstracted further.
#[derive(Clone)]
pub enum TextureSource<'a> {
    Handle(&'a TextureHandle),
    Id(TextureId),
}

impl<'a> std::fmt::Debug for TextureSource<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Handle(_) => write!(f, "TextureSource::Handle(...)"),
            Self::Id(id) => write!(f, "TextureSource::Id({:?})", id),
        }
    }
}


impl<'a> From<&'a TextureHandle> for TextureSource<'a> {
    fn from(handle: &'a TextureHandle) -> Self {
        Self::Handle(handle)
    }
}

/// Generic vertex for mesh rendering
#[derive(Clone, Copy, Debug)]
pub struct Vertex {
    pub pos: Pos2,
    pub uv: Pos2,
    pub color: Color32,
}

/// Mesh data for rendering
#[derive(Clone, Debug, Default)]
pub struct MeshData {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub texture_id: TextureId,
}

/// Common rendering operations
pub enum RenderCommand {
    DrawRect { rect: Rect, color: Color32 },
    DrawImage { rect: Rect, texture_id: TextureId, uv: Rect },
    DrawMesh(MeshData),
}
