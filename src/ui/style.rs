#![allow(unused)]
use iced::{Background, Color, Theme, widget::container::Appearance};
use iced::widget::text_input::{self, StyleSheet};

fn color_from_hex(hex: &str) -> Color {
    let r = u8::from_str_radix(&hex[1..3], 16).unwrap();
    let g = u8::from_str_radix(&hex[3..5], 16).unwrap();
    let b = u8::from_str_radix(&hex[5..7], 16).unwrap();
    Color::from_rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}

// Ableton Live-inspired color palette
pub const BACKGROUND: Color = Color {
    r: 0.13,
    g: 0.13,
    b: 0.14,
    a: 1.0,
};

pub const SURFACE: Color = Color {
    r: 0.18,
    g: 0.18,
    b: 0.19,
    a: 1.0,
};

pub const CARD_BACKGROUND: Color = Color {
    r: 0.22,
    g: 0.22,
    b: 0.23,
    a: 1.0,
};

pub const ACCENT: Color = Color {
    r: 0.92,
    g: 0.65,
    b: 0.26,  // Ableton's orange accent
    a: 1.0,
};

pub const TEXT: Color = Color {
    r: 0.847,
    g: 0.847,
    b: 0.847,
    a: 1.0,
};

pub const TEXT_SECONDARY: Color = Color {
    r: 0.65,
    g: 0.65,
    b: 0.65,
    a: 1.0,
};

pub const PANEL_BORDER: Color = Color {
    r: 0.25,
    g: 0.25,
    b: 0.26,
    a: 1.0,
};

pub const ROW_ALT_1: Color = Color {
    r: 0.20,
    g: 0.20,
    b: 0.21,
    a: 1.0,
};

pub const ROW_ALT_2: Color = Color {
    r: 0.22,
    g: 0.22,
    b: 0.23,
    a: 1.0,
};

pub const HEADER_BACKGROUND: Color = Color {
    r: 0.24,
    g: 0.24,
    b: 0.25,
    a: 1.0,
};

pub const SUCCESS: Color = Color {
    r: 0.22,
    g: 0.70,
    b: 0.29,
    a: 1.0,
};

pub const ERROR: Color = Color {
    r: 0.90,
    g: 0.30,
    b: 0.30,
    a: 1.0,
};

// Helper function to get the application theme
pub fn get_theme() -> Theme {
    Theme::Dark
}

// Reusable styles for containers
pub struct AbletonCardStyle;

impl iced::widget::container::StyleSheet for AbletonCardStyle {
    type Style = Theme;

    fn appearance(&self, _style: &Self::Style) -> Appearance {
        Appearance {
            text_color: Some(TEXT),
            background: Some(Background::Color(CARD_BACKGROUND)),
            border_radius: 4.0.into(),
            border_width: 1.0,
            border_color: PANEL_BORDER,
        }
    }
}

pub struct AbletonHeaderStyle;

impl iced::widget::container::StyleSheet for AbletonHeaderStyle {
    type Style = Theme;

    fn appearance(&self, _style: &Self::Style) -> Appearance {
        Appearance {
            text_color: Some(TEXT),
            background: Some(Background::Color(HEADER_BACKGROUND)),
            border_radius: 4.0.into(),
            border_width: 1.0,
            border_color: PANEL_BORDER,
        }
    }
}

pub struct AbletonRowStyle1;

impl iced::widget::container::StyleSheet for AbletonRowStyle1 {
    type Style = Theme;

    fn appearance(&self, _style: &Self::Style) -> Appearance {
        Appearance {
            text_color: Some(TEXT),
            background: Some(Background::Color(ROW_ALT_1)),
            border_radius: 0.0.into(),
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        }
    }
}

pub struct AbletonRowStyle2;

impl iced::widget::container::StyleSheet for AbletonRowStyle2 {
    type Style = Theme;

    fn appearance(&self, _style: &Self::Style) -> Appearance {
        Appearance {
            text_color: Some(TEXT),
            background: Some(Background::Color(ROW_ALT_2)),
            border_radius: 0.0.into(),
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        }
    }
}

pub struct AbletonPanelStyle;

impl iced::widget::container::StyleSheet for AbletonPanelStyle {
    type Style = Theme;

    fn appearance(&self, _style: &Self::Style) -> Appearance {
        Appearance {
            text_color: Some(TEXT),
            background: Some(Background::Color(SURFACE)),
            border_radius: 4.0.into(),
            border_width: 1.0,
            border_color: PANEL_BORDER,
        }
    }
}

pub struct AbletonBackgroundStyle;

impl iced::widget::container::StyleSheet for AbletonBackgroundStyle {
    type Style = Theme;

    fn appearance(&self, _style: &Self::Style) -> Appearance {
        Appearance {
            text_color: Some(TEXT),
            background: Some(Background::Color(BACKGROUND)),
            border_radius: 0.0.into(),
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        }
    }
}

// Custom scrollbar styling
pub fn custom_scrollbar_style() -> iced::theme::Scrollable {
    iced::theme::Scrollable::custom(CustomScrollableStyle)
}

