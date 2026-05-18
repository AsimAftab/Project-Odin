//! Shared color palette for Odin's mythology-themed UIs.
//!
//! Keeping these here means a future palette tweak ripples through every TUI
//! and command output at once.

use ratatui::style::Color;

/// Heimdall's gold — primary accent for titles, runes, and active markers.
pub const HEIM_GOLD: Color = Color::Rgb(255, 196, 87);

/// Cool blue used for body text highlights, profile names, key chips.
pub const RUNE_BLUE: Color = Color::Rgb(120, 175, 255);

/// Violet shimmer of the rainbow bridge — sync, github, gauges.
pub const BIFROST: Color = Color::Rgb(176, 137, 255);

/// Muted slate for borders, dimmed labels, and inactive markers.
pub const SHADOW: Color = Color::Rgb(110, 110, 130);

/// Soft green for the actively-bound profile, success states.
pub const ACTIVE: Color = Color::Rgb(120, 220, 150);

/// Selection background used in lists and tables.
pub const SELECTION_BG: Color = Color::Rgb(40, 40, 65);
