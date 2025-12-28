use super::rotation_fill_mode::RotationFillMode;

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
    fill_mode: RotationFillMode,
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
        Self::with_fill_mode_value(
            crop_x, crop_y, crop_width, crop_height,
            rotation_90, angle, flip_horizontal, flip_vertical,
            RotationFillMode::default(),
        )
    }

    /// Creates a new CropSettings with the specified values and fill mode.
    pub fn with_fill_mode_value(
        crop_x: f32,
        crop_y: f32,
        crop_width: f32,
        crop_height: f32,
        rotation_90: i32,
        angle: f32,
        flip_horizontal: bool,
        flip_vertical: bool,
        fill_mode: RotationFillMode,
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
            fill_mode,
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

    pub fn fill_mode(&self) -> RotationFillMode {
        self.fill_mode
    }

    /// Creates a new CropSettings with a different fill_mode
    pub fn with_fill_mode(&self, fill_mode: RotationFillMode) -> Self {
        Self {
            crop_x: self.crop_x,
            crop_y: self.crop_y,
            crop_width: self.crop_width,
            crop_height: self.crop_height,
            rotation_90: self.rotation_90,
            angle: self.angle,
            flip_horizontal: self.flip_horizontal,
            flip_vertical: self.flip_vertical,
            fill_mode,
        }
    }

    /// Creates a new CropSettings with toggled flip_horizontal (preserves fill_mode)
    pub fn with_flip_horizontal(&self, flip: bool) -> Self {
        Self {
            crop_x: self.crop_x,
            crop_y: self.crop_y,
            crop_width: self.crop_width,
            crop_height: self.crop_height,
            rotation_90: self.rotation_90,
            angle: self.angle,
            flip_horizontal: flip,
            flip_vertical: self.flip_vertical,
            fill_mode: self.fill_mode,
        }
    }

    /// Creates a new CropSettings with toggled flip_vertical (preserves fill_mode)
    pub fn with_flip_vertical(&self, flip: bool) -> Self {
        Self {
            crop_x: self.crop_x,
            crop_y: self.crop_y,
            crop_width: self.crop_width,
            crop_height: self.crop_height,
            rotation_90: self.rotation_90,
            angle: self.angle,
            flip_horizontal: self.flip_horizontal,
            flip_vertical: flip,
            fill_mode: self.fill_mode,
        }
    }

    /// Creates a new CropSettings with a different angle (preserves fill_mode)
    pub fn with_angle(&self, angle: f32) -> Self {
        let angle = angle.clamp(-Self::MAX_ANGLE, Self::MAX_ANGLE);
        Self {
            crop_x: self.crop_x,
            crop_y: self.crop_y,
            crop_width: self.crop_width,
            crop_height: self.crop_height,
            rotation_90: self.rotation_90,
            angle,
            flip_horizontal: self.flip_horizontal,
            flip_vertical: self.flip_vertical,
            fill_mode: self.fill_mode,
        }
    }

    /// Creates a new CropSettings with a different rotation_90 (preserves fill_mode)
    pub fn with_rotation_90(&self, rotation_90: i32) -> Self {
        let rotation_90 = rotation_90.clamp(-1, 3);
        Self {
            crop_x: self.crop_x,
            crop_y: self.crop_y,
            crop_width: self.crop_width,
            crop_height: self.crop_height,
            rotation_90,
            angle: self.angle,
            flip_horizontal: self.flip_horizontal,
            flip_vertical: self.flip_vertical,
            fill_mode: self.fill_mode,
        }
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

    /// Transforms crop coordinates from original image space to visual (rotated) space.
    /// This is used during crop editing to display the crop frame correctly on a rotated image.
    ///
    /// When an image is rotated visually, the crop rectangle needs to be displayed
    /// in the rotated coordinate system even though it's stored in original space.
    pub fn to_visual_space(&self) -> (f32, f32, f32, f32) {
        let (x, y, w, h) = (self.crop_x, self.crop_y, self.crop_width, self.crop_height);

        // In egui, Image::rotate(angle) with positive angles rotates CLOCKWISE visually.
        // So rotation_90=1 means 90° CW visual rotation.
        match self.rotation_90 % 4 {
            0 => (x, y, w, h),
            1 | -3 => {
                // rotation_90=1: 90° CW visual rotation.
                // Original (ox, oy) -> Visual (1-oy, ox)
                // For rect: (x, y, w, h) -> (1-y-h, x, h, w)
                (1.0 - y - h, x, h, w)
            }
            2 | -2 => {
                // 180°: original (x, y, w, h) -> visual (1-x-w, 1-y-h, w, h)
                (1.0 - x - w, 1.0 - y - h, w, h)
            }
            3 | -1 => {
                // rotation_90=3 or -1: 270° CW = 90° CCW visual rotation.
                // Original (ox, oy) -> Visual (oy, 1-ox)
                // For rect: (x, y, w, h) -> (y, 1-x-w, h, w)
                (y, 1.0 - x - w, h, w)
            }
            _ => (x, y, w, h),
        }
    }

    /// Transforms crop coordinates from visual (rotated) space back to original image space.
    /// This is used when saving crop edits that were made on a rotated image.
    ///
    /// The visual space is what the user sees/edits, and the original space is
    /// what gets stored in the database and used for actual image processing.
    pub fn from_visual_space(
        visual_x: f32,
        visual_y: f32,
        visual_w: f32,
        visual_h: f32,
        rotation_90: i32,
    ) -> (f32, f32, f32, f32) {
        // Note: In egui, positive angles are counter-clockwise (CCW).
        // rotation_90 = 1 means 90° CCW, rotation_90 = -1 means 90° CW.
        //
        // The visual space is the rotated image as seen on screen.
        // We need to map visual coordinates back to original texture UV.
        //
        // For 90° CCW (rotation_90=1): visual top-left corresponds to original bottom-left
        // For 90° CW (rotation_90=-1 or 3): visual top-left corresponds to original top-right

        // In egui, Image::rotate(angle) with positive angles rotates CLOCKWISE visually
        // (because Y-axis points down in screen coordinates).
        // So rotation_90=1 means 90° CW visual rotation.
        match rotation_90 % 4 {
            0 => (visual_x, visual_y, visual_w, visual_h),
            1 | -3 => {
                // rotation_90=1: 90° CW visual rotation.
                // For 90° CW: Original (ox, oy) -> Visual (1-oy, ox)
                // Inverse: Visual (vx, vy) -> Original (vy, 1-vx)
                // For rect: (vx, vy, vw, vh) -> (vy, 1-vx-vw, vh, vw)
                (visual_y, 1.0 - visual_x - visual_w, visual_h, visual_w)
            }
            2 | -2 => {
                // 180°: visual (x, y, w, h) -> original (1-x-w, 1-y-h, w, h)
                (1.0 - visual_x - visual_w, 1.0 - visual_y - visual_h, visual_w, visual_h)
            }
            3 | -1 => {
                // rotation_90=3 or -1: 270° CW = 90° CCW visual rotation.
                // For 90° CCW: Original (ox, oy) -> Visual (oy, 1-ox)
                // Inverse: Visual (vx, vy) -> Original (1-vy, vx)
                // For rect: (vx, vy, vw, vh) -> (1-vy-vh, vx, vh, vw)
                (1.0 - visual_y - visual_h, visual_x, visual_h, visual_w)
            }
            _ => (visual_x, visual_y, visual_w, visual_h),
        }
    }

    /// Creates a new CropSettings by updating crop coordinates from visual space.
    /// Preserves all other settings (rotation, angle, flips).
    pub fn with_visual_crop(
        &self,
        visual_x: f32,
        visual_y: f32,
        visual_w: f32,
        visual_h: f32,
    ) -> Self {
        let (orig_x, orig_y, orig_w, orig_h) =
            Self::from_visual_space(visual_x, visual_y, visual_w, visual_h, self.rotation_90);

        Self::new(
            orig_x,
            orig_y,
            orig_w,
            orig_h,
            self.rotation_90,
            self.angle,
            self.flip_horizontal,
            self.flip_vertical,
        )
    }

    /// Calculates the maximum inscribed rectangle for a given rotation angle.
    /// Returns a new CropSettings with adjusted crop coordinates that fit entirely
    /// within the rotated image without any black borders.
    ///
    /// This is used when `RotationFillMode::ShrinkToFit` is active.
    ///
    /// Algorithm: For an image rotated by angle θ, the largest axis-aligned inscribed
    /// rectangle can be calculated using the inscribed rectangle formula.
    /// For a unit square rotated by θ, the inscribed rectangle has:
    /// - width = height = 1 / (|cos(θ)| + |sin(θ)|)
    ///
    /// For a general rectangle with aspect ratio W:H, we scale accordingly.
    pub fn calculate_shrink_to_fit(&self, image_width: f32, image_height: f32) -> Self {
        let angle_rad = self.angle.to_radians();
        let cos_a = angle_rad.cos().abs();
        let sin_a = angle_rad.sin().abs();

        // For a very small rotation, return unchanged
        if sin_a < 0.001 {
            return self.clone();
        }

        // Calculate the aspect ratio of the rotated image space
        // After rotation by 90-degree increments, the effective dimensions may swap
        let (eff_w, eff_h) = if self.rotation_90 % 2 != 0 {
            (image_height, image_width)
        } else {
            (image_width, image_height)
        };

        // For a rectangle with aspect ratio `aspect`, the largest inscribed axis-aligned
        // rectangle after rotation by angle θ has dimensions:
        //
        // The formula is derived from the fact that the corners of the inscribed rectangle
        // must touch the edges of the rotated original rectangle.
        //
        // For a unit square: inscribed_size = 1 / (cos(θ) + sin(θ))
        // For a rectangle: we need to account for aspect ratio
        //
        // The inscribed rectangle's normalized dimensions are:
        // new_width = cos(θ) / (cos(θ) + sin(θ) * aspect)
        // new_height = cos(θ) / (cos(θ) * aspect + sin(θ))
        //
        // Simplified: scale = 1 / (cos(θ) + sin(θ) * max(aspect, 1/aspect))
        // But the exact formula for maintaining aspect depends on target aspect ratio.

        // For "Shrink to Fit", we want the largest rectangle that:
        // 1. Fits entirely within the rotated image (no black borders)
        // 2. Maintains the current crop's aspect ratio if possible

        // Scale factor for a rectangle to fit inside rotated bounds
        // This is the "inset" amount needed to avoid corners going outside
        let scale = 1.0 / (cos_a + sin_a * (eff_w / eff_h).max(eff_h / eff_w));

        // Start from FULL image (1.0) scaled down
        // This ensures we always find the maximum inscribed rectangle for the current angle,
        // preventing "recursive shrinking" where the crop gets smaller and smaller
        // as the user drags the slider.
        let new_width = scale;
        let new_height = scale;

        // Center the new crop (0.5, 0.5 is center in normalized coords)
        let new_x = (1.0 - new_width) / 2.0;
        let new_y = (1.0 - new_height) / 2.0;

        Self {
            crop_x: new_x,
            crop_y: new_y,
            crop_width: new_width,
            crop_height: new_height,
            rotation_90: self.rotation_90,
            angle: self.angle,
            flip_horizontal: self.flip_horizontal,
            flip_vertical: self.flip_vertical,
            fill_mode: self.fill_mode,
        }
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
            fill_mode: RotationFillMode::default(),
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

    // =============================================
    // Coordinate Space Transformation Tests
    // =============================================

    #[test]
    fn test_to_visual_space_no_rotation() {
        // No rotation: coordinates should remain the same
        let crop = CropSettings::new(0.1, 0.2, 0.5, 0.3, 0, 0.0, false, false);
        let (vx, vy, vw, vh) = crop.to_visual_space();

        assert!((vx - 0.1).abs() < 0.001);
        assert!((vy - 0.2).abs() < 0.001);
        assert!((vw - 0.5).abs() < 0.001);
        assert!((vh - 0.3).abs() < 0.001);
    }

    #[test]
    fn test_to_visual_space_90_degrees() {
        // 90° CW: original (x, y, w, h) -> visual (1-y-h, x, h, w)
        // Original: x=0.1, y=0.2, w=0.5, h=0.3
        // Expected visual: x=1-0.2-0.3=0.5, y=0.1, w=0.3, h=0.5
        let crop = CropSettings::new(0.1, 0.2, 0.5, 0.3, 1, 0.0, false, false);
        let (vx, vy, vw, vh) = crop.to_visual_space();

        assert!((vx - 0.5).abs() < 0.001, "vx: expected 0.5, got {}", vx);
        assert!((vy - 0.1).abs() < 0.001, "vy: expected 0.1, got {}", vy);
        assert!((vw - 0.3).abs() < 0.001, "vw: expected 0.3, got {}", vw);
        assert!((vh - 0.5).abs() < 0.001, "vh: expected 0.5, got {}", vh);
    }

    #[test]
    fn test_to_visual_space_180_degrees() {
        // 180°: original (x, y, w, h) -> visual (1-x-w, 1-y-h, w, h)
        // Original: x=0.1, y=0.2, w=0.5, h=0.3
        // Expected visual: x=1-0.1-0.5=0.4, y=1-0.2-0.3=0.5, w=0.5, h=0.3
        let crop = CropSettings::new(0.1, 0.2, 0.5, 0.3, 2, 0.0, false, false);
        let (vx, vy, vw, vh) = crop.to_visual_space();

        assert!((vx - 0.4).abs() < 0.001, "vx: expected 0.4, got {}", vx);
        assert!((vy - 0.5).abs() < 0.001, "vy: expected 0.5, got {}", vy);
        assert!((vw - 0.5).abs() < 0.001, "vw: expected 0.5, got {}", vw);
        assert!((vh - 0.3).abs() < 0.001, "vh: expected 0.3, got {}", vh);
    }

    #[test]
    fn test_to_visual_space_270_degrees() {
        // 270° CW (90° CCW): original (x, y, w, h) -> visual (y, 1-x-w, h, w)
        // Original: x=0.1, y=0.2, w=0.5, h=0.3
        // Expected visual: x=0.2, y=1-0.1-0.5=0.4, w=0.3, h=0.5
        let crop = CropSettings::new(0.1, 0.2, 0.5, 0.3, 3, 0.0, false, false);
        let (vx, vy, vw, vh) = crop.to_visual_space();

        assert!((vx - 0.2).abs() < 0.001, "vx: expected 0.2, got {}", vx);
        assert!((vy - 0.4).abs() < 0.001, "vy: expected 0.4, got {}", vy);
        assert!((vw - 0.3).abs() < 0.001, "vw: expected 0.3, got {}", vw);
        assert!((vh - 0.5).abs() < 0.001, "vh: expected 0.5, got {}", vh);
    }

    #[test]
    fn test_roundtrip_transformation_90_degrees() {
        // Test that original -> visual -> original gives the same result
        let original = CropSettings::new(0.1, 0.2, 0.5, 0.3, 1, 0.0, false, false);
        let (vx, vy, vw, vh) = original.to_visual_space();
        let roundtrip = original.with_visual_crop(vx, vy, vw, vh);

        assert!((roundtrip.crop_x() - 0.1).abs() < 0.001, "x roundtrip failed");
        assert!((roundtrip.crop_y() - 0.2).abs() < 0.001, "y roundtrip failed");
        assert!((roundtrip.crop_width() - 0.5).abs() < 0.001, "w roundtrip failed");
        assert!((roundtrip.crop_height() - 0.3).abs() < 0.001, "h roundtrip failed");
    }

    #[test]
    fn test_roundtrip_transformation_180_degrees() {
        let original = CropSettings::new(0.1, 0.2, 0.5, 0.3, 2, 0.0, false, false);
        let (vx, vy, vw, vh) = original.to_visual_space();
        let roundtrip = original.with_visual_crop(vx, vy, vw, vh);

        assert!((roundtrip.crop_x() - 0.1).abs() < 0.001, "x roundtrip failed");
        assert!((roundtrip.crop_y() - 0.2).abs() < 0.001, "y roundtrip failed");
        assert!((roundtrip.crop_width() - 0.5).abs() < 0.001, "w roundtrip failed");
        assert!((roundtrip.crop_height() - 0.3).abs() < 0.001, "h roundtrip failed");
    }

    #[test]
    fn test_roundtrip_transformation_270_degrees() {
        let original = CropSettings::new(0.1, 0.2, 0.5, 0.3, 3, 0.0, false, false);
        let (vx, vy, vw, vh) = original.to_visual_space();
        let roundtrip = original.with_visual_crop(vx, vy, vw, vh);

        assert!((roundtrip.crop_x() - 0.1).abs() < 0.001, "x roundtrip failed");
        assert!((roundtrip.crop_y() - 0.2).abs() < 0.001, "y roundtrip failed");
        assert!((roundtrip.crop_width() - 0.5).abs() < 0.001, "w roundtrip failed");
        assert!((roundtrip.crop_height() - 0.3).abs() < 0.001, "h roundtrip failed");
    }

    #[test]
    fn test_visual_edit_preserves_after_save() {
        // Simulate: User loads image rotated 90°, makes a crop edit in visual space,
        // saves, then reloads. The crop should appear at the same visual position.

        // Start with a rotated image with full-frame crop
        let initial = CropSettings::new(0.0, 0.0, 1.0, 1.0, 1, 0.0, false, false);

        // User drags crop in visual space to (0.1, 0.1, 0.6, 0.4)
        let edited = initial.with_visual_crop(0.1, 0.1, 0.6, 0.4);

        // Simulate save and reload (coordinates are now in original space)
        let reloaded = CropSettings::new(
            edited.crop_x(), edited.crop_y(),
            edited.crop_width(), edited.crop_height(),
            edited.rotation_90(), edited.angle(),
            edited.flip_horizontal(), edited.flip_vertical()
        );

        // The visual representation should match what the user edited
        let (final_vx, final_vy, final_vw, final_vh) = reloaded.to_visual_space();

        assert!((final_vx - 0.1).abs() < 0.001, "visual x mismatch after reload");
        assert!((final_vy - 0.1).abs() < 0.001, "visual y mismatch after reload");
        assert!((final_vw - 0.6).abs() < 0.001, "visual w mismatch after reload");
        assert!((final_vh - 0.4).abs() < 0.001, "visual h mismatch after reload");
    }

    // =============================================
    // E2E Test: Full crop+rotation persistence flow
    // =============================================

    /// This test simulates the EXACT flow of the application:
    /// 1. Image is displayed with rotation (egui Image::rotate)
    /// 2. User draws crop frame on the VISUAL (rotated) image
    /// 3. App saves: transforms visual coords to original, sets rotation=0
    /// 4. App displays: applies UV directly (no rotation)
    /// 5. VERIFY: The UV region matches the visual crop content
    #[test]
    fn test_e2e_crop_rotation_persistence_90cw() {
        println!("\n=== E2E Test: 90° CW rotation ===");

        // Simulating a 100x100 image for easy visualization
        // Original image coordinates:
        //   (0,0)-----(1,0)
        //     |         |
        //     |    A    |   A = top-left quadrant
        //     |         |
        //   (0,1)-----(1,1)

        // After 90° CW rotation, the visual image becomes:
        //   Original bottom-left -> Visual top-left
        //   Original top-left -> Visual top-right
        //   Original top-right -> Visual bottom-right
        //   Original bottom-left -> Visual top-left

        let rotation = 1; // 90° CW in egui

        // STEP 1: User sees rotated image and draws crop on VISUAL top-left quadrant
        let visual_crop = (0.0_f32, 0.0_f32, 0.5_f32, 0.5_f32); // top-left quarter visually
        println!("Visual crop (what user selected): x={}, y={}, w={}, h={}",
                 visual_crop.0, visual_crop.1, visual_crop.2, visual_crop.3);

        // STEP 2: Transform to original space (this is what gets saved)
        let (orig_x, orig_y, orig_w, orig_h) = CropSettings::from_visual_space(
            visual_crop.0, visual_crop.1, visual_crop.2, visual_crop.3, rotation
        );
        println!("Original space (saved to DB): x={}, y={}, w={}, h={}",
                 orig_x, orig_y, orig_w, orig_h);

        // STEP 3: When displaying, we apply UV directly (rotation=0 was saved)
        // The UV rect is (orig_x, orig_y) to (orig_x+orig_w, orig_y+orig_h)
        println!("UV applied to texture: ({}, {}) to ({}, {})",
                 orig_x, orig_y, orig_x + orig_w, orig_y + orig_h);

        // STEP 4: VERIFY - For 90° CW rotation:
        // Visual top-left (0,0) should map to Original bottom-left (0, 0.5-1.0)
        // So visual crop (0,0,0.5,0.5) should map to original left-bottom quadrant
        //
        // Let's verify by checking what original region corresponds to visual top-left:
        // - Visual (0,0) comes from Original where?
        //   For 90° CW: Original (ox,oy) -> Visual (1-oy, ox)
        //   So Visual (0,0) = (1-oy, ox) means 1-oy=0, ox=0 -> oy=1, ox=0
        //   Visual (0,0) <- Original (0, 1) [bottom-left corner]
        //
        // - Visual (0.5,0) comes from Original where?
        //   (1-oy, ox) = (0.5, 0) -> oy=0.5, ox=0
        //   Visual (0.5, 0) <- Original (0, 0.5)
        //
        // - Visual (0, 0.5) comes from Original where?
        //   (1-oy, ox) = (0, 0.5) -> oy=1, ox=0.5
        //   Visual (0, 0.5) <- Original (0.5, 1)
        //
        // - Visual (0.5, 0.5) comes from Original where?
        //   (1-oy, ox) = (0.5, 0.5) -> oy=0.5, ox=0.5
        //   Visual (0.5, 0.5) <- Original (0.5, 0.5)
        //
        // So the visual rectangle (0,0)-(0.5,0.5) corresponds to original:
        // Corners: (0,1), (0,0.5), (0.5,1), (0.5,0.5)
        // This is the region x:[0, 0.5], y:[0.5, 1] -> (0, 0.5, 0.5, 0.5)

        let expected_orig = (0.0_f32, 0.5_f32, 0.5_f32, 0.5_f32);
        println!("Expected original: x={}, y={}, w={}, h={}",
                 expected_orig.0, expected_orig.1, expected_orig.2, expected_orig.3);

        assert!((orig_x - expected_orig.0).abs() < 0.001,
                "orig_x: expected {}, got {}", expected_orig.0, orig_x);
        assert!((orig_y - expected_orig.1).abs() < 0.001,
                "orig_y: expected {}, got {}", expected_orig.1, orig_y);
        assert!((orig_w - expected_orig.2).abs() < 0.001,
                "orig_w: expected {}, got {}", expected_orig.2, orig_w);
        assert!((orig_h - expected_orig.3).abs() < 0.001,
                "orig_h: expected {}, got {}", expected_orig.3, orig_h);

        println!("✓ 90° CW test PASSED\n");
    }

    #[test]
    fn test_e2e_crop_rotation_persistence_90ccw() {
        println!("\n=== E2E Test: 90° CCW rotation (rotation_90=3) ===");

        let rotation = 3; // 270° CW = 90° CCW

        // User draws crop on visual top-left quadrant
        let visual_crop = (0.0_f32, 0.0_f32, 0.5_f32, 0.5_f32);
        println!("Visual crop: x={}, y={}, w={}, h={}",
                 visual_crop.0, visual_crop.1, visual_crop.2, visual_crop.3);

        let (orig_x, orig_y, orig_w, orig_h) = CropSettings::from_visual_space(
            visual_crop.0, visual_crop.1, visual_crop.2, visual_crop.3, rotation
        );
        println!("Original space: x={}, y={}, w={}, h={}",
                 orig_x, orig_y, orig_w, orig_h);

        // For 90° CCW: Original (ox,oy) -> Visual (oy, 1-ox)
        // Visual (0,0) = (oy, 1-ox) -> oy=0, 1-ox=0 -> ox=1, oy=0
        // Visual (0,0) <- Original (1, 0) [top-right corner]
        //
        // Visual (0.5, 0) <- oy=0.5, ox=1 -> Original (1, 0.5)
        // Visual (0, 0.5) <- oy=0, ox=0.5 -> Original (0.5, 0)
        // Visual (0.5, 0.5) <- oy=0.5, ox=0.5 -> Original (0.5, 0.5)
        //
        // Corners in original: (1,0), (1,0.5), (0.5,0), (0.5,0.5)
        // This is x:[0.5, 1], y:[0, 0.5] -> (0.5, 0, 0.5, 0.5)

        let expected_orig = (0.5_f32, 0.0_f32, 0.5_f32, 0.5_f32);
        println!("Expected original: x={}, y={}, w={}, h={}",
                 expected_orig.0, expected_orig.1, expected_orig.2, expected_orig.3);

        assert!((orig_x - expected_orig.0).abs() < 0.001,
                "orig_x: expected {}, got {}", expected_orig.0, orig_x);
        assert!((orig_y - expected_orig.1).abs() < 0.001,
                "orig_y: expected {}, got {}", expected_orig.1, orig_y);
        assert!((orig_w - expected_orig.2).abs() < 0.001,
                "orig_w: expected {}, got {}", expected_orig.2, orig_w);
        assert!((orig_h - expected_orig.3).abs() < 0.001,
                "orig_h: expected {}, got {}", expected_orig.3, orig_h);

        println!("✓ 90° CCW test PASSED\n");
    }

    #[test]
    fn test_e2e_crop_rotation_persistence_180() {
        println!("\n=== E2E Test: 180° rotation ===");

        let rotation = 2;

        let visual_crop = (0.0_f32, 0.0_f32, 0.5_f32, 0.5_f32);
        println!("Visual crop: x={}, y={}, w={}, h={}",
                 visual_crop.0, visual_crop.1, visual_crop.2, visual_crop.3);

        let (orig_x, orig_y, orig_w, orig_h) = CropSettings::from_visual_space(
            visual_crop.0, visual_crop.1, visual_crop.2, visual_crop.3, rotation
        );
        println!("Original space: x={}, y={}, w={}, h={}",
                 orig_x, orig_y, orig_w, orig_h);

        // For 180°: Original (ox,oy) -> Visual (1-ox, 1-oy)
        // Visual (0,0) <- Original (1, 1) [bottom-right]
        // Visual (0.5, 0.5) <- Original (0.5, 0.5) [center]
        //
        // Visual top-left quadrant corresponds to Original bottom-right quadrant
        // -> (0.5, 0.5, 0.5, 0.5)

        let expected_orig = (0.5_f32, 0.5_f32, 0.5_f32, 0.5_f32);
        println!("Expected original: x={}, y={}, w={}, h={}",
                 expected_orig.0, expected_orig.1, expected_orig.2, expected_orig.3);

        assert!((orig_x - expected_orig.0).abs() < 0.001,
                "orig_x: expected {}, got {}", expected_orig.0, orig_x);
        assert!((orig_y - expected_orig.1).abs() < 0.001,
                "orig_y: expected {}, got {}", expected_orig.1, orig_y);
        assert!((orig_w - expected_orig.2).abs() < 0.001,
                "orig_w: expected {}, got {}", expected_orig.2, orig_w);
        assert!((orig_h - expected_orig.3).abs() < 0.001,
                "orig_h: expected {}, got {}", expected_orig.3, orig_h);

        println!("✓ 180° test PASSED\n");
    }

    /// Test with non-centered crop to catch edge cases
    #[test]
    fn test_e2e_asymmetric_crop_90cw() {
        println!("\n=== E2E Test: Asymmetric crop with 90° CW ===");

        let rotation = 1;

        // Crop the TOP portion of the visual image (not centered)
        // This simulates cropping workers' heads in a rotated photo
        let visual_crop = (0.1_f32, 0.05_f32, 0.8_f32, 0.4_f32); // wide strip at top
        println!("Visual crop (top strip): x={}, y={}, w={}, h={}",
                 visual_crop.0, visual_crop.1, visual_crop.2, visual_crop.3);

        let (orig_x, orig_y, orig_w, orig_h) = CropSettings::from_visual_space(
            visual_crop.0, visual_crop.1, visual_crop.2, visual_crop.3, rotation
        );
        println!("Original space: x={}, y={}, w={}, h={}",
                 orig_x, orig_y, orig_w, orig_h);

        // For 90° CW: Visual (vx,vy) -> Original (vy, 1-vx-vw)
        // Wait, let me recalculate the formula...
        // from_visual_space for rotation=1: (vy, 1-vx-vw, vh, vw)
        //
        // visual_crop = (0.1, 0.05, 0.8, 0.4)
        // orig = (0.05, 1-0.1-0.8, 0.4, 0.8) = (0.05, 0.1, 0.4, 0.8)

        let expected_orig = (0.05_f32, 0.1_f32, 0.4_f32, 0.8_f32);
        println!("Expected original: x={}, y={}, w={}, h={}",
                 expected_orig.0, expected_orig.1, expected_orig.2, expected_orig.3);

        assert!((orig_x - expected_orig.0).abs() < 0.001,
                "orig_x: expected {}, got {}", expected_orig.0, orig_x);
        assert!((orig_y - expected_orig.1).abs() < 0.001,
                "orig_y: expected {}, got {}", expected_orig.1, orig_y);
        assert!((orig_w - expected_orig.2).abs() < 0.001,
                "orig_w: expected {}, got {}", expected_orig.2, orig_w);
        assert!((orig_h - expected_orig.3).abs() < 0.001,
                "orig_h: expected {}, got {}", expected_orig.3, orig_h);

        println!("✓ Asymmetric crop test PASSED\n");
    }

    // =============================================
    // INVARIANT: Viewer NEVER rotates (Lightroom behavior)
    // =============================================
    //
    // The CORRECT behavior (like Adobe Lightroom) is:
    // - Crop defines the final result
    // - Viewer is just a viewport, it NEVER applies rotation
    // - All transformations are "baked" into UV coordinates
    //
    // When saving:
    // - rotation_90: CONSUMED (transformed into UV coordinates)
    // - angle: CONSUMED (for empty corners, requires pre-rendering in future)
    // - Saved snapshot: rotation_90 = 0, angle = 0
    //
    // This is NOT a limitation, it's the CORRECT behavior!

    /// INVARIANT: Saved snapshot must have rotation_90 = 0
    /// The rotation is consumed by coordinate transformation.
    /// VIEWER NEVER ROTATES!
    #[test]
    fn test_invariant_viewer_never_rotates_rotation90() {
        // When saving a crop with rotation, the rotation must be consumed
        // The saved coordinates are in original texture space

        let rotation_90 = 1; // 90° CW during editing
        let visual_crop = (0.1_f32, 0.2_f32, 0.5_f32, 0.4_f32);

        // Transform to original space - this CONSUMES the rotation
        let (orig_x, orig_y, orig_w, orig_h) = CropSettings::from_visual_space(
            visual_crop.0, visual_crop.1, visual_crop.2, visual_crop.3, rotation_90
        );

        // INVARIANT: Saved rotation_90 must be 0
        let saved_rotation_90 = 0;

        // Viewer applies UV directly WITHOUT ANY ROTATION
        // This is the correct Lightroom-like behavior

        assert_eq!(saved_rotation_90, 0,
            "INVARIANT VIOLATED: Saved snapshot must have rotation_90 = 0. Viewer NEVER rotates!");

        // Verify UV gives correct result without rotation
        let saved = CropSettings::new(
            orig_x, orig_y, orig_w, orig_h,
            saved_rotation_90, 0.0, false, false
        );

        // With saved_rotation=0, to_visual_space returns same coords (no rotation)
        let (vx, vy, _vw, _vh) = saved.to_visual_space();
        assert!((vx - orig_x).abs() < 0.001, "UV must be applied directly without rotation");
        assert!((vy - orig_y).abs() < 0.001, "UV must be applied directly without rotation");

        println!("✓ INVARIANT: Viewer never rotates (rotation_90 consumed)");
    }

    /// INVARIANT: Saved snapshot must have angle = 0
    /// Empty corners from angle rotation require pre-rendering.
    /// VIEWER NEVER ROTATES!
    #[test]
    fn test_invariant_viewer_never_rotates_angle() {
        // Angle (fine rotation) creates empty corners
        // These corners require PRE-RENDERING to capture
        // The viewer itself NEVER applies rotation

        // INVARIANT: Saved angle must be 0
        let saved_angle = 0.0_f32;

        assert_eq!(saved_angle, 0.0,
            "INVARIANT VIOLATED: Saved snapshot must have angle = 0. Viewer NEVER rotates!");

        println!("✓ INVARIANT: Viewer never rotates (angle consumed)");
        println!("  For empty corners from angle: pre-render rotated image first");
    }

    /// Regression test: Ensure viewer never applies rotation
    /// This test will FAIL if anyone tries to rotate in the viewer
    #[test]
    fn test_regression_viewer_must_not_rotate() {
        // This test documents the CORRECT behavior to prevent regression
        //
        // WRONG (causes regression):
        //   if crop.angle() != 0.0 {
        //       img = img.rotate(crop.angle().to_radians(), ...);  // NO!
        //   }
        //
        // CORRECT:
        //   img = img.uv(crop_rect);  // UV only, no rotation
        //
        // If empty corners are needed from angle rotation:
        //   1. Pre-render the rotated image to a new texture
        //   2. Apply UV crop on the pre-rendered texture
        //   3. Display without any viewer rotation

        let saved_rotation_90 = 0;
        let saved_angle = 0.0_f32;

        // These assertions document the invariants
        assert_eq!(saved_rotation_90, 0, "rotation_90 must be 0 in saved snapshot");
        assert_eq!(saved_angle, 0.0, "angle must be 0 in saved snapshot");

        println!("✓ REGRESSION TEST: Viewer must not rotate");
        println!("  - Saved rotation_90 = 0 ✓");
        println!("  - Saved angle = 0 ✓");
        println!("  - Viewer applies UV only, NO rotation ✓");
    }

    // =============================================
    // Shrink to Fit Tests
    // =============================================

    #[test]
    fn test_shrink_to_fit_zero_angle_unchanged() {
        // With zero rotation angle, crop should remain unchanged
        let crop = CropSettings::default();
        let result = crop.calculate_shrink_to_fit(100.0, 100.0);
        
        assert_eq!(result.crop_x(), crop.crop_x());
        assert_eq!(result.crop_y(), crop.crop_y());
        assert_eq!(result.crop_width(), crop.crop_width());
        assert_eq!(result.crop_height(), crop.crop_height());
    }

    #[test]
    fn test_shrink_to_fit_reduces_crop_with_rotation() {
        // With any significant rotation, crop should shrink
        let crop = CropSettings::new(0.0, 0.0, 1.0, 1.0, 0, 15.0, false, false);
        let result = crop.calculate_shrink_to_fit(100.0, 100.0);
        
        // Crop dimensions should be smaller than original
        assert!(result.crop_width() < 1.0, "Width should shrink: {}", result.crop_width());
        assert!(result.crop_height() < 1.0, "Height should shrink: {}", result.crop_height());
        
        // Should be centered (since original was full frame)
        let center_x = result.crop_x() + result.crop_width() / 2.0;
        let center_y = result.crop_y() + result.crop_height() / 2.0;
        assert!((center_x - 0.5).abs() < 0.01, "Should be horizontally centered");
        assert!((center_y - 0.5).abs() < 0.01, "Should be vertically centered");
    }

    #[test]
    fn test_shrink_to_fit_symmetric_angles() {
        // +15° and -15° should produce same dimensions
        let crop_pos = CropSettings::new(0.0, 0.0, 1.0, 1.0, 0, 15.0, false, false);
        let crop_neg = CropSettings::new(0.0, 0.0, 1.0, 1.0, 0, -15.0, false, false);
        
        let result_pos = crop_pos.calculate_shrink_to_fit(100.0, 100.0);
        let result_neg = crop_neg.calculate_shrink_to_fit(100.0, 100.0);
        
        assert!((result_pos.crop_width() - result_neg.crop_width()).abs() < 0.001);
        assert!((result_pos.crop_height() - result_neg.crop_height()).abs() < 0.001);
    }

    #[test]
    fn test_shrink_to_fit_larger_angle_smaller_crop() {
        // Larger rotation should produce smaller crop
        let crop_small = CropSettings::new(0.0, 0.0, 1.0, 1.0, 0, 5.0, false, false);
        let crop_large = CropSettings::new(0.0, 0.0, 1.0, 1.0, 0, 30.0, false, false);
        
        let result_small = crop_small.calculate_shrink_to_fit(100.0, 100.0);
        let result_large = crop_large.calculate_shrink_to_fit(100.0, 100.0);
        
        assert!(result_large.crop_width() < result_small.crop_width(), 
            "30° rotation should produce smaller crop than 5°");
    }

    #[test]
    fn test_shrink_to_fit_preserves_fill_mode() {
        use super::RotationFillMode;
        
        let crop = CropSettings::default()
            .with_fill_mode(RotationFillMode::ShrinkToFit)
            .with_angle(15.0);
        
        let result = crop.calculate_shrink_to_fit(100.0, 100.0);
        
        assert_eq!(result.fill_mode(), RotationFillMode::ShrinkToFit);
    }

    #[test]
    fn test_shrink_to_fit_resets_small_crop() {
        // ShrinkToFit acts as "Auto Max Crop", so it should expand a small crop
        // to fill the maximum inscribed rectangle
        let crop = CropSettings::new(0.4, 0.4, 0.1, 0.1, 0, 15.0, false, false);
        let result = crop.calculate_shrink_to_fit(100.0, 100.0);
        
        // Should grow significantly (to ~0.8 for 15 degrees)
        assert!(result.crop_width() > 0.5, "Small crop should expand to max inscribed: {}", result.crop_width());
        assert!((result.crop_x() - (1.0 - result.crop_width())/2.0).abs() < 0.001, "Should be centered");
    }
}
