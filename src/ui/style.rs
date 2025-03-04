#![allow(unused)]
//! # Theme and Styling System
//! 
//! This module provides a consistent styling system for the application UI.
//! 
//! ## Theme API
//! 
//! Access theme colors using the get_color function:
//! 
//! ```rust
//! use crate::ui::style;
//! 
//! // Get colors by their name in the Ableton theme file
//! let background = style::get_color("Desktop");
//! let text = style::get_color("ControlForeground");
//! let accent = style::get_color("ChosenDefault");
//! ```
//! 
//! ## Reusable Style Components
//! 
//! This module provides several reusable style components:
//! 
//! - `AbletonCardStyle` - For card containers
//! - `AbletonHeaderStyle` - For header containers
//! - `AbletonRowStyle1` - For primary row styling
//! - `AbletonRowStyle2` - For alternate row styling
//! - `AbletonListRowStyle` - For list rows
//! - `AbletonDividerStyle` - For dividers
//! - `AbletonPanelStyle` - For panels
//! - `AbletonBackgroundStyle` - For backgrounds
//! - `AbletonTextInputStyle` - For text inputs
//! - `custom_scrollbar_style()` - For scrollbars

use iced::{
    widget::{container, text_input},
    Color, Theme,
};
use iced::widget::container::Appearance;

use crate::ui::theme_loader::get_current_theme;

/// Get a color from the current theme by its name in the Ableton theme file
/// 
/// # Arguments
/// 
/// * `name` - The name of the color as defined in the Ableton theme file
/// 
/// # Returns
/// 
/// * `Color` - The color if found, or a default color if not found
/// 
/// # Examples
/// 
/// ```
/// use crate::ui::style;
/// 
/// let background = style::get_color("Desktop");
/// let text = style::get_color("ControlForeground");
/// ```
pub fn get_color(name: &str) -> Color {
    get_current_theme().get_color(name)
}

/// Get the current theme
pub fn get_theme() -> Theme {
    iced::Theme::Dark
}

/// Card style for containers
pub struct AbletonCardStyle;

impl iced::widget::container::StyleSheet for AbletonCardStyle {
    type Style = Theme;

    fn appearance(&self, _style: &Self::Style) -> Appearance {
        Appearance {
            text_color: Some(get_color("ControlForeground")),
            background: Some(iced::Background::Color(get_color("ControlBackground"))),
            border_radius: 4.0.into(),
            border_width: 1.0,
            border_color: get_color("ControlContrastFrame"),
        }
    }
}

impl From<AbletonCardStyle> for iced::theme::Container {
    fn from(_: AbletonCardStyle) -> Self {
        iced::theme::Container::Custom(Box::new(AbletonCardStyle))
    }
}

/// Header style for containers
pub struct AbletonHeaderStyle;

impl iced::widget::container::StyleSheet for AbletonHeaderStyle {
    type Style = Theme;

    fn appearance(&self, _style: &Self::Style) -> Appearance {
        Appearance {
            text_color: Some(get_color("ControlForeground")),
            background: Some(iced::Background::Color(get_color("SurfaceHighlight"))),
            border_radius: 0.0.into(),
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        }
    }
}

impl From<AbletonHeaderStyle> for iced::theme::Container {
    fn from(_: AbletonHeaderStyle) -> Self {
        iced::theme::Container::Custom(Box::new(AbletonHeaderStyle))
    }
}

/// Primary row style for containers
pub struct AbletonRowStyle1;

impl iced::widget::container::StyleSheet for AbletonRowStyle1 {
    type Style = Theme;

    fn appearance(&self, _style: &Self::Style) -> Appearance {
        Appearance {
            text_color: Some(get_color("ControlForeground")),
            background: Some(iced::Background::Color(get_color("SurfaceBackground"))),
            border_radius: 0.0.into(),
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        }
    }
}

impl From<AbletonRowStyle1> for iced::theme::Container {
    fn from(_: AbletonRowStyle1) -> Self {
        iced::theme::Container::Custom(Box::new(AbletonRowStyle1))
    }
}

impl iced::widget::button::StyleSheet for AbletonRowStyle1 {
    type Style = Theme;

    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            text_color: get_color("ControlForeground"),
            background: Some(iced::Background::Color(get_color("SurfaceBackground"))),
            border_radius: 0.0.into(),
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
            shadow_offset: iced::Vector::default(),
        }
    }

    fn hovered(&self, style: &Self::Style) -> iced::widget::button::Appearance {
        let active = self.active(style);
        iced::widget::button::Appearance {
            background: Some(iced::Background::Color(get_color("SelectionBackground"))),
            text_color: get_color("SelectionForeground"),
            ..active
        }
    }

    fn pressed(&self, style: &Self::Style) -> iced::widget::button::Appearance {
        let active = self.active(style);
        iced::widget::button::Appearance {
            background: Some(iced::Background::Color(get_color("SelectionBackground"))),
            text_color: get_color("SelectionForeground"),
            ..active
        }
    }
}

