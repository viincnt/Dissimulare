use ratatui::style::{Color, Modifier, Style};

/// `#8b2fc9` — the violet used across the project's README badges.
pub const ACCENT: Color = Color::Rgb(0x8b, 0x2f, 0xc9);
/// A dimmed-down `ACCENT`, used for chrome (panel borders) that should hint
/// at the brand color without competing with `ACCENT` itself, which stays
/// reserved for interactive/selected elements.
pub const ACCENT_DIM: Color = Color::Rgb(0x5f, 0x3e, 0x70);
pub const SUCCESS: Color = Color::Rgb(152, 195, 121);
pub const ERR: Color = Color::Rgb(224, 108, 117);
pub const MUTED: Color = Color::Rgb(92, 99, 112);
pub const SUBTLE: Color = Color::Rgb(55, 62, 77);
pub const BRIGHT: Color = Color::Rgb(171, 178, 191);
pub const WHITE: Color = Color::Rgb(220, 223, 228);

pub fn accent() -> Style {
    Style::default().fg(ACCENT)
}
pub fn accent_dim() -> Style {
    Style::default().fg(ACCENT_DIM)
}
pub fn success() -> Style {
    Style::default().fg(SUCCESS)
}
pub fn err() -> Style {
    Style::default().fg(ERR)
}
pub fn muted() -> Style {
    Style::default().fg(MUTED)
}
pub fn subtle() -> Style {
    Style::default().fg(SUBTLE)
}
pub fn bright() -> Style {
    Style::default().fg(BRIGHT)
}
pub fn bold_white() -> Style {
    Style::default().fg(WHITE).add_modifier(Modifier::BOLD)
}
pub fn bold_accent() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}
pub fn bold_success() -> Style {
    Style::default().fg(SUCCESS).add_modifier(Modifier::BOLD)
}
pub fn bold_err() -> Style {
    Style::default().fg(ERR).add_modifier(Modifier::BOLD)
}
