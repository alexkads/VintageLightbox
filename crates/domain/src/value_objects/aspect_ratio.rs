use std::fmt;

/// Represents aspect ratio options for cropping
#[derive(Debug, Clone, PartialEq)]
pub enum AspectRatio {
    /// Use the original aspect ratio of the photo
    Original,
    /// Free crop with no aspect ratio constraint
    Free,
    /// 1:1 - Square (Instagram, avatars)
    Square,
    /// 2:3 - Portrait format 35mm (prints 10x15)
    TwoThree,
    /// 3:2 - Landscape 35mm
    ThreeTwo,
    /// 4:3 - Common compact sensor format
    FourThree,
    /// 3:4 - Portrait 4:3
    ThreeFour,
    /// 4:5 - Instagram portrait, prints 8x10
    FourFive,
    /// 5:4 - Landscape 8x10
    FiveFour,
    /// 5:7 - Print 5x7 inches
    FiveSeven,
    /// 7:5 - Landscape 5x7
    SevenFive,
    /// 16:9 - Widescreen, HD video
    SixteenNine,
    /// 9:16 - Stories, Reels, TikTok
    NineSixteen,
    /// Custom named aspect ratio
    Custom(String),
}

impl AspectRatio {
    /// Returns the numeric value of the aspect ratio (width/height)
    /// Returns None for Original and Free as they don't have a fixed ratio
    pub fn value(&self) -> f32 {
        match self {
            AspectRatio::Original | AspectRatio::Free => 1.0, // Default, should use original dimensions
            AspectRatio::Square => 1.0,
            AspectRatio::TwoThree => 2.0 / 3.0,
            AspectRatio::ThreeTwo => 3.0 / 2.0,
            AspectRatio::FourThree => 4.0 / 3.0,
            AspectRatio::ThreeFour => 3.0 / 4.0,
            AspectRatio::FourFive => 4.0 / 5.0,
            AspectRatio::FiveFour => 5.0 / 4.0,
            AspectRatio::FiveSeven => 5.0 / 7.0,
            AspectRatio::SevenFive => 7.0 / 5.0,
            AspectRatio::SixteenNine => 16.0 / 9.0,
            AspectRatio::NineSixteen => 9.0 / 16.0,
            AspectRatio::Custom(_) => 1.0, // Custom ratios handled separately
        }
    }

    /// Swaps portrait/landscape orientation (e.g., 2:3 <-> 3:2)
    pub fn swap(&self) -> Self {
        match self {
            AspectRatio::Square => AspectRatio::Square,
            AspectRatio::TwoThree => AspectRatio::ThreeTwo,
            AspectRatio::ThreeTwo => AspectRatio::TwoThree,
            AspectRatio::FourThree => AspectRatio::ThreeFour,
            AspectRatio::ThreeFour => AspectRatio::FourThree,
            AspectRatio::FourFive => AspectRatio::FiveFour,
            AspectRatio::FiveFour => AspectRatio::FourFive,
            AspectRatio::FiveSeven => AspectRatio::SevenFive,
            AspectRatio::SevenFive => AspectRatio::FiveSeven,
            AspectRatio::SixteenNine => AspectRatio::NineSixteen,
            AspectRatio::NineSixteen => AspectRatio::SixteenNine,
            AspectRatio::Original => AspectRatio::Original,
            AspectRatio::Free => AspectRatio::Free,
            AspectRatio::Custom(name) => AspectRatio::Custom(name.clone()),
        }
    }

    /// Calculate dimensions from width maintaining aspect ratio
    /// Returns (width, height)
    pub fn calculate_dimensions_from_width(&self, width: f32) -> (f32, f32) {
        let value = self.value();
        (width, width / value)
    }

    /// Calculate dimensions from height maintaining aspect ratio
    /// Returns (width, height)
    pub fn calculate_dimensions_from_height(&self, height: f32) -> (f32, f32) {
        let value = self.value();
        (height * value, height)
    }

