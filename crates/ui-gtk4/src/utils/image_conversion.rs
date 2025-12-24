//! Image Conversion Utilities
//!
//! Functions for converting between different image formats (DynamicImage, Pixbuf, Texture).

use gdk_pixbuf::Pixbuf;
use image::DynamicImage;

/// Convert image bytes to gdk_pixbuf::Pixbuf
///
/// Loads the image from memory, resizes it to fit within max_width x max_height,
/// and converts to Pixbuf format for GTK rendering.
pub fn bytes_to_pixbuf(
    image_bytes: &[u8],
    max_width: u32,
    max_height: u32,
) -> Result<Pixbuf, String> {
    // 1. Load image from memory
    let img = image::load_from_memory(image_bytes)
        .map_err(|e| format!("Failed to load image: {}", e))?;

    // 2. Resize to fit within max dimensions (maintaining aspect ratio)
    let img = img.resize(
        max_width,
        max_height,
        image::imageops::FilterType::Lanczos3,
    );

    // 3. Convert to Pixbuf
    dynamic_image_to_pixbuf(&img)
}

/// Convert DynamicImage to Pixbuf
pub fn dynamic_image_to_pixbuf(img: &DynamicImage) -> Result<Pixbuf, String> {
    // Convert to RGB8
    let rgb = img.to_rgb8();
    let (width, height) = rgb.dimensions();

    // Create Pixbuf from raw RGB data
    // Safety: We're creating a new Vec that owns the data
    let data = rgb.into_raw();

    Ok(Pixbuf::from_mut_slice(
        data,
        gdk_pixbuf::Colorspace::Rgb,
        false, // has_alpha
        8,     // bits_per_sample
        width as i32,
        height as i32,
        (width * 3) as i32, // rowstride (RGB = 3 bytes per pixel)
    ))
}

/// Load image from file path
pub fn load_image_from_file(path: &str) -> Result<DynamicImage, String> {
    image::open(path).map_err(|e| format!("Failed to open {}: {}", path, e))
}

/// Convert Pixbuf to gdk::Texture
///
/// This is useful for displaying images in GTK4 widgets like GtkPicture.
pub fn pixbuf_to_texture(pixbuf: &Pixbuf) -> gdk4::Texture {
    gdk4::Texture::for_pixbuf(pixbuf)
}
