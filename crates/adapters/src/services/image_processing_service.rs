use image::DynamicImage;
use domain::value_objects::{PhotoEdits, CropSettings};

#[cfg_attr(test, mockall::automock)]
pub trait ImageProcessingService {
    /// Process an image with the given edits
    fn process_image(&self, img: &DynamicImage, edits: &PhotoEdits) -> DynamicImage;

    /// Apply crop to an image
    fn apply_crop(&self, img: &DynamicImage, crop: &CropSettings) -> DynamicImage;
}