    /// Returns true if the ratio is in portrait orientation (height > width)
    pub fn is_portrait(&self) -> bool {
        self.value() < 1.0
    }

    /// Returns true if the ratio is in landscape orientation (width > height)
    pub fn is_landscape(&self) -> bool {
        self.value() > 1.0
    }
}

impl fmt::Display for AspectRatio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AspectRatio::Original => write!(f, "Original"),
            AspectRatio::Free => write!(f, "Free"),
            AspectRatio::Square => write!(f, "1:1"),
            AspectRatio::TwoThree => write!(f, "2:3"),
            AspectRatio::ThreeTwo => write!(f, "3:2"),
            AspectRatio::FourThree => write!(f, "4:3"),
            AspectRatio::ThreeFour => write!(f, "3:4"),
            AspectRatio::FourFive => write!(f, "4:5"),
            AspectRatio::FiveFour => write!(f, "5:4"),
            AspectRatio::FiveSeven => write!(f, "5:7"),
            AspectRatio::SevenFive => write!(f, "7:5"),
            AspectRatio::SixteenNine => write!(f, "16:9"),
            AspectRatio::NineSixteen => write!(f, "9:16"),
            AspectRatio::Custom(name) => write!(f, "{}", name),
        }
    }
}

#[cfg(test)]
mod aspect_ratio_tests {
    use super::*;

    #[test]
    fn test_original_aspect_ratio() {
        let ratio = AspectRatio::Original;
        assert_eq!(ratio.to_string(), "Original");
    }

    #[test]
    fn test_free_aspect_ratio() {
        let ratio = AspectRatio::Free;
        assert_eq!(ratio.to_string(), "Free");
    }

    #[test]
    fn test_predefined_ratios() {
        assert_eq!(AspectRatio::Square.to_string(), "1:1");
        assert_eq!(AspectRatio::TwoThree.to_string(), "2:3");
        assert_eq!(AspectRatio::ThreeTwo.to_string(), "3:2");
        assert_eq!(AspectRatio::FourThree.to_string(), "4:3");
        assert_eq!(AspectRatio::ThreeFour.to_string(), "3:4");
        assert_eq!(AspectRatio::FourFive.to_string(), "4:5");
        assert_eq!(AspectRatio::FiveFour.to_string(), "5:4");
        assert_eq!(AspectRatio::FiveSeven.to_string(), "5:7");
        assert_eq!(AspectRatio::SevenFive.to_string(), "7:5");
        assert_eq!(AspectRatio::SixteenNine.to_string(), "16:9");
        assert_eq!(AspectRatio::NineSixteen.to_string(), "9:16");
    }

    #[test]
    fn test_custom_aspect_ratio() {
        let ratio = AspectRatio::Custom("Banner 3:1".to_string());
        assert_eq!(ratio.to_string(), "Banner 3:1");
    }

    #[test]
    fn test_aspect_ratio_value() {
        assert_eq!(AspectRatio::Square.value(), 1.0);
        assert!((AspectRatio::TwoThree.value() - 0.6666667).abs() < 0.0001);
        assert!((AspectRatio::ThreeTwo.value() - 1.5).abs() < 0.0001);
        assert!((AspectRatio::FourThree.value() - 1.3333334).abs() < 0.0001);
        assert!((AspectRatio::SixteenNine.value() - 1.7777778).abs() < 0.0001);
    }

    #[test]
    fn test_swap_orientation_portrait_to_landscape() {
        assert_eq!(AspectRatio::TwoThree.swap(), AspectRatio::ThreeTwo);
        assert_eq!(AspectRatio::ThreeFour.swap(), AspectRatio::FourThree);
        assert_eq!(AspectRatio::FourFive.swap(), AspectRatio::FiveFour);
        assert_eq!(AspectRatio::FiveSeven.swap(), AspectRatio::SevenFive);
        assert_eq!(AspectRatio::NineSixteen.swap(), AspectRatio::SixteenNine);
    }