// Define scrollbar colors
const SCROLLBAR_BACKGROUND: Color = Color::TRANSPARENT; // Transparent background
const SCROLLBAR_COLOR: Color = Color { r: 0.55, g: 0.55, b: 0.55, a: 0.4 }; // Modern Chromium-style grey
const SCROLLBAR_HOVER_COLOR: Color = Color { r: 0.65, g: 0.65, b: 0.65, a: 0.6 }; // Slightly lighter grey when hovered
const SCROLLBAR_DRAG_COLOR: Color = Color { r: 0.75, g: 0.75, b: 0.75, a: 0.8 }; // Even lighter when dragging

// Custom scrollable style implementation
struct CustomScrollableStyle;

impl iced::widget::scrollable::StyleSheet for CustomScrollableStyle {
    type Style = iced::Theme;

    fn active(&self, _style: &Self::Style) -> iced::widget::scrollable::Scrollbar {
        iced::widget::scrollable::Scrollbar {
            background: Some(Background::Color(SCROLLBAR_BACKGROUND)),
            border_radius: 0.0.into(), // Flat background
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
            scroller: iced::widget::scrollable::Scroller {
                color: SCROLLBAR_COLOR,
                border_radius: 3.0.into(), // Pill shape (half of width)
                border_width: 0.0,
                border_color: Color::TRANSPARENT,
            },
        }
    }

    fn hovered(
        &self,
        style: &Self::Style,
        is_mouse_over_scrollbar: bool,
    ) -> iced::widget::scrollable::Scrollbar {
        let mut scrollbar = self.active(style);
        
        if is_mouse_over_scrollbar {
            scrollbar.scroller.color = SCROLLBAR_HOVER_COLOR;
        }
        
        scrollbar
    }

    fn dragging(&self, style: &Self::Style) -> iced::widget::scrollable::Scrollbar {
        let mut scrollbar = self.active(style);
        scrollbar.scroller.color = SCROLLBAR_DRAG_COLOR;
        scrollbar
    }
    
    // Override horizontal scrollbar styling to maintain consistent look
    fn active_horizontal(&self, style: &Self::Style) -> iced::widget::scrollable::Scrollbar {
        self.active(style) // Use the same style for horizontal scrollbars
    }
    
    fn hovered_horizontal(
        &self,
        style: &Self::Style,
        is_mouse_over_scrollbar: bool,
    ) -> iced::widget::scrollable::Scrollbar {
        self.hovered(style, is_mouse_over_scrollbar) // Use the same hover style for horizontal scrollbars
    }
    
    fn dragging_horizontal(&self, style: &Self::Style) -> iced::widget::scrollable::Scrollbar {
        self.dragging(style) // Use the same dragging style for horizontal scrollbars
    }
}

// Add this new style for text inputs
pub struct AbletonTextInputStyle;

impl text_input::StyleSheet for AbletonTextInputStyle {
    type Style = iced::Theme;

    fn active(&self, _style: &Self::Style) -> text_input::Appearance {
        text_input::Appearance {
            background: iced::Background::Color(iced::Color::from_rgb(0.2, 0.2, 0.2)),
            border_radius: 0.0.into(),
            border_width: 1.0,
            border_color: iced::Color::from_rgb(0.3, 0.3, 0.3),
            icon_color: iced::Color::from_rgb(0.7, 0.7, 0.7),
        }
    }

    fn focused(&self, _style: &Self::Style) -> text_input::Appearance {
        text_input::Appearance {
            background: iced::Background::Color(iced::Color::from_rgb(0.2, 0.2, 0.2)),
            border_radius: 0.0.into(),
            border_width: 1.0,
            border_color: iced::Color::from_rgb(0.5, 0.5, 0.5),
            icon_color: iced::Color::from_rgb(0.7, 0.7, 0.7),
        }
    }

    fn placeholder_color(&self, _style: &Self::Style) -> iced::Color {
        iced::Color::from_rgb(0.5, 0.5, 0.5)
    }

    fn value_color(&self, _style: &Self::Style) -> iced::Color {
        iced::Color::from_rgb(0.9, 0.9, 0.9)
    }

    fn selection_color(&self, _style: &Self::Style) -> iced::Color {
        iced::Color::from_rgb(0.3, 0.3, 0.7)
    }

    fn disabled_color(&self, _style: &Self::Style) -> iced::Color {
        iced::Color::from_rgb(0.4, 0.4, 0.4) // Dimmer text for disabled state
    }

    fn disabled(&self, _style: &Self::Style) -> text_input::Appearance {
        text_input::Appearance {
            background: iced::Background::Color(iced::Color::from_rgb(0.15, 0.15, 0.15)), // Darker background
            border_radius: 0.0.into(),
            border_width: 1.0,
            border_color: iced::Color::from_rgb(0.2, 0.2, 0.2), // Dimmer border
            icon_color: iced::Color::from_rgb(0.4, 0.4, 0.4), // Dimmer icon
        }
    }
} 