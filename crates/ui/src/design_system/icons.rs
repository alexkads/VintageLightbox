// VintageLightbox Icon System
// Phosphor Icons integration for modern, professional UI
//
// Usage: Use icon constants directly in RichText or Button labels
//   Example: ui.button(RichText::new(icons::STAR_FILL).size(16.0))

// Re-export the regular variant for direct use
pub use egui_phosphor::regular::*;

// ============================================
// NAVIGATION ICONS
// ============================================
pub const NAV_LIBRARY: &str = egui_phosphor::regular::IMAGES;
pub const NAV_DEVELOP: &str = egui_phosphor::regular::SLIDERS;
pub const NAV_MAP: &str = egui_phosphor::regular::MAP_PIN;
pub const NAV_BOOK: &str = egui_phosphor::regular::BOOK_OPEN;
pub const NAV_PRINT: &str = egui_phosphor::regular::PRINTER;

// ============================================
// ACTION ICONS
// ============================================
pub const ACTION_IMPORT: &str = egui_phosphor::regular::DOWNLOAD_SIMPLE;
pub const ACTION_EXPORT: &str = egui_phosphor::regular::EXPORT;
pub const ACTION_TRASH: &str = egui_phosphor::regular::TRASH;
pub const ACTION_UNDO: &str = egui_phosphor::regular::ARROW_U_UP_LEFT;
pub const ACTION_REDO: &str = egui_phosphor::regular::ARROW_U_UP_RIGHT;
pub const ACTION_RESET: &str = egui_phosphor::regular::ARROW_COUNTER_CLOCKWISE;
pub const ACTION_SAVE: &str = egui_phosphor::regular::FLOPPY_DISK;
pub const ACTION_COPY: &str = egui_phosphor::regular::COPY;
pub const ACTION_PASTE: &str = egui_phosphor::regular::CLIPBOARD;
pub const ACTION_SEARCH: &str = egui_phosphor::regular::MAGNIFYING_GLASS;
pub const ACTION_FILTER: &str = egui_phosphor::regular::FUNNEL;
pub const ACTION_SORT: &str = egui_phosphor::regular::SORT_ASCENDING;
pub const ACTION_SETTINGS: &str = egui_phosphor::regular::GEAR;
pub const ACTION_INFO: &str = egui_phosphor::regular::INFO;
pub const ACTION_HELP: &str = egui_phosphor::regular::QUESTION;

// ============================================
// EDITING ICONS
// ============================================
pub const EDIT_CROP: &str = egui_phosphor::regular::CROP;
pub const EDIT_ROTATE_LEFT: &str = egui_phosphor::regular::ARROW_COUNTER_CLOCKWISE;
pub const EDIT_ROTATE_RIGHT: &str = egui_phosphor::regular::ARROW_CLOCKWISE;
pub const EDIT_FLIP_HORIZONTAL: &str = egui_phosphor::regular::ARROWS_HORIZONTAL;
pub const EDIT_FLIP_VERTICAL: &str = egui_phosphor::regular::ARROWS_VERTICAL;
pub const EDIT_BRUSH: &str = egui_phosphor::regular::PAINT_BRUSH;
pub const EDIT_ERASER: &str = egui_phosphor::regular::ERASER;
pub const EDIT_HEALING: &str = egui_phosphor::regular::BANDAIDS;
pub const EDIT_ADJUSTMENT: &str = egui_phosphor::regular::SLIDERS_HORIZONTAL;

// ============================================
// RATING ICONS
// ============================================
pub const STAR_EMPTY: &str = egui_phosphor::regular::STAR;
pub const STAR_FILLED: &str = egui_phosphor::regular::STAR;  // Note: v0.9 doesn't have STAR_FILL, use STAR with color

// ============================================
// FLAG ICONS
// ============================================
pub const FLAG_PICK: &str = egui_phosphor::regular::CHECK_CIRCLE;  // Pick flag (checkmark)
pub const FLAG_REJECT: &str = egui_phosphor::regular::X_CIRCLE;
pub const FLAG_UNFLAGGED: &str = egui_phosphor::regular::FLAG_BANNER;

// ============================================
// VIEW ICONS
// ============================================
pub const VIEW_GRID: &str = egui_phosphor::regular::GRID_FOUR;
pub const VIEW_LIST: &str = egui_phosphor::regular::LIST;
pub const VIEW_DETAIL: &str = egui_phosphor::regular::SQUARE;
pub const VIEW_COMPARE: &str = egui_phosphor::regular::COLUMNS;
pub const VIEW_LOUPE: &str = egui_phosphor::regular::MAGNIFYING_GLASS_PLUS;
pub const VIEW_FULL_SCREEN: &str = egui_phosphor::regular::ARROWS_OUT;
pub const VIEW_EXIT_FULL_SCREEN: &str = egui_phosphor::regular::ARROWS_IN;

// ============================================
// MEDIA CONTROL ICONS
// ============================================
pub const MEDIA_PLAY: &str = egui_phosphor::regular::PLAY;
pub const MEDIA_PAUSE: &str = egui_phosphor::regular::PAUSE;
pub const MEDIA_STOP: &str = egui_phosphor::regular::STOP;
pub const MEDIA_PREVIOUS: &str = egui_phosphor::regular::CARET_LEFT;
pub const MEDIA_NEXT: &str = egui_phosphor::regular::CARET_RIGHT;
pub const MEDIA_FIRST: &str = egui_phosphor::regular::CARET_DOUBLE_LEFT;
pub const MEDIA_LAST: &str = egui_phosphor::regular::CARET_DOUBLE_RIGHT;

// ============================================
// FILE/FOLDER ICONS
// ============================================
pub const FILE_IMAGE: &str = egui_phosphor::regular::IMAGE;
pub const FILE_FOLDER: &str = egui_phosphor::regular::FOLDER;
pub const FILE_FOLDER_OPEN: &str = egui_phosphor::regular::FOLDER_OPEN;
pub const FILE_ADD: &str = egui_phosphor::regular::PLUS;

// ============================================
// UI CONTROL ICONS
// ============================================
pub const CHEVRON_DOWN: &str = egui_phosphor::regular::CARET_DOWN;
pub const CHEVRON_UP: &str = egui_phosphor::regular::CARET_UP;
pub const CHEVRON_LEFT: &str = egui_phosphor::regular::CARET_LEFT;
pub const CHEVRON_RIGHT: &str = egui_phosphor::regular::CARET_RIGHT;
pub const CLOSE: &str = egui_phosphor::regular::X;
pub const CHECK: &str = egui_phosphor::regular::CHECK;
pub const PLUS: &str = egui_phosphor::regular::PLUS;
pub const MINUS: &str = egui_phosphor::regular::MINUS;
pub const MORE_HORIZONTAL: &str = egui_phosphor::regular::DOTS_THREE;
pub const MORE_VERTICAL: &str = egui_phosphor::regular::DOTS_THREE_VERTICAL;