/// Alternate row style for containers
pub struct AbletonRowStyle2;

impl iced::widget::container::StyleSheet for AbletonRowStyle2 {
    type Style = Theme;

    fn appearance(&self, _style: &Self::Style) -> Appearance {
        Appearance {
            text_color: Some(get_color("ControlForeground")),
            background: Some(iced::Background::Color(get_color("SurfaceHighlight"))),
            border_radius: 0.0.into(),
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        }
    }
}

impl From<AbletonRowStyle2> for iced::theme::Container {
    fn from(_: AbletonRowStyle2) -> Self {
        iced::theme::Container::Custom(Box::new(AbletonRowStyle2))
    }
}

impl iced::widget::button::StyleSheet for AbletonRowStyle2 {
    type Style = Theme;

    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            text_color: get_color("ControlForeground"),
            background: Some(iced::Background::Color(get_color("SurfaceHighlight"))),
            border_radius: 0.0.into(),
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
            shadow_offset: iced::Vector::default(),
        }
    }

    fn hovered(&self, style: &Self::Style) -> iced::widget::button::Appearance {
        let active = self.active(style);
        iced::widget::button::Appearance {
            background: Some(iced::Background::Color(get_color("SelectionBackground"))),
            text_color: get_color("SelectionForeground"),
            ..active
        }
    }

    fn pressed(&self, style: &Self::Style) -> iced::widget::button::Appearance {
        let active = self.active(style);
        iced::widget::button::Appearance {
            background: Some(iced::Background::Color(get_color("SelectionBackground"))),
            text_color: get_color("SelectionForeground"),
            ..active
        }
    }
}

/// List row style for containers
pub struct AbletonListRowStyle;

impl iced::widget::container::StyleSheet for AbletonListRowStyle {
    type Style = Theme;

    fn appearance(&self, _style: &Self::Style) -> Appearance {
        Appearance {
            text_color: Some(get_color("ControlForeground")),
            background: Some(iced::Background::Color(get_color("SurfaceBackground"))),
            border_radius: 0.0.into(),
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        }
    }
}

impl From<AbletonListRowStyle> for iced::theme::Container {
    fn from(_: AbletonListRowStyle) -> Self {
        iced::theme::Container::Custom(Box::new(AbletonListRowStyle))
    }
}

/// Divider style for containers
pub struct AbletonDividerStyle;

impl iced::widget::container::StyleSheet for AbletonDividerStyle {
    type Style = Theme;

    fn appearance(&self, _style: &Self::Style) -> Appearance {
        Appearance {
            text_color: Some(get_color("ControlForeground")),
            background: Some(iced::Background::Color(get_color("ControlContrastFrame"))),
            border_radius: 0.0.into(),
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        }
    }
}

impl From<AbletonDividerStyle> for iced::theme::Container {
    fn from(_: AbletonDividerStyle) -> Self {
        iced::theme::Container::Custom(Box::new(AbletonDividerStyle))
    }
}

/// Panel style for containers
pub struct AbletonPanelStyle;

impl iced::widget::container::StyleSheet for AbletonPanelStyle {
    type Style = Theme;

    fn appearance(&self, _style: &Self::Style) -> Appearance {
        Appearance {
            text_color: Some(get_color("ControlForeground")),
            background: Some(iced::Background::Color(get_color("SurfaceBackground"))),
            border_radius: 0.0.into(),
            border_width: 1.0,
            border_color: get_color("ControlContrastFrame"),
        }
    }
}

impl From<AbletonPanelStyle> for iced::theme::Container {
    fn from(_: AbletonPanelStyle) -> Self {
        iced::theme::Container::Custom(Box::new(AbletonPanelStyle))
    }
}

/// Background style for containers
pub struct AbletonBackgroundStyle;

impl iced::widget::container::StyleSheet for AbletonBackgroundStyle {
    type Style = Theme;

    fn appearance(&self, _style: &Self::Style) -> Appearance {
        Appearance {
            text_color: Some(get_color("ControlForeground")),
            background: Some(iced::Background::Color(get_color("Desktop"))),
            border_radius: 0.0.into(),
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        }
    }
}

impl From<AbletonBackgroundStyle> for iced::theme::Container {
    fn from(_: AbletonBackgroundStyle) -> Self {
        iced::theme::Container::Custom(Box::new(AbletonBackgroundStyle))
    }
}

/// Create a custom scrollbar style
pub fn custom_scrollbar_style() -> iced::theme::Scrollable {
    iced::theme::Scrollable::Custom(Box::new(CustomScrollableStyle))
}

