/// Represents crop and rotation settings for a photo.
/// All crop coordinates are normalized (0.0 to 1.0) relative to the original image dimensions.
#[derive(Debug, Clone, PartialEq)]
pub struct CropSettings {
    crop_x: f32,
    crop_y: f32,
    crop_width: f32,
    crop_height: f32,
    rotation_90: i32,
    angle: f32,
    flip_horizontal: bool,
    flip_vertical: bool,
}

impl CropSettings {
    const MIN_CROP_SIZE: f32 = 0.01; // Minimum 1% of image size
    const MAX_ANGLE: f32 = 45.0;

    /// Creates a new CropSettings with the specified values.
    /// All values are clamped to valid ranges.
    pub fn new(
        crop_x: f32,
        crop_y: f32,
        crop_width: f32,
        crop_height: f32,
        rotation_90: i32,
        angle: f32,
        flip_horizontal: bool,
        flip_vertical: bool,
    ) -> Self {
        let crop_x = crop_x.clamp(0.0, 1.0);
        let crop_y = crop_y.clamp(0.0, 1.0);
        let crop_width = crop_width.clamp(Self::MIN_CROP_SIZE, 1.0);
        let crop_height = crop_height.clamp(Self::MIN_CROP_SIZE, 1.0);

        // Ensure crop doesn't extend beyond image bounds
        let crop_width = crop_width.min(1.0 - crop_x);
        let crop_height = crop_height.min(1.0 - crop_y);

        let rotation_90 = rotation_90.clamp(-1, 3);
        let angle = angle.clamp(-Self::MAX_ANGLE, Self::MAX_ANGLE);

        Self {
            crop_x,
            crop_y,
            crop_width,
            crop_height,
            rotation_90,
            angle,
            flip_horizontal,
            flip_vertical,
        }
    }

    // Getters
    pub fn crop_x(&self) -> f32 {
        self.crop_x
    }

    pub fn crop_y(&self) -> f32 {
        self.crop_y
    }

    pub fn crop_width(&self) -> f32 {
        self.crop_width
    }

    pub fn crop_height(&self) -> f32 {
        self.crop_height
    }

    pub fn rotation_90(&self) -> i32 {
        self.rotation_90
    }

    pub fn angle(&self) -> f32 {
        self.angle
    }

    pub fn flip_horizontal(&self) -> bool {
        self.flip_horizontal
    }

    pub fn flip_vertical(&self) -> bool {
        self.flip_vertical
    }

    /// Returns the total rotation in degrees (90° increments + fine angle)
    pub fn total_rotation(&self) -> f32 {
        (self.rotation_90 as f32 * 90.0) + self.angle
    }

    /// Returns true if the image is cropped (not full frame)
    pub fn is_cropped(&self) -> bool {
        self.crop_x > 0.0
            || self.crop_y > 0.0
            || self.crop_width < 1.0
            || self.crop_height < 1.0
    }

    /// Returns true if the image is rotated
    pub fn is_rotated(&self) -> bool {
        self.rotation_90 != 0 || self.angle.abs() > 0.01
    }

    /// Returns true if the image is flipped
    pub fn is_flipped(&self) -> bool {
        self.flip_horizontal || self.flip_vertical
    }

    /// Returns true if any crop/rotate/flip modifications are applied
    pub fn has_modifications(&self) -> bool {
        self.is_cropped() || self.is_rotated() || self.is_flipped()
    }
}

impl Default for CropSettings {
    fn default() -> Self {
        Self {
            crop_x: 0.0,
            crop_y: 0.0,
            crop_width: 1.0,
            crop_height: 1.0,
            rotation_90: 0,
            angle: 0.0,
            flip_horizontal: false,
            flip_vertical: false,
        }
    }
}

#[cfg(test)]
mod crop_settings_tests {
    use super::*;

    #[test]
    fn test_create_default_crop_settings() {
        let crop = CropSettings::default();
        
        assert_eq!(crop.crop_x(), 0.0);
        assert_eq!(crop.crop_y(), 0.0);
        assert_eq!(crop.crop_width(), 1.0);
        assert_eq!(crop.crop_height(), 1.0);
        assert_eq!(crop.rotation_90(), 0);
        assert_eq!(crop.angle(), 0.0);
        assert!(!crop.flip_horizontal());
        assert!(!crop.flip_vertical());
    }

    #[test]
    fn test_create_crop_settings_with_valid_values() {
        let crop = CropSettings::new(0.1, 0.2, 0.5, 0.6, 1, 15.0, true, false);
        
        assert_eq!(crop.crop_x(), 0.1);
        assert_eq!(crop.crop_y(), 0.2);
        assert_eq!(crop.crop_width(), 0.5);
        assert_eq!(crop.crop_height(), 0.6);
        assert_eq!(crop.rotation_90(), 1);
        assert_eq!(crop.angle(), 15.0);
        assert!(crop.flip_horizontal());
        assert!(!crop.flip_vertical());
    }