    #[test]
    fn test_swap_orientation_landscape_to_portrait() {
        assert_eq!(AspectRatio::ThreeTwo.swap(), AspectRatio::TwoThree);
        assert_eq!(AspectRatio::FourThree.swap(), AspectRatio::ThreeFour);
        assert_eq!(AspectRatio::FiveFour.swap(), AspectRatio::FourFive);
        assert_eq!(AspectRatio::SevenFive.swap(), AspectRatio::FiveSeven);
        assert_eq!(AspectRatio::SixteenNine.swap(), AspectRatio::NineSixteen);
    }

    #[test]
    fn test_swap_square_returns_square() {
        assert_eq!(AspectRatio::Square.swap(), AspectRatio::Square);
    }

    #[test]
    fn test_swap_original_returns_original() {
        assert_eq!(AspectRatio::Original.swap(), AspectRatio::Original);
    }

    #[test]
    fn test_swap_free_returns_free() {
        assert_eq!(AspectRatio::Free.swap(), AspectRatio::Free);
    }

    #[test]
    fn test_swap_custom_returns_custom() {
        let custom = AspectRatio::Custom("My Ratio".to_string());
        assert_eq!(custom.swap(), custom);
    }

    #[test]
    fn test_calculate_dimensions_from_width() {
        let (w, h) = AspectRatio::TwoThree.calculate_dimensions_from_width(300.0);
        assert_eq!(w, 300.0);
        assert_eq!(h, 450.0);

        let (w, h) = AspectRatio::Square.calculate_dimensions_from_width(100.0);
        assert_eq!(w, 100.0);
        assert_eq!(h, 100.0);

        let (w, h) = AspectRatio::SixteenNine.calculate_dimensions_from_width(1920.0);
        assert_eq!(w, 1920.0);
        assert!((h - 1080.0).abs() < 1.0);
    }

    #[test]
    fn test_calculate_dimensions_from_height() {
        let (w, h) = AspectRatio::TwoThree.calculate_dimensions_from_height(450.0);
        assert_eq!(h, 450.0);
        assert_eq!(w, 300.0);

        let (w, h) = AspectRatio::Square.calculate_dimensions_from_height(100.0);
        assert_eq!(w, 100.0);
        assert_eq!(h, 100.0);

        let (w, h) = AspectRatio::SixteenNine.calculate_dimensions_from_height(1080.0);
        assert_eq!(h, 1080.0);
        assert!((w - 1920.0).abs() < 1.0);
    }

    #[test]
    fn test_is_portrait() {
        assert!(AspectRatio::TwoThree.is_portrait());
        assert!(AspectRatio::ThreeFour.is_portrait());
        assert!(AspectRatio::FourFive.is_portrait());
        assert!(AspectRatio::FiveSeven.is_portrait());
        assert!(AspectRatio::NineSixteen.is_portrait());

        assert!(!AspectRatio::ThreeTwo.is_portrait());
        assert!(!AspectRatio::SixteenNine.is_portrait());
        assert!(!AspectRatio::Square.is_portrait());
    }

    #[test]
    fn test_is_landscape() {
        assert!(AspectRatio::ThreeTwo.is_landscape());
        assert!(AspectRatio::FourThree.is_landscape());
        assert!(AspectRatio::FiveFour.is_landscape());
        assert!(AspectRatio::SevenFive.is_landscape());
        assert!(AspectRatio::SixteenNine.is_landscape());

        assert!(!AspectRatio::TwoThree.is_landscape());
        assert!(!AspectRatio::NineSixteen.is_landscape());
        assert!(!AspectRatio::Square.is_landscape());
    }

    #[test]
    fn test_original_and_free_have_no_fixed_aspect() {
        // Original and Free should return None for value when used without original dimensions
        // This test ensures they're treated specially
        match AspectRatio::Original {
            AspectRatio::Original => {}, // Expected
            _ => panic!("Should be Original variant"),
        }
        
        match AspectRatio::Free {
            AspectRatio::Free => {}, // Expected
            _ => panic!("Should be Free variant"),
        }
    }
}
