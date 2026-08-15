//! Hand-written ANSI escape sequence emission for backends that cannot use
//! crossterm (wasm32, where crossterm does not compile).
//!
//! The functions here mirror what the native `CrosstermBackend` asks crossterm
//! to emit — same modifier-diff logic, same color/underline state machine in
//! [`draw`], same trailing reset — so a wasm32 backend rendering through them
//! tracks the native backend's behavior. One deliberate divergence: named
//! colors use the classic SGR codes (`31`, `41`, …) rather than crossterm's
//! palette form (`38;5;9`); both select the same color in xterm.js.
//!
//! Everything in this module is pure over [`Write`], so it is compiled (and
//! unit-tested against pinned byte sequences) on native hosts under
//! `cfg(test)` as well — see the repo precedent in `helix-core/src/storage.rs`
//! and `helix-loader/src/grammar.rs` for wasm-only logic kept visible to
//! `cargo test --workspace`.

use std::io::{self, Write};

use crate::buffer::Cell;
use helix_view::graphics::{Color, Modifier, UnderlineStyle};

/// Writes the SGR parameters (no `\x1b[` prefix, no `m` suffix) selecting the
/// given color. `base` is 38 for foreground, 48 for background; the classic
/// palette is offset relative to 30/40.
fn write_color(out: &mut impl Write, color: Color, base: u8) -> io::Result<()> {
    let classic = base - 8; // 30 for fg, 40 for bg
    match color {
        Color::Reset => write!(out, "{}", classic + 9),
        Color::Black => write!(out, "{}", classic),
        Color::Red => write!(out, "{}", classic + 1),
        Color::Green => write!(out, "{}", classic + 2),
        Color::Yellow => write!(out, "{}", classic + 3),
        Color::Blue => write!(out, "{}", classic + 4),
        Color::Magenta => write!(out, "{}", classic + 5),
        Color::Cyan => write!(out, "{}", classic + 6),
        Color::LightGray => write!(out, "{}", classic + 7),
        Color::Gray => write!(out, "{}", classic + 60),
        Color::LightRed => write!(out, "{}", classic + 61),
        Color::LightGreen => write!(out, "{}", classic + 62),
        Color::LightYellow => write!(out, "{}", classic + 63),
        Color::LightBlue => write!(out, "{}", classic + 64),
        Color::LightMagenta => write!(out, "{}", classic + 65),
        Color::LightCyan => write!(out, "{}", classic + 66),
        Color::White => write!(out, "{}", classic + 67),
        Color::Indexed(i) => write!(out, "{};5;{}", base, i),
        Color::Rgb(r, g, b) => write!(out, "{};2;{};{};{}", base, r, g, b),
    }
}

/// Emits one SGR sequence setting both foreground and background, like the
/// native backend's `SetColors`.
fn set_colors(out: &mut impl Write, fg: Color, bg: Color) -> io::Result<()> {
    write!(out, "\x1b[")?;
    write_color(out, fg, 38)?;
    write!(out, ";")?;
    write_color(out, bg, 48)?;
    write!(out, "m")
}

/// `\x1b[58…m` underline color, colon-separated (see the native backend's
/// `SetUnderlineColor` for why colons rather than semicolons).
fn set_underline_color(out: &mut impl Write, color: Color) -> io::Result<()> {
    match color {
        Color::Reset => write!(out, "\x1b[59m"),
        Color::Rgb(r, g, b) => write!(out, "\x1b[58:2::{}:{}:{}m", r, g, b),
        Color::Indexed(i) => write!(out, "\x1b[58:5:{}m", i),
        classic => {
            // Map the named colors onto the first 16 palette entries.
            let index = match classic {
                Color::Black => 0,
                Color::Red => 1,
                Color::Green => 2,
                Color::Yellow => 3,
                Color::Blue => 4,
                Color::Magenta => 5,
                Color::Cyan => 6,
                Color::LightGray => 7,
                Color::Gray => 8,
                Color::LightRed => 9,
                Color::LightGreen => 10,
                Color::LightYellow => 11,
                Color::LightBlue => 12,
                Color::LightMagenta => 13,
                Color::LightCyan => 14,
                _ => 15, // Color::White
            };
            write!(out, "\x1b[58:5:{}m", index)
        }
    }
}