    #[test]
    fn test_crop_x_clamped_to_valid_range() {
        let crop = CropSettings::new(-0.5, 0.0, 0.5, 0.5, 0, 0.0, false, false);
        assert_eq!(crop.crop_x(), 0.0);

        let crop = CropSettings::new(1.5, 0.0, 0.5, 0.5, 0, 0.0, false, false);
        assert_eq!(crop.crop_x(), 1.0);
    }

    #[test]
    fn test_crop_y_clamped_to_valid_range() {
        let crop = CropSettings::new(0.0, -0.5, 0.5, 0.5, 0, 0.0, false, false);
        assert_eq!(crop.crop_y(), 0.0);

        let crop = CropSettings::new(0.0, 1.5, 0.5, 0.5, 0, 0.0, false, false);
        assert_eq!(crop.crop_y(), 1.0);
    }

    #[test]
    fn test_crop_width_clamped_to_valid_range() {
        let crop = CropSettings::new(0.0, 0.0, -0.1, 0.5, 0, 0.0, false, false);
        assert!(crop.crop_width() > 0.0);

        let crop = CropSettings::new(0.0, 0.0, 1.5, 0.5, 0, 0.0, false, false);
        assert_eq!(crop.crop_width(), 1.0);
    }

    #[test]
    fn test_crop_height_clamped_to_valid_range() {
        let crop = CropSettings::new(0.0, 0.0, 0.5, -0.1, 0, 0.0, false, false);
        assert!(crop.crop_height() > 0.0);

        let crop = CropSettings::new(0.0, 0.0, 0.5, 1.5, 0, 0.0, false, false);
        assert_eq!(crop.crop_height(), 1.0);
    }

    #[test]
    fn test_crop_bounds_validation() {
        // Crop extends beyond right edge
        let crop = CropSettings::new(0.7, 0.0, 0.5, 0.5, 0, 0.0, false, false);
        assert!(crop.crop_x() + crop.crop_width() <= 1.0);

        // Crop extends beyond bottom edge
        let crop = CropSettings::new(0.0, 0.7, 0.5, 0.5, 0, 0.0, false, false);
        assert!(crop.crop_y() + crop.crop_height() <= 1.0);
    }

    #[test]
    fn test_rotation_90_clamped() {
        let crop = CropSettings::new(0.0, 0.0, 1.0, 1.0, -2, 0.0, false, false);
        assert!(crop.rotation_90() >= -1);

        let crop = CropSettings::new(0.0, 0.0, 1.0, 1.0, 5, 0.0, false, false);
        assert!(crop.rotation_90() <= 3);
    }

    #[test]
    fn test_angle_clamped_to_valid_range() {
        let crop = CropSettings::new(0.0, 0.0, 1.0, 1.0, 0, -50.0, false, false);
        assert_eq!(crop.angle(), -45.0);

        let crop = CropSettings::new(0.0, 0.0, 1.0, 1.0, 0, 50.0, false, false);
        assert_eq!(crop.angle(), 45.0);
    }

    #[test]
    fn test_total_rotation_calculation() {
        let crop = CropSettings::new(0.0, 0.0, 1.0, 1.0, 1, 15.0, false, false);
        assert_eq!(crop.total_rotation(), 90.0 + 15.0);

        let crop = CropSettings::new(0.0, 0.0, 1.0, 1.0, -1, -10.0, false, false);
        assert_eq!(crop.total_rotation(), -90.0 - 10.0);
    }

    #[test]
    fn test_is_cropped() {
        let default_crop = CropSettings::default();
        assert!(!default_crop.is_cropped());

        let cropped = CropSettings::new(0.1, 0.1, 0.8, 0.8, 0, 0.0, false, false);
        assert!(cropped.is_cropped());
    }

    #[test]
    fn test_is_rotated() {
        let default_crop = CropSettings::default();
        assert!(!default_crop.is_rotated());

        let rotated_90 = CropSettings::new(0.0, 0.0, 1.0, 1.0, 1, 0.0, false, false);
        assert!(rotated_90.is_rotated());

        let rotated_fine = CropSettings::new(0.0, 0.0, 1.0, 1.0, 0, 5.0, false, false);
        assert!(rotated_fine.is_rotated());
    }

    #[test]
    fn test_is_flipped() {
        let default_crop = CropSettings::default();
        assert!(!default_crop.is_flipped());

        let flipped_h = CropSettings::new(0.0, 0.0, 1.0, 1.0, 0, 0.0, true, false);
        assert!(flipped_h.is_flipped());

        let flipped_v = CropSettings::new(0.0, 0.0, 1.0, 1.0, 0, 0.0, false, true);
        assert!(flipped_v.is_flipped());
    }

    #[test]
    fn test_has_modifications() {
        let default_crop = CropSettings::default();
        assert!(!default_crop.has_modifications());

        let cropped = CropSettings::new(0.1, 0.0, 0.9, 1.0, 0, 0.0, false, false);
        assert!(cropped.has_modifications());

        let rotated = CropSettings::new(0.0, 0.0, 1.0, 1.0, 1, 0.0, false, false);
        assert!(rotated.has_modifications());

        let flipped = CropSettings::new(0.0, 0.0, 1.0, 1.0, 0, 0.0, true, false);
        assert!(flipped.has_modifications());
    }
}
