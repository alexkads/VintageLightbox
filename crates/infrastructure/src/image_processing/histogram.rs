use image::DynamicImage;

/// Histogram data for RGB channels (Infrastructure Layer)
#[derive(Debug, Clone)]
pub struct HistogramData {
    pub red: [u32; 256],
    pub green: [u32; 256],
    pub blue: [u32; 256],
    pub max_value: u32,
}

impl HistogramData {
    /// Calculate histogram from a DynamicImage
    pub fn from_image(img: &DynamicImage) -> Self {
        let mut data = Self {
            red: [0; 256],
            green: [0; 256],
            blue: [0; 256],
            max_value: 0,
        };
        
        let rgb_img = img.to_rgb8();
        for pixel in rgb_img.pixels() {
            data.red[pixel[0] as usize] += 1;
            data.green[pixel[1] as usize] += 1;
            data.blue[pixel[2] as usize] += 1;
        }
        
        // Find max value for normalization
        data.max_value = data.red.iter()
            .chain(data.green.iter())
            .chain(data.blue.iter())
            .copied()
            .max()
            .unwrap_or(1);
        
        data
    }
}