fn underline_style_param(style: UnderlineStyle) -> &'static str {
    match style {
        UnderlineStyle::Reset => "24",
        UnderlineStyle::Line => "4",
        UnderlineStyle::Curl => "4:3",
        UnderlineStyle::Dotted => "4:4",
        UnderlineStyle::Dashed => "4:5",
        UnderlineStyle::DoubleLine => "4:2",
    }
}

/// Absolute cursor positioning; `x`/`y` are 0-based cell coordinates.
pub fn move_to(out: &mut impl Write, x: u16, y: u16) -> io::Result<()> {
    // ANSI cursor positions are 1-based.
    write!(out, "\x1b[{};{}H", y + 1, x + 1)
}

/// Emits the attribute changes needed to go from modifier set `from` to `to`,
/// mirroring the native backend's `ModifierDiff::queue` (including re-applying
/// DIM after clearing BOLD, since both are reset by SGR 22).
fn write_modifier_diff(out: &mut impl Write, from: Modifier, to: Modifier) -> io::Result<()> {
    let removed = from - to;
    if removed.contains(Modifier::REVERSED) {
        write!(out, "\x1b[27m")?;
    }
    if removed.contains(Modifier::BOLD) {
        write!(out, "\x1b[22m")?;
        if to.contains(Modifier::DIM) {
            write!(out, "\x1b[2m")?;
        }
    }
    if removed.contains(Modifier::ITALIC) {
        write!(out, "\x1b[23m")?;
    }
    if removed.contains(Modifier::DIM) {
        write!(out, "\x1b[22m")?;
    }
    if removed.contains(Modifier::CROSSED_OUT) {
        write!(out, "\x1b[29m")?;
    }
    if removed.contains(Modifier::SLOW_BLINK) || removed.contains(Modifier::RAPID_BLINK) {
        write!(out, "\x1b[25m")?;
    }
    if removed.contains(Modifier::HIDDEN) {
        write!(out, "\x1b[28m")?;
    }

    let added = to - from;
    if added.contains(Modifier::REVERSED) {
        write!(out, "\x1b[7m")?;
    }
    if added.contains(Modifier::BOLD) {
        write!(out, "\x1b[1m")?;
    }
    if added.contains(Modifier::ITALIC) {
        write!(out, "\x1b[3m")?;
    }
    if added.contains(Modifier::DIM) {
        write!(out, "\x1b[2m")?;
    }
    if added.contains(Modifier::CROSSED_OUT) {
        write!(out, "\x1b[9m")?;
    }
    if added.contains(Modifier::SLOW_BLINK) {
        write!(out, "\x1b[5m")?;
    }
    if added.contains(Modifier::RAPID_BLINK) {
        write!(out, "\x1b[6m")?;
    }
    if added.contains(Modifier::HIDDEN) {
        write!(out, "\x1b[8m")?;
    }

    Ok(())
}

