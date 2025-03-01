use iced::{Color, Theme};

// Define some common colors
pub const BACKGROUND: Color = Color {
    r: 0.15,
    g: 0.15,
    b: 0.15,
    a: 1.0,
};

pub const SURFACE: Color = Color {
    r: 0.2,
    g: 0.2,
    b: 0.2,
    a: 1.0,
};

pub const ACCENT: Color = Color {
    r: 0.4,
    g: 0.67,
    b: 0.97,
    a: 1.0,
};

pub const TEXT: Color = Color {
    r: 0.9,
    g: 0.9,
    b: 0.9,
    a: 1.0,
};

pub const TEXT_SECONDARY: Color = Color {
    r: 0.7,
    g: 0.7,
    b: 0.7,
    a: 1.0,
};

// Helper function to get the application theme
pub fn get_theme() -> Theme {
    Theme::Dark
} 