/// Custom scrollable style
struct CustomScrollableStyle;

impl iced::widget::scrollable::StyleSheet for CustomScrollableStyle {
    type Style = iced::Theme;

    fn active(&self, _style: &Self::Style) -> iced::widget::scrollable::Scrollbar {
        iced::widget::scrollable::Scrollbar {
            background: Some(iced::Background::Color(get_color("ScrollbarInnerTrack"))),
            border_radius: 0.0.into(),
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
            scroller: iced::widget::scrollable::Scroller {
                color: get_color("ScrollbarInnerHandle"),
                border_radius: 0.0.into(),
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
        let scrollbar = self.active(style);

        if is_mouse_over_scrollbar {
            iced::widget::scrollable::Scrollbar {
                background: Some(iced::Background::Color(get_color("ScrollbarInnerTrack"))),
                scroller: iced::widget::scrollable::Scroller {
                    color: get_color("ScrollbarInnerHandle"),
                    ..scrollbar.scroller
                },
                ..scrollbar
            }
        } else {
            scrollbar
        }
    }

    fn dragging(&self, style: &Self::Style) -> iced::widget::scrollable::Scrollbar {
        let scrollbar = self.active(style);

        iced::widget::scrollable::Scrollbar {
            scroller: iced::widget::scrollable::Scroller {
                color: get_color("ScrollbarInnerHandle"),
                ..scrollbar.scroller
            },
            ..scrollbar
        }
    }

    fn active_horizontal(&self, style: &Self::Style) -> iced::widget::scrollable::Scrollbar {
        self.active(style)
    }

    fn hovered_horizontal(
        &self,
        style: &Self::Style,
        is_mouse_over_scrollbar: bool,
    ) -> iced::widget::scrollable::Scrollbar {
        self.hovered(style, is_mouse_over_scrollbar)
    }

    fn dragging_horizontal(&self, style: &Self::Style) -> iced::widget::scrollable::Scrollbar {
        self.dragging(style)
    }
}

/// Text input style
pub struct AbletonTextInputStyle;

impl text_input::StyleSheet for AbletonTextInputStyle {
    type Style = iced::Theme;

    fn active(&self, _style: &Self::Style) -> text_input::Appearance {
        text_input::Appearance {
            background: iced::Background::Color(get_color("ControlBackground")),
            border_radius: 2.0.into(),
            border_width: 1.0,
            border_color: get_color("ControlContrastFrame"),
            icon_color: get_color("ControlForeground"),
        }
    }

    fn focused(&self, _style: &Self::Style) -> text_input::Appearance {
        text_input::Appearance {
            background: iced::Background::Color(get_color("ControlBackground")),
            border_radius: 2.0.into(),
            border_width: 1.0,
            border_color: get_color("ControlSelectionFrame"),
            icon_color: get_color("ControlForeground"),
        }
    }

    fn placeholder_color(&self, _style: &Self::Style) -> iced::Color {
        get_color("TextDisabled")
    }

    fn value_color(&self, _style: &Self::Style) -> iced::Color {
        get_color("ControlForeground")
    }

    fn selection_color(&self, _style: &Self::Style) -> iced::Color {
        get_color("SelectionBackground")
    }

    fn disabled_color(&self, _style: &Self::Style) -> iced::Color {
        get_color("TextDisabled")
    }

    fn disabled(&self, _style: &Self::Style) -> text_input::Appearance {
        let active = self.active(_style);
        let bg_color = get_color("ControlBackground");
        let border_color = get_color("ControlContrastFrame");

        text_input::Appearance {
            background: iced::Background::Color(Color::new(bg_color.r, bg_color.g, bg_color.b, 0.5)),
            border_color: Color::new(border_color.r, border_color.g, border_color.b, 0.5),
            ..active
        }
    }
}

impl From<AbletonTextInputStyle> for iced::theme::TextInput {
    fn from(_: AbletonTextInputStyle) -> Self {
        iced::theme::TextInput::Custom(Box::new(AbletonTextInputStyle))
    }
}

/// Dynamic style for containers that uses theme colors by name
pub struct AbletonDynamicStyle;

impl iced::widget::container::StyleSheet for AbletonDynamicStyle {
    type Style = Theme;

    fn appearance(&self, _style: &Self::Style) -> Appearance {
        Appearance {
            text_color: Some(get_color("ControlForeground")),
            background: Some(iced::Background::Color(get_color("SurfaceBackground"))),
            border_radius: 4.0.into(),
            border_width: 1.0,
            border_color: get_color("ControlContrastFrame"),
        }
    }
}

impl From<AbletonDynamicStyle> for iced::theme::Container {
    fn from(_: AbletonDynamicStyle) -> Self {
        iced::theme::Container::Custom(Box::new(AbletonDynamicStyle))
    }
} 