/// The cell-diff draw loop of the native `CrosstermBackend::draw`, emitting
/// escape sequences by hand: cursor moves are skipped for horizontally
/// contiguous cells, styles are diffed statefully, and a full underline/
/// color/attribute reset trails the content.
///
/// Unlike the native backend there is no terminfo capability check: xterm.js
/// supports extended underlines, so they are emitted unconditionally.
pub fn draw<'a, I>(out: &mut impl Write, content: I) -> io::Result<()>
where
    I: Iterator<Item = (u16, u16, &'a Cell)>,
{
    let mut fg = Color::Reset;
    let mut bg = Color::Reset;
    let mut underline_color = Color::Reset;
    let mut underline_style = UnderlineStyle::Reset;
    let mut modifier = Modifier::empty();
    let mut last_pos: Option<(u16, u16)> = None;
    for (x, y, cell) in content {
        // Move the cursor if the previous location was not (x - 1, y)
        if !matches!(last_pos, Some(p) if x == p.0 + 1 && y == p.1) {
            move_to(out, x, y)?;
        }
        last_pos = Some((x, y));
        if cell.modifier != modifier {
            write_modifier_diff(out, modifier, cell.modifier)?;
            modifier = cell.modifier;
        }
        if cell.fg != fg || cell.bg != bg {
            set_colors(out, cell.fg, cell.bg)?;
            fg = cell.fg;
            bg = cell.bg;
        }

        if cell.underline_color != underline_color {
            set_underline_color(out, cell.underline_color)?;
            underline_color = cell.underline_color;
        }

        if cell.underline_style != underline_style {
            write!(out, "\x1b[{}m", underline_style_param(cell.underline_style))?;
            underline_style = cell.underline_style;
        }

        out.write_all(cell.symbol.as_bytes())?;
    }

    set_underline_color(out, Color::Reset)?;
    write!(out, "\x1b[39m\x1b[49m\x1b[0m")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emitted(f: impl FnOnce(&mut Vec<u8>) -> io::Result<()>) -> String {
        let mut out = Vec::new();
        f(&mut out).unwrap();
        String::from_utf8(out).unwrap()
    }

    /// Renders a crossterm command to its ANSI bytes, for cross-checking the
    /// sequences that are byte-identical to what the native backend emits.
    #[cfg(feature = "crossterm")]
    fn crossterm_ansi(cmd: impl crossterm::Command) -> String {
        // A NO_COLOR env var would otherwise make crossterm emit nothing.
        crossterm::style::Colored::set_ansi_color_disabled(false);
        let mut s = String::new();
        cmd.write_ansi(&mut s).unwrap();
        s
    }

    #[test]
    fn classic_colors_use_classic_sgr_codes() {
        assert_eq!(
            emitted(|o| set_colors(o, Color::Red, Color::Blue)),
            "\x1b[31;44m"
        );
        assert_eq!(
            emitted(|o| set_colors(o, Color::Black, Color::White)),
            "\x1b[30;107m"
        );
        assert_eq!(
            emitted(|o| set_colors(o, Color::LightRed, Color::Gray)),
            "\x1b[91;100m"
        );
        // Reset selects the default fg/bg, not a palette entry.
        assert_eq!(
            emitted(|o| set_colors(o, Color::Reset, Color::Reset)),
            "\x1b[39;49m"
        );
    }

    #[test]
    fn indexed_and_rgb_colors_match_native_backend_bytes() {
        assert_eq!(
            emitted(|o| set_colors(o, Color::Indexed(123), Color::Rgb(1, 22, 255))),
            "\x1b[38;5;123;48;2;1;22;255m"
        );

        // crossterm emits the identical parameters for these forms; pin that.
        #[cfg(feature = "crossterm")]
        {
            use crossterm::style::{Color as CColor, SetBackgroundColor, SetForegroundColor};
            assert_eq!(
                crossterm_ansi(SetForegroundColor(CColor::AnsiValue(123))),
                "\x1b[38;5;123m"
            );
            assert_eq!(
                crossterm_ansi(SetBackgroundColor(CColor::Rgb {
                    r: 1,
                    g: 22,
                    b: 255
                })),
                "\x1b[48;2;1;22;255m"
            );
        }
    }

    #[test]
    fn underline_color_uses_colon_separated_form() {
        assert_eq!(
            emitted(|o| set_underline_color(o, Color::Reset)),
            "\x1b[59m"
        );
        assert_eq!(
            emitted(|o| set_underline_color(o, Color::Rgb(10, 20, 30))),
            "\x1b[58:2::10:20:30m"
        );
        assert_eq!(
            emitted(|o| set_underline_color(o, Color::Indexed(200))),
            "\x1b[58:5:200m"
        );
        // Named colors map onto the first 16 palette entries.
        assert_eq!(
            emitted(|o| set_underline_color(o, Color::Red)),
            "\x1b[58:5:1m"
        );
        assert_eq!(
            emitted(|o| set_underline_color(o, Color::White)),
            "\x1b[58:5:15m"
        );
    }

    #[test]
    fn move_to_is_one_based_row_column() {
        assert_eq!(emitted(|o| move_to(o, 0, 0)), "\x1b[1;1H");
        assert_eq!(emitted(|o| move_to(o, 7, 3)), "\x1b[4;8H");

        #[cfg(feature = "crossterm")]
        assert_eq!(
            emitted(|o| move_to(o, 7, 3)),
            crossterm_ansi(crossterm::cursor::MoveTo(7, 3))
        );
    }

    #[test]
    fn modifier_diff_adds_and_removes_attributes() {
        // Adding attributes emits their set codes in the native backend's order.
        assert_eq!(
            emitted(|o| write_modifier_diff(
                o,
                Modifier::empty(),
                Modifier::BOLD | Modifier::ITALIC | Modifier::REVERSED
            )),
            "\x1b[7m\x1b[1m\x1b[3m"
        );
        // Removing them emits the corresponding reset codes.
        assert_eq!(
            emitted(|o| write_modifier_diff(
                o,
                Modifier::BOLD | Modifier::ITALIC | Modifier::REVERSED,
                Modifier::empty()
            )),
            "\x1b[27m\x1b[22m\x1b[23m"
        );
        // No change emits nothing.
        assert_eq!(
            emitted(|o| write_modifier_diff(o, Modifier::BOLD, Modifier::BOLD)),
            ""
        );
    }

    #[test]
    fn modifier_diff_reapplies_dim_when_bold_is_cleared() {
        // SGR 22 clears both BOLD and DIM, so dropping BOLD while keeping DIM
        // must re-emit DIM — the exact case the native ModifierDiff handles.
        assert_eq!(
            emitted(|o| write_modifier_diff(o, Modifier::BOLD | Modifier::DIM, Modifier::DIM)),
            "\x1b[22m\x1b[2m"
        );
    }

    #[cfg(feature = "crossterm")]
    #[test]
    fn modifier_codes_match_crossterm_attributes() {
        use crossterm::style::{Attribute, SetAttribute};
        for (ours, native) in [
            ("\x1b[7m", Attribute::Reverse),
            ("\x1b[27m", Attribute::NoReverse),
            ("\x1b[1m", Attribute::Bold),
            ("\x1b[22m", Attribute::NormalIntensity),
            ("\x1b[3m", Attribute::Italic),
            ("\x1b[23m", Attribute::NoItalic),
            ("\x1b[2m", Attribute::Dim),
            ("\x1b[9m", Attribute::CrossedOut),
            ("\x1b[29m", Attribute::NotCrossedOut),
            ("\x1b[5m", Attribute::SlowBlink),
            ("\x1b[6m", Attribute::RapidBlink),
            ("\x1b[25m", Attribute::NoBlink),
            ("\x1b[8m", Attribute::Hidden),
            ("\x1b[28m", Attribute::NoHidden),
        ] {
            assert_eq!(ours, crossterm_ansi(SetAttribute(native)), "{:?}", native);
        }
    }

    fn cell(symbol: &str) -> Cell {
        let mut cell = Cell::default();
        cell.set_symbol(symbol);
        cell
    }

    #[test]
    fn draw_skips_cursor_moves_for_contiguous_cells() {
        let a = cell("a");
        let b = cell("b");
        let c = cell("c");
        let content = [(2u16, 0u16, &a), (3, 0, &b), (7, 1, &c)];
        let out = emitted(|o| draw(o, content.iter().copied()));
        // One move for the run starting at (2,0) — none between "a" and "b" —
        // then a move for (7,1), then the trailing reset.
        assert_eq!(out, "\x1b[1;3Hab\x1b[2;8Hc\x1b[59m\x1b[39m\x1b[49m\x1b[0m");
    }

    #[test]
    fn draw_diffs_styles_between_cells() {
        let plain = cell("a");
        let mut styled = cell("b");
        styled.set_fg(Color::Red).set_bg(Color::Reset);
        styled.modifier.insert(Modifier::BOLD);
        let mut underlined = cell("c");
        underlined.set_fg(Color::Red).set_bg(Color::Reset);
        underlined.modifier.insert(Modifier::BOLD);
        underlined.underline_color = Color::Indexed(4);
        underlined.underline_style = UnderlineStyle::Curl;

        let content = [(0u16, 0u16, &plain), (1, 0, &styled), (2, 0, &underlined)];
        let out = emitted(|o| draw(o, content.iter().copied()));
        assert_eq!(
            out,
            concat!(
                "\x1b[1;1Ha",                      // default state: no styling emitted
                "\x1b[1m\x1b[31;49mb",             // add BOLD, set fg (bg unchanged but paired)
                "\x1b[58:5:4m\x1b[4:3mc",          // underline color + curl style only
                "\x1b[59m\x1b[39m\x1b[49m\x1b[0m"  // trailing reset
            )
        );
    }

    #[test]
    fn draw_of_nothing_still_resets() {
        let out = emitted(|o| draw(o, std::iter::empty()));
        assert_eq!(out, "\x1b[59m\x1b[39m\x1b[49m\x1b[0m");
    }
}
