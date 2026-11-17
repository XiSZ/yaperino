//! Fonts, colours and the small drawing helpers of the Chats window.

use crate::chat::ChatLine;
use crate::emotes::{self, Seg, Store};
use egui::{Color32, FontData, FontDefinitions, FontFamily, RichText, Sense, Stroke, Vec2};
use std::collections::VecDeque;
use std::sync::Arc;

/// Font families: the UI and chat use Segoe UI (regular, like Chatterino on Windows);
/// "bold" is Segoe UI Bold for user names; "list" is Arial Bold for the Chatty-style
/// Live Streams list (as in the person's picture). Windows' emoji and symbol fonts
/// fill in hearts and sparkles.
pub const BOLD: &str = "bold";
pub const LIST: &str = "list";

pub fn bold() -> FontFamily {
    FontFamily::Name(BOLD.into())
}
pub fn list_family() -> FontFamily {
    FontFamily::Name(LIST.into())
}

/// How the app looks (Settings > Customization), like chatterino7's / Moltorino's
/// appearance settings: a theme, the accent colour, the font and the zoom.
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Appearance {
    /// Dark, Black, Light or White (Moltorino's resources/themes).
    pub theme: String,
    /// Replaces the theme's accent colour (#00aeef) when set.
    pub accent: Option<[u8; 3]>,
    /// Segoe UI, Arial, Verdana, Tahoma, Calibri or Consolas.
    pub font: String,
    /// Size of everything (Chatterino's zoom), 0.8 .. 1.5.
    pub zoom: f32,
    /// Chatterino's "Show tab close button".
    pub tab_close_buttons: bool,
    /// Chatterino's own title bar (account, settings, window buttons) instead of Windows'.
    pub own_title_bar: bool,
    /// Where the tabs are: Top, Left, Right or Bottom (Chatterino's tab direction).
    pub tab_position: String,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            theme: "Dark".into(),
            accent: None,
            font: "Segoe UI".into(),
            zoom: 1.0,
            tab_close_buttons: true,
            own_title_bar: true,
            tab_position: "Top".into(),
        }
    }
}

pub const THEMES: [&str; 4] = ["Dark", "Black", "Light", "White"];
/// (name, regular file, bold file) in C:\Windows\Fonts.
pub const FONTS: [(&str, &str, &str); 6] = [
    ("Segoe UI", "segoeui.ttf", "segoeuib.ttf"),
    ("Arial", "arial.ttf", "arialbd.ttf"),
    ("Verdana", "verdana.ttf", "verdanab.ttf"),
    ("Tahoma", "tahoma.ttf", "tahomabd.ttf"),
    ("Calibri", "calibri.ttf", "calibrib.ttf"),
    ("Consolas", "consola.ttf", "consolab.ttf"),
];

/// Noto Emoji, black and white (SIL Open Font License 1.1, see
/// assets/fonts/OFL-NotoEmoji.txt), for macOS and Linux only; Windows has Segoe UI Emoji.
#[cfg(not(windows))]
const NOTO_EMOJI: &[u8] = include_bytes!("../assets/fonts/NotoEmoji.ttf");

/// Reads a font by its Windows file name (in C:\Windows\Fonts). On macOS and Linux the
/// closest font that comes with the system is used: Arial and Apple Symbols on macOS,
/// Liberation Sans (looks like Arial), DejaVu Sans and Noto Sans Symbols 2 on Linux.
fn read_system_font(file: &str) -> Option<Vec<u8>> {
    #[cfg(windows)]
    {
        let dir = std::env::var_os("WINDIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| "C:\\Windows".into())
            .join("Fonts");
        std::fs::read(dir.join(file)).ok()
    }
    #[cfg(not(windows))]
    {
        other_system_font_files(file)
            .iter()
            .find_map(|p| std::fs::read(p).ok())
    }
}

#[cfg(target_os = "macos")]
fn other_system_font_files(file: &str) -> &'static [&'static str] {
    match file {
        "segoeui.ttf" | "arial.ttf" | "calibri.ttf" => {
            &["/System/Library/Fonts/Supplemental/Arial.ttf"]
        }
        "segoeuib.ttf" | "arialbd.ttf" | "calibrib.ttf" => {
            &["/System/Library/Fonts/Supplemental/Arial Bold.ttf"]
        }
        "verdana.ttf" => &["/System/Library/Fonts/Supplemental/Verdana.ttf"],
        "verdanab.ttf" => &["/System/Library/Fonts/Supplemental/Verdana Bold.ttf"],
        "tahoma.ttf" => &["/System/Library/Fonts/Supplemental/Tahoma.ttf"],
        "tahomabd.ttf" => &["/System/Library/Fonts/Supplemental/Tahoma Bold.ttf"],
        "consola.ttf" => &["/System/Library/Fonts/Supplemental/Courier New.ttf"],
        "consolab.ttf" => &["/System/Library/Fonts/Supplemental/Courier New Bold.ttf"],
        "seguisym.ttf" => &["/System/Library/Fonts/Apple Symbols.ttf"],
        _ => &[],
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
fn other_system_font_files(file: &str) -> &'static [&'static str] {
    // Debian/Ubuntu, Fedora and Arch keep the same fonts in different folders
    match file {
        "segoeui.ttf" | "arial.ttf" | "verdana.ttf" | "tahoma.ttf" | "calibri.ttf" => &[
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            "/usr/share/fonts/liberation-sans/LiberationSans-Regular.ttf",
            "/usr/share/fonts/liberation-sans-fonts/LiberationSans-Regular.ttf",
            "/usr/share/fonts/TTF/LiberationSans-Regular.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
        ],
        "segoeuib.ttf" | "arialbd.ttf" | "verdanab.ttf" | "tahomabd.ttf" | "calibrib.ttf" => &[
            "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
            "/usr/share/fonts/liberation-sans/LiberationSans-Bold.ttf",
            "/usr/share/fonts/liberation-sans-fonts/LiberationSans-Bold.ttf",
            "/usr/share/fonts/TTF/LiberationSans-Bold.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
            "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans-Bold.ttf",
            "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
        ],
        "consola.ttf" => &[
            "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
            "/usr/share/fonts/dejavu-sans-mono-fonts/DejaVuSansMono.ttf",
            "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
        ],
        "consolab.ttf" => &[
            "/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf",
            "/usr/share/fonts/dejavu-sans-mono-fonts/DejaVuSansMono-Bold.ttf",
            "/usr/share/fonts/TTF/DejaVuSansMono-Bold.ttf",
        ],
        // symbols such as ♡ ⚔ ✨
        "seguisym.ttf" => &[
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
        ],
        "seguiemj.ttf" => &[
            "/usr/share/fonts/truetype/noto/NotoSansSymbols2-Regular.ttf",
            "/usr/share/fonts/google-noto/NotoSansSymbols2-Regular.ttf",
            "/usr/share/fonts/noto/NotoSansSymbols2-Regular.ttf",
        ],
        _ => &[],
    }
}

pub fn install(ctx: &egui::Context) {
    // web images for emotes and badges (webp, gif, png over https)
    egui_extras::install_image_loaders(ctx);
    apply(ctx, &Appearance::default());
}

/// Applies an appearance: fonts, the colour palette and the widget look.
pub fn apply(ctx: &egui::Context, look: &Appearance) {
    theme::set(theme::Palette::named(&look.theme, look.accent));
    let p = theme::c();
    let mut fonts = FontDefinitions::default();
    let mut load = |key: &str, file: &str| -> Option<String> {
        let bytes = read_system_font(file)?;
        fonts
            .font_data
            .insert(key.into(), Arc::new(FontData::from_owned(bytes)));
        Some(key.to_string())
    };
    let (_, reg_file, bold_file) = FONTS
        .iter()
        .find(|(n, _, _)| *n == look.font)
        .copied()
        .unwrap_or(FONTS[0]);
    let regular = load("ui-regular", reg_file).or_else(|| load("segoeui", "segoeui.ttf"));
    let semibold = load("ui-bold", bold_file).or_else(|| load("segoeuib", "segoeuib.ttf"));
    let arialbd = load("arialbd", "arialbd.ttf");
    #[allow(unused_mut)]
    let mut back: Vec<String> = [
        ("seguisym", "seguisym.ttf"),
        ("seguiemj", "seguiemj.ttf"),
        ("msyh", "msyh.ttc"),
    ]
    .iter()
    .filter_map(|(k, f)| load(k, f))
    .collect();
    // macOS and Linux: emoji such as 💬 and ✨ from the bundled black-and-white font (their
    // own emoji fonts are in colour, which egui can't draw)
    #[cfg(not(windows))]
    {
        fonts.font_data.insert(
            "noto-emoji".into(),
            Arc::new(FontData::from_static(NOTO_EMOJI)),
        );
        back.push("noto-emoji".into());
    }
    let default_prop = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let family = |first: Option<String>| -> Vec<String> {
        let mut v: Vec<String> = first.into_iter().collect();
        v.extend(default_prop.iter().cloned());
        v.extend(back.iter().cloned());
        v
    };
    fonts
        .families
        .insert(FontFamily::Proportional, family(regular));
    fonts.families.insert(bold(), family(semibold.clone()));
    fonts
        .families
        .insert(list_family(), family(arialbd.or(semibold)));
    if let Some(list) = fonts.families.get_mut(&FontFamily::Monospace) {
        list.extend(back.iter().cloned());
    }
    ctx.set_fonts(fonts);
    ctx.set_zoom_factor(look.zoom.clamp(0.8, 1.5));

    // flat, minimal widgets in the theme's colours
    let mut style = (*ctx.global_style()).clone();
    let v = &mut style.visuals;
    *v = if p.light {
        egui::Visuals::light()
    } else {
        egui::Visuals::dark()
    };
    // light themes need stronger steps: their pages are already a light grey
    let shade = |d: i16| p.shade(if p.light { d * 2 + 12 } else { d });
    v.panel_fill = p.window_bg;
    v.window_fill = shade(13);
    v.window_stroke = Stroke::new(1.0, shade(34));
    v.window_corner_radius = egui::CornerRadius::same(4);
    v.window_shadow = egui::Shadow {
        offset: [0, 4],
        blur: 16,
        spread: 0,
        color: Color32::from_black_alpha(if p.light { 50 } else { 120 }),
    };
    v.popup_shadow = egui::Shadow {
        offset: [0, 2],
        blur: 8,
        spread: 0,
        color: Color32::from_black_alpha(if p.light { 40 } else { 110 }),
    };
    v.menu_corner_radius = egui::CornerRadius::same(3);
    v.extreme_bg_color = p.input_bg;
    v.faint_bg_color = p.alternate_bg;
    v.override_text_color = Some(p.text);
    v.selection.bg_fill =
        Color32::from_rgba_unmultiplied(p.accent.r(), p.accent.g(), p.accent.b(), 90);
    v.selection.stroke = Stroke::new(1.0, p.accent);
    v.hyperlink_color = Color32::from_rgb(0x42, 0x86, 0xf4);
    for (w, fill) in [
        (&mut v.widgets.noninteractive, shade(13)),
        (&mut v.widgets.inactive, shade(26)),
        (&mut v.widgets.hovered, shade(41)),
        (&mut v.widgets.active, shade(55)),
        (&mut v.widgets.open, shade(34)),
    ] {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.corner_radius = egui::CornerRadius::same(2);
        w.expansion = 0.0;
        w.bg_stroke = Stroke::NONE;
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, shade(31));
    v.widgets.hovered.fg_stroke.color = p.strong_text;
    style.spacing.item_spacing = Vec2::new(6.0, 4.0);
    style.spacing.button_padding = Vec2::new(8.0, 3.0);
    style.spacing.interact_size.y = 22.0;
    style.spacing.window_margin = egui::Margin::same(10);
    style.spacing.scroll = egui::style::ScrollStyle::solid();
    style.spacing.scroll.bar_width = 10.0;
    style.spacing.scroll.bar_inner_margin = 0.0;
    style.spacing.scroll.bar_outer_margin = 0.0;
    // text: 13 px like Chatterino; headings a little larger
    use egui::{FontId, TextStyle};
    style.text_styles = [
        (TextStyle::Small, FontId::proportional(11.0)),
        (TextStyle::Body, FontId::proportional(13.0)),
        (TextStyle::Button, FontId::proportional(13.0)),
        (TextStyle::Heading, FontId::new(17.0, bold())),
        (TextStyle::Monospace, FontId::monospace(12.0)),
    ]
    .into();
    ctx.set_global_style(style);
}

/// The colour palette: Moltorino's themes (resources/themes/Dark, Black, Light, White.json).
pub mod theme {
    use egui::Color32;
    use std::sync::RwLock;

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Palette {
        pub light: bool,
        pub window_bg: Color32,
        pub split_bg: Color32,
        pub alternate_bg: Color32,
        pub input_bg: Color32,
        pub tab_bg: Color32,
        pub tab_line: Color32,
        pub tab_text: Color32,
        pub tab_selected_bg: Color32,
        pub tab_selected_text: Color32,
        pub tab_new_line: Color32,
        pub tab_highlight_line: Color32,
        pub header_bg: Color32,
        pub header_text: Color32,
        pub header_focused_bg: Color32,
        pub header_focused_text: Color32,
        pub separator: Color32,
        pub accent: Color32,
        pub live: Color32,
        pub text: Color32,
        /// Text that stands out (names, headings, selected): white on dark, black on light.
        pub strong_text: Color32,
        pub system: Color32,
    }

    const fn hex(v: u32) -> Color32 {
        Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }

    impl Palette {
        pub const DARK: Palette = Palette {
            light: false,
            window_bg: hex(0x111111),
            split_bg: hex(0x191919),
            alternate_bg: hex(0x222222),
            input_bg: hex(0x242424),
            tab_bg: hex(0x252525),
            tab_line: hex(0x444444),
            tab_text: hex(0xaaaaaa),
            tab_selected_bg: hex(0x555555),
            tab_selected_text: hex(0xffffff),
            tab_new_line: hex(0x888888),
            tab_highlight_line: hex(0xee6166),
            header_bg: hex(0x2e2e2e),
            header_text: hex(0xffffff),
            header_focused_bg: hex(0x444444),
            header_focused_text: hex(0x84c1ff),
            separator: hex(0x3c3c3c),
            accent: hex(0x00aeef),
            live: hex(0xff0000),
            text: hex(0xe6e6e6),
            strong_text: hex(0xffffff),
            system: hex(0x8c7f7f),
        };
        pub const BLACK: Palette = Palette {
            window_bg: hex(0x040404),
            split_bg: hex(0x000000),
            alternate_bg: hex(0x0a0a0a),
            input_bg: hex(0x080808),
            tab_bg: hex(0x0b0b0b),
            tab_selected_bg: hex(0x333333),
            header_bg: hex(0x050505),
            header_focused_bg: hex(0x1a1a1a),
            ..Palette::DARK
        };
        pub const LIGHT: Palette = Palette {
            light: true,
            window_bg: hex(0xffffff),
            split_bg: hex(0xe6e6e6),
            alternate_bg: hex(0xdddddd),
            input_bg: hex(0xdbdbdb),
            tab_bg: hex(0xffffff),
            tab_line: hex(0xffffff),
            tab_text: hex(0x444444),
            tab_selected_bg: hex(0xb4d7ff),
            tab_selected_text: hex(0x000000),
            tab_new_line: hex(0xbbbbbb),
            tab_highlight_line: hex(0xff0000),
            header_bg: hex(0xe6e6e6),
            header_text: hex(0x000000),
            header_focused_bg: hex(0xdbdbdb),
            header_focused_text: hex(0x0051a3),
            separator: hex(0x7f7f7f),
            accent: hex(0x00aeef),
            live: hex(0xff0000),
            text: hex(0x111111),
            strong_text: hex(0x000000),
            system: hex(0x8c7f7f),
        };
        pub const WHITE: Palette = Palette {
            split_bg: hex(0xffffff),
            alternate_bg: hex(0xf5f5f5),
            input_bg: hex(0xf2f2f2),
            header_bg: hex(0xffffff),
            header_focused_bg: hex(0xf2f2f2),
            ..Palette::LIGHT
        };

        pub fn named(name: &str, accent: Option<[u8; 3]>) -> Palette {
            let mut p = match name {
                "Black" => Palette::BLACK,
                "Light" => Palette::LIGHT,
                "White" => Palette::WHITE,
                _ => Palette::DARK,
            };
            if let Some([r, g, b]) = accent {
                p.accent = Color32::from_rgb(r, g, b);
            }
            p
        }

        /// The window colour moved `d` steps towards the text (lighter on dark themes,
        /// darker on light ones), for widget fills.
        pub fn shade(&self, d: i16) -> Color32 {
            let b = self.window_bg;
            let f = |c: u8| -> u8 {
                let v = if self.light {
                    c as i16 - d
                } else {
                    c as i16 + d
                };
                v.clamp(0, 255) as u8
            };
            Color32::from_rgb(f(b.r()), f(b.g()), f(b.b()))
        }
    }

    static CURRENT: RwLock<Palette> = RwLock::new(Palette::DARK);

    /// The current palette.
    pub fn c() -> Palette {
        CURRENT.read().map(|p| *p).unwrap_or(Palette::DARK)
    }

    pub fn set(p: Palette) {
        if let Ok(mut c) = CURRENT.write() {
            *c = p;
        }
    }
}
/// How a tab looks: selected, new messages, a mention, live.
#[derive(Clone, Copy, Default)]
pub struct TabState {
    pub selected: bool,
    pub unread: bool,
    pub mentioned: bool,
    pub live: bool,
    /// Show the close "×" (Customization > tab close button).
    pub closable: bool,
    /// Locked (the tab, or the whole tab layout): a lock instead of the close "×".
    pub locked: bool,
    /// Locked, but show no padlock and keep no room for it (Moltorino).
    pub hide_lock: bool,
    /// Moltorino custom tab colour: the tab's line.
    pub color: Option<[u8; 3]>,
    /// Moltorino thinner tab lines and tighter spacing.
    pub compact: bool,
}

/// A tab in Chatterino's style (Dark.json tabs): #252525 with a 2 px line on top,
/// #888 for new messages, #ee6166 for a mention; selected #555 with the accent line;
/// a small red dot when a channel in it is live.
/// Also Chatterino's / Moltorino's tab close button: an "×" on the right of the hovered
/// or selected tab; the second value is true when it was clicked.
pub fn tab_button(
    ui: &mut egui::Ui,
    label: &str,
    st: TabState,
    min_w: f32,
) -> (egui::Response, bool) {
    let font = egui::FontId::proportional(13.0);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font, Color32::WHITE);
    let dot = if st.live { 10.0 } else { 0.0 };
    let show_lock = st.locked && !st.hide_lock;
    let close_w = if (st.closable && !st.locked) || show_lock {
        16.0
    } else {
        0.0
    };
    let (pad, height, line_w) = if st.compact {
        (12.0, 22.0, 1.0)
    } else {
        (20.0, 26.0, 2.0)
    };
    let size = Vec2::new((galley.size().x + pad + dot + close_w).max(min_w), height);
    // drag to move the tab (unless locked)
    let (rect, resp) = ui.allocate_exact_size(
        size,
        if st.locked {
            Sense::click()
        } else {
            Sense::click_and_drag()
        },
    );
    let bg = if st.selected {
        theme::c().tab_selected_bg
    } else if resp.hovered() {
        theme::c().shade(31)
    } else {
        theme::c().tab_bg
    };
    let line = if let Some([r, g, b]) = st.color {
        Color32::from_rgb(r, g, b)
    } else if st.selected {
        theme::c().accent
    } else if st.mentioned {
        theme::c().tab_highlight_line
    } else if st.unread {
        theme::c().tab_new_line
    } else {
        theme::c().tab_line
    };
    let p = ui.painter();
    p.rect_filled(rect, 0.0, bg);
    p.hline(
        rect.x_range(),
        rect.top() + line_w / 2.0,
        Stroke::new(line_w, line),
    );
    if st.color.is_some() && st.mentioned && !st.selected {
        // a coloured tab still shows a mention: a thin red line under its colour
        p.hline(
            rect.x_range(),
            rect.top() + line_w + 0.5,
            Stroke::new(1.0, theme::c().tab_highlight_line),
        );
    }
    let text_color = if st.selected {
        theme::c().tab_selected_text
    } else if st.unread || st.mentioned {
        theme::c().strong_text
    } else {
        theme::c().tab_text
    };
    let x = rect.left() + pad / 2.0;
    if st.live {
        p.circle_filled(
            egui::Pos2::new(x + 3.0, rect.center().y),
            3.0,
            theme::c().live,
        );
    }
    p.galley_with_override_text_color(
        egui::Pos2::new(x + dot, rect.center().y - galley.size().y / 2.0),
        galley,
        text_color,
    );
    // the close "×"
    let xr = egui::Rect::from_center_size(
        egui::Pos2::new(rect.right() - 12.0, rect.center().y + 1.0),
        Vec2::splat(12.0),
    );
    let over_x = resp.hover_pos().is_some_and(|h| xr.expand(2.0).contains(h));
    let mut closed = false;
    if show_lock {
        // a small padlock
        let c = egui::Pos2::new(rect.right() - 12.0, rect.center().y + 1.0);
        let col = theme::c().tab_text;
        p.rect_filled(
            egui::Rect::from_center_size(c + Vec2::new(0.0, 2.0), Vec2::new(8.0, 6.0)),
            1.0,
            col,
        );
        p.circle_stroke(c + Vec2::new(0.0, -1.5), 2.6, Stroke::new(1.3, col));
    } else if st.closable && !st.locked && (resp.hovered() || st.selected) {
        if over_x {
            p.rect_filled(xr.expand(2.0), 2.0, theme::c().shade(95));
        }
        let c = if over_x {
            theme::c().strong_text
        } else {
            theme::c().tab_text
        };
        let s = Stroke::new(1.3, c);
        let k = xr.shrink(2.5);
        p.line_segment([k.left_top(), k.right_bottom()], s);
        p.line_segment([k.right_top(), k.left_bottom()], s);
        closed = resp.clicked()
            && resp
                .interact_pointer_pos()
                .is_some_and(|h| xr.expand(2.0).contains(h));
    }
    (resp, closed)
}

/// A flat text tab (used for the platform choice in "Select a channel").
pub fn flat_tab(ui: &mut egui::Ui, label: &str, selected: bool, accent: Color32) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(
        label.to_string(),
        egui::FontId::proportional(13.0),
        Color32::WHITE,
    );
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(galley.size().x + 18.0, 26.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() && !selected {
        p.rect_filled(rect, 2.0, theme::c().shade(25));
    }
    if selected {
        p.hline(
            rect.shrink2(Vec2::new(4.0, 0.0)).x_range(),
            rect.bottom() - 1.0,
            Stroke::new(2.0, accent),
        );
    }
    let c = if selected {
        theme::c().strong_text
    } else {
        theme::c().tab_text
    };
    p.galley_with_override_text_color(rect.center() - galley.size() / 2.0, galley, c);
    resp
}
pub fn name_color(c: Option<[u8; 3]>, name: &str) -> Color32 {
    if let Some([r, g, b]) = c {
        // lift very dark colours so they stay readable on black
        let lum = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
        if lum < 70.0 {
            let k = 70.0 / lum.max(1.0);
            return Color32::from_rgb(
                (r as f32 * k).min(255.0) as u8,
                (g as f32 * k).min(255.0) as u8,
                (b as f32 * k).min(255.0) as u8,
            );
        }
        return Color32::from_rgb(r, g, b);
    }
    // stable default colour per name, like chat clients do for users without one
    const PALETTE: [Color32; 8] = [
        Color32::from_rgb(255, 99, 71),
        Color32::from_rgb(30, 144, 255),
        Color32::from_rgb(50, 205, 50),
        Color32::from_rgb(218, 165, 32),
        Color32::from_rgb(255, 105, 180),
        Color32::from_rgb(138, 43, 226),
        Color32::from_rgb(0, 206, 209),
        Color32::from_rgb(255, 140, 0),
    ];
    let h = name
        .bytes()
        .fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
    PALETTE[(h as usize) % PALETTE.len()]
}

/// How chat is drawn (Settings > Chat and Emotes).
pub struct ChatLook {
    pub font_size: f32,
    pub timestamps: bool,
    pub emote_scale: f32,
    pub emote_images: bool,
    pub tooltips: bool,
    pub badges: bool,
    pub alternate: bool,
    pub separators: bool,
    /// Lower-case name whose mentions are highlighted.
    pub mention: Option<String>,
    /// Highlights (Chatterino): lower-case user names and phrases with their colours.
    /// Highlights (Chatterino): message, user and badge rules, compiled.
    pub matchers: std::sync::Arc<crate::rules::Matchers>,
    /// The split's filters (a message must pass all), and whether one's own messages skip them.
    pub filters: Vec<crate::rules::Expr>,
    pub filter_skip_own: bool,
    /// Emote names in the emote window's favourites (for the right-click menu).
    pub favourites: std::collections::HashSet<String>,
    /// Moderation in this (Twitch) chat: the message menu's Moderation items, Chatterino's
    /// moderation buttons before each message, and the timeout buttons (label, seconds).
    pub moderation: bool,
    pub mod_buttons: bool,
    pub timeouts: Vec<(String, u64)>,
    /// Nicknames: lower-case user name -> shown name.
    pub nicknames: std::collections::HashMap<String, String>,
    /// Ctrl+F: only lines containing this (lower-case).
    pub search: Option<String>,
    pub platform_badges: bool,
    /// Moltorino message source: a platform-coloured background.
    pub source_background: bool,
    /// Moltorino /hide: lower-case names whose messages aren't shown.
    pub hidden: std::collections::HashSet<String>,
    pub bold_names: bool,
    pub user_colors: bool,
}

const MENTION_BG: Color32 = Color32::from_rgb(0x4b, 0x28, 0x2c);

/// What a chat line's menu or buttons asked for (emote right-click, moderation).
pub enum LineAction {
    /// Timeout, ban, unban or delete (moderation buttons, message menu > Moderation).
    Moderate(crate::moderation::ModAction),
    /// Put this text into the message box.
    Add(String),
    /// Add it to (or take it out of) the emote window's favourites.
    Favourite(emotes::Emote),
    /// Hide this user's messages (Moltorino /hide).
    Hide(String),
    /// A clicked #channel: switch to its tab or open it.
    OpenChannel(String),
    /// A clicked user name: open the usercard.
    UserCard(UserRef),
    /// Message menu > Reply: the next message answers this one.
    Reply(crate::chat::Reply),
    /// A clicked "Replying to" line: show that reply thread (its first message's id).
    Thread(String),
    /// Message menu > Translate (Moltorino): the translation goes under this message.
    Translate(ChatLine),
}

/// Who a clicked name belongs to.
#[derive(Clone, Debug)]
pub struct UserRef {
    pub name: String,
    /// Twitch's user id (empty on the other platforms).
    pub user_id: String,
    pub platform: crate::platforms::Platform,
}

/// Draws a chat; returns what an emote's right-click menu asked for.
pub fn chat_lines(
    ui: &mut egui::Ui,
    lines: &VecDeque<ChatLine>,
    store: &Store,
    channel: &str,
    look: &ChatLook,
) -> Option<LineAction> {
    let mut insert: Option<LineAction> = None;
    let font = egui::FontId::proportional(look.font_size);
    let text_color = theme::c().text;
    let emote_h = emotes::EMOTE_HEIGHT * look.emote_scale;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .stick_to_bottom(true)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = if look.separators { 0.0 } else { 2.0 };
            let mut n = 0usize;
            for line in lines.iter() {
                // Moltorino /hide: hidden here, not blocked
                if let ChatLine::Msg {
                    name, own: false, ..
                } = line
                {
                    if !look.hidden.is_empty() && look.hidden.contains(&name.to_lowercase()) {
                        continue;
                    }
                }
                if let Some(q) = look.search.as_ref().filter(|q| !q.is_empty()) {
                    let hay = match line {
                        ChatLine::Msg { name, text, .. } => {
                            format!("{} {}", name.to_lowercase(), text.to_lowercase())
                        }
                        ChatLine::System(t) => t.to_lowercase(),
                        ChatLine::AutoMod(h) => h.summary().to_lowercase(),
                    };
                    if !hay.contains(q.as_str()) {
                        continue;
                    }
                }
                // the split's filters (Chatterino's Filters): a message must pass all of them
                if !look.filters.is_empty() {
                    if let ChatLine::Msg {
                        name,
                        text,
                        own,
                        platform,
                        badges,
                        segs,
                        ..
                    } = line
                    {
                        if !(*own && look.filter_skip_own) {
                            let input = crate::rules::FilterInput {
                                author: name,
                                content: text,
                                channel,
                                platform: platform.label(),
                                badges: badges.iter().map(|(set, _)| set.clone()).collect(),
                                emote_only: segs.iter().any(|s| matches!(s, Seg::Emote(_)))
                                    && segs.iter().all(|s| match s {
                                        Seg::Emote(_) => true,
                                        Seg::Text(t) => t.trim().is_empty(),
                                    }),
                            };
                            if !look.filters.iter().all(|f| crate::rules::eval(f, &input)) {
                                continue;
                            }
                        }
                    }
                }
                n += 1;
                let mentioned = match (line, &look.mention) {
                    (
                        ChatLine::Msg {
                            text, own: false, ..
                        },
                        Some(me),
                    ) => text
                        .to_lowercase()
                        .split(|c: char| !c.is_alphanumeric() && c != '_')
                        .any(|w| w == me),
                    _ => false,
                };
                let custom = match line {
                    ChatLine::Msg {
                        name,
                        text,
                        own: false,
                        badges,
                        ..
                    } => look
                        .matchers
                        .highlight(name, text, badges)
                        .map(|h| Color32::from_rgb(h.color[0], h.color[1], h.color[2])),
                    _ => None,
                };
                let bg = if let Some(c) = custom {
                    c
                } else if mentioned {
                    MENTION_BG
                } else if let (true, ChatLine::Msg { platform, .. }) =
                    (look.source_background, line)
                {
                    // Moltorino message source: a faint platform-coloured background
                    let c = platform.color();
                    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 22)
                } else if look.alternate && n % 2 == 1 {
                    theme::c().alternate_bg
                } else {
                    Color32::TRANSPARENT
                };
                let frame = egui::Frame::new()
                    .fill(bg)
                    .inner_margin(egui::Margin::symmetric(
                        2,
                        if look.separators { 3 } else { 0 },
                    ));
                let mut hits: Vec<(egui::Rect, emotes::Emote)> = Vec::new();
                let mut mod_hits: Vec<(egui::Rect, crate::moderation::ModAction)> = Vec::new();
                let mut chan_hits: Vec<(egui::Rect, String)> = Vec::new();
                let mut name_hits: Vec<(egui::Rect, UserRef)> = Vec::new();
                let mut reply_hits: Vec<(egui::Rect, String)> = Vec::new();
                let r = frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    draw_line(
                        ui,
                        line,
                        store,
                        channel,
                        look,
                        &font,
                        text_color,
                        emote_h,
                        &mut hits,
                        &mut mod_hits,
                        &mut chan_hits,
                        &mut name_hits,
                        &mut reply_hits,
                    );
                });
                if look.separators {
                    let y = r.response.rect.bottom();
                    ui.painter().hline(
                        r.response.rect.x_range(),
                        y,
                        Stroke::new(1.0, theme::c().separator),
                    );
                }
                // right-click a message: copy it (Chatterino's message menu); dragging over the
                // text still selects it, and Ctrl+C copies the selection
                let menu = ui.interact(
                    r.response.rect,
                    ui.make_persistent_id(("chat-line", n)),
                    Sense::click(),
                );
                // right-click on an emote: the emote's menu first (Chatterino's emote menu)
                let emote_key = menu.id.with("emote");
                // Chatterino's moderation buttons before the name (drawn under this area)
                if menu.clicked() {
                    let at = menu.interact_pointer_pos();
                    if let Some(a) = at.and_then(|p| {
                        mod_hits
                            .iter()
                            .find(|(r, _)| r.contains(p))
                            .map(|(_, a)| a.clone())
                    }) {
                        insert = Some(LineAction::Moderate(a));
                    } else if let Some(c) = at.and_then(|p| {
                        chan_hits
                            .iter()
                            .find(|(r, _)| r.contains(p))
                            .map(|(_, c)| c.clone())
                    }) {
                        insert = Some(LineAction::OpenChannel(c));
                    } else if let Some(u) = at.and_then(|p| {
                        name_hits
                            .iter()
                            .find(|(r, _)| r.contains(p))
                            .map(|(_, u)| u.clone())
                    }) {
                        insert = Some(LineAction::UserCard(u));
                    } else if let Some(t) = at.and_then(|p| {
                        reply_hits
                            .iter()
                            .find(|(r, _)| r.contains(p))
                            .map(|(_, t)| t.clone())
                    }) {
                        insert = Some(LineAction::Thread(t));
                    }
                }
                if menu.hovered()
                    && menu.hover_pos().is_some_and(|p| {
                        chan_hits.iter().any(|(r, _)| r.contains(p))
                            || name_hits.iter().any(|(r, _)| r.contains(p))
                            || reply_hits.iter().any(|(r, _)| r.contains(p))
                    })
                {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if menu.secondary_clicked() {
                    let hit = menu.interact_pointer_pos().and_then(|p| {
                        hits.iter()
                            .find(|(r, _)| r.contains(p))
                            .map(|(_, e)| e.clone())
                    });
                    ui.data_mut(|d| d.insert_temp(emote_key, hit));
                }
                menu.context_menu(|ui| {
                    if let Some(e) = ui
                        .data(|d| d.get_temp::<Option<emotes::Emote>>(emote_key))
                        .flatten()
                    {
                        if let Some(a) = emote_menu_items(ui, &e, look.favourites.contains(&e.name))
                        {
                            insert = Some(a);
                        }
                        ui.separator();
                    }
                    let (full, name, body) = match line {
                        ChatLine::Msg {
                            time, name, text, ..
                        } => (
                            format!("{time} {name}: {text}"),
                            Some(name.clone()),
                            text.clone(),
                        ),
                        ChatLine::System(t) => (t.clone(), None, t.clone()),
                        ChatLine::AutoMod(h) => (h.summary(), Some(h.user.clone()), h.text.clone()),
                    };
                    // Twitch replies (Chatterino's message menu > Reply)
                    if let ChatLine::Msg {
                        name,
                        text,
                        msg_id,
                        reply,
                        platform: crate::platforms::Platform::Twitch,
                        ..
                    } = line
                    {
                        if !msg_id.is_empty() && ui.button("Reply").clicked() {
                            insert = Some(LineAction::Reply(crate::chat::Reply {
                                parent_id: msg_id.clone(),
                                parent_name: name.clone(),
                                parent_body: text.clone(),
                                thread_id: reply
                                    .as_ref()
                                    .map(|r| r.thread_id.clone())
                                    .unwrap_or_else(|| msg_id.clone()),
                            }));
                            ui.close();
                        }
                    }
                    if matches!(line, ChatLine::Msg { .. }) && ui.button("Translate").clicked() {
                        insert = Some(LineAction::Translate(line.clone()));
                        ui.close();
                    }
                    // pin it at the top of the chat (where you moderate; Twitch's pins)
                    if let ChatLine::Msg {
                        name,
                        msg_id,
                        user_id,
                        platform: crate::platforms::Platform::Twitch,
                        ..
                    } = line
                    {
                        if look.moderation && !msg_id.is_empty() {
                            ui.menu_button("Pin message", |ui| {
                                let lengths = [
                                    ("Until the stream ends", None),
                                    ("1 minute", Some(60)),
                                    ("5 minutes", Some(300)),
                                    ("10 minutes", Some(600)),
                                    ("30 minutes", Some(1800)),
                                ];
                                for (label, secs) in lengths {
                                    if ui.button(label).clicked() {
                                        insert = Some(LineAction::Moderate(
                                            crate::moderation::ModAction {
                                                user: name.clone(),
                                                user_id: user_id.clone(),
                                                kind: crate::moderation::ModKind::Pin {
                                                    msg_id: msg_id.clone(),
                                                    secs,
                                                },
                                            },
                                        ));
                                        ui.close();
                                    }
                                }
                            });
                        }
                    }
                    if ui.button("Copy message").clicked() {
                        ui.ctx().copy_text(body);
                        ui.close();
                    }
                    if ui.button("Copy full message").clicked() {
                        ui.ctx().copy_text(full);
                        ui.close();
                    }
                    if let Some(name) = name {
                        if ui.button("Copy user name").clicked() {
                            ui.ctx().copy_text(name.clone());
                            ui.close();
                        }
                        if !matches!(line, ChatLine::Msg { own: true, .. })
                            && ui
                                .button(format!("Hide messages from {name}"))
                                .on_hover_text("Only here; /unhide brings them back")
                                .clicked()
                        {
                            insert = Some(LineAction::Hide(name));
                            ui.close();
                        }
                    }
                    // message menu > Moderation (Twitch channels you moderate)
                    if let ChatLine::Msg {
                        name,
                        own: false,
                        platform: crate::platforms::Platform::Twitch,
                        msg_id,
                        user_id,
                        ..
                    } = line
                    {
                        if look.moderation && !user_id.is_empty() {
                            ui.separator();
                            ui.menu_button("Moderation", |ui| {
                                let act = |kind| crate::moderation::ModAction {
                                    user: name.clone(),
                                    user_id: user_id.clone(),
                                    kind,
                                };
                                ui.menu_button("Timeout", |ui| {
                                    for (label, secs) in &look.timeouts {
                                        if ui.button(label).clicked() {
                                            insert = Some(LineAction::Moderate(act(
                                                crate::moderation::ModKind::Timeout(*secs),
                                            )));
                                            ui.close();
                                        }
                                    }
                                });
                                if ui.button("Ban").clicked() {
                                    insert = Some(LineAction::Moderate(act(
                                        crate::moderation::ModKind::Ban,
                                    )));
                                    ui.close();
                                }
                                if ui.button("Unban").clicked() {
                                    insert = Some(LineAction::Moderate(act(
                                        crate::moderation::ModKind::Unban,
                                    )));
                                    ui.close();
                                }
                                if !msg_id.is_empty() && ui.button("Delete message").clicked() {
                                    insert = Some(LineAction::Moderate(act(
                                        crate::moderation::ModKind::Delete(msg_id.clone()),
                                    )));
                                    ui.close();
                                }
                            });
                        }
                    }
                });
            }
        });
    insert
}

/// Chatterino's emote menu: the emote's name and source, then Add to message, Copy emote
/// name, Copy emote link, Open emote link. Returns the text to add to the message box.
pub fn emote_menu_items(
    ui: &mut egui::Ui,
    e: &emotes::Emote,
    favourite: bool,
) -> Option<LineAction> {
    let mut add = None;
    ui.set_min_width(200.0);
    ui.horizontal(|ui| {
        ui.add(
            egui::Image::new(e.url.as_str())
                .fit_to_original_size(1.0)
                .max_height(28.0)
                .max_width(84.0),
        );
        ui.vertical(|ui| {
            ui.label(RichText::new(&e.name).strong());
            ui.weak(format!("{} emote", e.provider.label()));
        });
    });
    ui.separator();
    if ui.button("Add to message").clicked() {
        add = Some(LineAction::Add(e.name.clone()));
        ui.close();
    }
    if ui
        .button(if favourite {
            "Remove from favourites"
        } else {
            "Add to favourites"
        })
        .clicked()
    {
        add = Some(LineAction::Favourite(e.clone()));
        ui.close();
    }
    if ui.button("Copy emote name").clicked() {
        ui.ctx().copy_text(e.name.clone());
        ui.close();
    }
    if ui.button("Copy emote link").clicked() {
        ui.ctx().copy_text(e.page());
        ui.close();
    }
    if ui.button("Open emote link").clicked() {
        crate::browser::open_url(&e.page());
        ui.close();
    }
    if ui.button("Open emote picture").clicked() {
        crate::browser::open_url(&e.large_url());
        ui.close();
    }
    add
}

#[allow(clippy::too_many_arguments)]
fn draw_line(
    ui: &mut egui::Ui,
    line: &ChatLine,
    store: &Store,
    channel: &str,
    look: &ChatLook,
    font: &egui::FontId,
    text_color: Color32,
    emote_h: f32,
    hits: &mut Vec<(egui::Rect, emotes::Emote)>,
    mod_hits: &mut Vec<(egui::Rect, crate::moderation::ModAction)>,
    chan_hits: &mut Vec<(egui::Rect, String)>,
    name_hits: &mut Vec<(egui::Rect, UserRef)>,
    reply_hits: &mut Vec<(egui::Rect, String)>,
) {
    match line {
        ChatLine::Msg {
            time,
            name,
            color,
            text,
            action,
            badges,
            segs,
            platform,
            own,
            msg_id,
            user_id,
            reply,
            ..
        } => {
            let nc = name_color(*color, name);
            // a reply: what it answers, small, above it (Chatterino); a click opens the thread
            if let Some(r) = reply {
                let body: String = r.parent_body.chars().take(90).collect();
                let more = if r.parent_body.chars().count() > 90 {
                    "…"
                } else {
                    ""
                };
                let resp = ui.add(
                    egui::Label::new(
                        RichText::new(format!("↪ Replying to @{}: {body}{more}", r.parent_name))
                            .size(font.size * 0.85)
                            .color(theme::c().tab_text),
                    )
                    .truncate(),
                );
                reply_hits.push((resp.rect, r.thread_id.clone()));
            }
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                if look.timestamps {
                    ui.label(
                        RichText::new(format!("{time} "))
                            .font(font.clone())
                            .color(Color32::from_rgb(130, 130, 130)),
                    );
                }
                // which platform the message came from: a badge before every message
                // (Moltorino shows platform badges)
                if look.platform_badges {
                    platform_badge(ui, *platform, (font.size + 2.0).clamp(13.0, 20.0))
                        .on_hover_text(platform.label());
                    ui.add_space(4.0);
                }
                // Chatterino's moderation buttons: delete, timeout (10 minutes or the first
                // timeout button), ban; clicked through the line's area (see chat_lines)
                if look.mod_buttons
                    && look.moderation
                    && *platform == crate::platforms::Platform::Twitch
                    && !*own
                    && !user_id.is_empty()
                {
                    let secs = look
                        .timeouts
                        .iter()
                        .find(|(_, s)| *s == 600)
                        .or(look.timeouts.first())
                        .map(|(_, s)| *s)
                        .unwrap_or(600);
                    let act = |kind| crate::moderation::ModAction {
                        user: name.clone(),
                        user_id: user_id.clone(),
                        kind,
                    };
                    let buttons = [
                        (
                            "🗑",
                            "Delete message",
                            (!msg_id.is_empty())
                                .then(|| act(crate::moderation::ModKind::Delete(msg_id.clone()))),
                        ),
                        (
                            "⏱",
                            "Timeout",
                            Some(act(crate::moderation::ModKind::Timeout(secs))),
                        ),
                        ("🚫", "Ban", Some(act(crate::moderation::ModKind::Ban))),
                    ];
                    for (icon, tip, a) in buttons {
                        let Some(a) = a else { continue };
                        let r = ui.add(
                            egui::Label::new(
                                RichText::new(icon)
                                    .font(egui::FontId::proportional(font.size))
                                    .color(Color32::from_rgb(0xb0, 0xb0, 0xb0)),
                            )
                            .sense(Sense::hover()),
                        );
                        let tip = if tip == "Timeout" {
                            format!("Timeout {}", crate::moderation::duration_text(secs))
                        } else {
                            tip.to_string()
                        };
                        mod_hits.push((r.rect, a));
                        r.on_hover_text(tip);
                        ui.add_space(3.0);
                    }
                }
                if look.badges {
                    for (set, version) in badges {
                        if let Some((url, title)) = store.badge(channel, set, version) {
                            let r = ui.add(
                                egui::Image::new(url.as_str())
                                    .fit_to_exact_size(Vec2::splat(emotes::BADGE_SIZE)),
                            );
                            if look.tooltips {
                                r.on_hover_text(title);
                            }
                            ui.add_space(3.0);
                        }
                    }
                }
                let shown = look
                    .nicknames
                    .get(&name.to_lowercase())
                    .cloned()
                    .unwrap_or_else(|| name.clone());
                let name_font = if look.bold_names {
                    egui::FontId::new(font.size, bold())
                } else {
                    font.clone()
                };
                let nc = if look.user_colors {
                    nc
                } else {
                    theme::c().strong_text
                };
                let name_rect = ui
                    .label(RichText::new(shown).font(name_font).color(nc))
                    .rect;
                // clicking the name opens the usercard (see chat_lines)
                name_hits.push((
                    name_rect,
                    UserRef {
                        name: name.clone(),
                        user_id: user_id.clone(),
                        platform: *platform,
                    },
                ));
                let body_color = if *action { nc } else { text_color };
                ui.label(
                    RichText::new(if *action { " " } else { ": " })
                        .font(font.clone())
                        .color(body_color),
                );
                if segs.is_empty() {
                    text_with_channels(ui, text, font, body_color, chan_hits);
                }
                for seg in segs {
                    match seg {
                        Seg::Text(t) => {
                            text_with_channels(ui, t, font, body_color, chan_hits);
                        }
                        Seg::Emote(e) if look.emote_images => {
                            let r = ui.add(
                                egui::Image::new(e.url.as_str())
                                    .fit_to_original_size(look.emote_scale)
                                    .max_height(emote_h),
                            );
                            hits.push((r.rect, e.clone()));
                            if look.tooltips {
                                r.on_hover_text(format!(
                                    "{}\n{} emote",
                                    e.name,
                                    e.provider.label()
                                ));
                            }
                        }
                        Seg::Emote(e) => {
                            ui.label(RichText::new(&e.name).font(font.clone()).color(body_color));
                        }
                    }
                }
            });
        }
        ChatLine::System(t) => {
            ui.label(RichText::new(t).font(font.clone()).color(theme::c().system));
        }
        // AutoMod held a message (Chatterino's AutoMod line): who, why, the message, and
        // Allow / Deny, clicked through the line's area like the moderation buttons
        ChatLine::AutoMod(h) => {
            let dim = Color32::from_rgb(130, 130, 130);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                if look.timestamps {
                    ui.label(
                        RichText::new(format!("{} ", h.time))
                            .font(font.clone())
                            .color(dim),
                    );
                }
                ui.label(
                    RichText::new("AutoMod ")
                        .font(font.clone())
                        .strong()
                        .color(Color32::from_rgb(0x8f, 0x8f, 0xff)),
                );
                ui.label(
                    RichText::new(format!("held a message from {} ({})", h.user, h.reason))
                        .font(font.clone())
                        .color(theme::c().system),
                );
            });
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.label(
                    RichText::new(format!("{}: {}", h.user, h.text))
                        .font(font.clone())
                        .color(text_color),
                );
                if h.status.is_empty() {
                    let buttons = [
                        ("Allow", true, Color32::from_rgb(0x4c, 0xc2, 0x6b)),
                        ("Deny", false, Color32::from_rgb(0xe8, 0x48, 0x48)),
                    ];
                    for (label, allow, col) in buttons {
                        let r = ui.add(
                            egui::Label::new(
                                RichText::new(format!(" {label} "))
                                    .font(font.clone())
                                    .strong()
                                    .color(col)
                                    .background_color(col.gamma_multiply(0.18)),
                            )
                            .sense(Sense::hover()),
                        );
                        mod_hits.push((
                            r.rect,
                            crate::moderation::ModAction {
                                user: h.user.clone(),
                                user_id: h.user_id.clone(),
                                kind: crate::moderation::ModKind::AutoMod {
                                    msg_id: h.msg_id.clone(),
                                    allow,
                                },
                            },
                        ));
                        r.on_hover_text(if allow {
                            "Allow: the message is posted in the chat"
                        } else {
                            "Deny: nobody sees the message"
                        });
                    }
                } else {
                    ui.label(
                        RichText::new(&h.status)
                            .font(font.clone())
                            .italics()
                            .color(dim),
                    );
                }
            });
        }
    }
}

/// Text with Moltorino's clickable #channel names (a word that starts with # and is a
/// valid channel name); clicks are read through the line's area (chan_hits).
fn text_with_channels(
    ui: &mut egui::Ui,
    t: &str,
    font: &egui::FontId,
    color: Color32,
    chan_hits: &mut Vec<(egui::Rect, String)>,
) {
    if !t.contains('#') {
        ui.label(RichText::new(t).font(font.clone()).color(color));
        return;
    }
    let mut plain = String::new();
    for word in t.split_inclusive(' ') {
        let w = word.trim_end();
        if let Some(name) = channel_word(w) {
            if !plain.is_empty() {
                ui.label(
                    RichText::new(std::mem::take(&mut plain))
                        .font(font.clone())
                        .color(color),
                );
            }
            let r = ui.label(
                RichText::new(w)
                    .font(font.clone())
                    .color(theme::c().accent)
                    .underline(),
            );
            chan_hits.push((r.rect, name.to_string()));
            r.on_hover_text(format!("Open #{name}"));
            plain.push_str(&word[w.len()..]);
        } else {
            plain.push_str(word);
        }
    }
    if !plain.is_empty() {
        ui.label(RichText::new(plain).font(font.clone()).color(color));
    }
}

/// "#name" -> "name" when it's a channel name (letters, digits, _; 1-25), else None.
pub fn channel_word(w: &str) -> Option<&str> {
    let name = w.strip_prefix('#')?;
    let ok = (1..=25).contains(&name.len())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    ok.then_some(name)
}

/// Icons of the title-bar buttons.
#[derive(Clone, Copy, PartialEq)]
pub enum BarIcon {
    Gear,
    Minimize,
    Maximize,
    Restore,
    Close,
    /// Chatterino's chatters (viewer list) button.
    People,
    /// Chatterino's split menu button (⋮).
    Dots,
    Plus,
    /// The live channels button: a red dot with broadcast waves.
    Live,
    /// The emote menu button: a smiling face.
    Smiley,
    /// Chatterino's moderation mode button: a sword.
    Sword,
}

/// A header button that stays lit while `on` (the moderation-mode sword).
pub fn bar_icon_toggle(ui: &mut egui::Ui, icon: BarIcon, size: Vec2, on: bool) -> egui::Response {
    let r = bar_icon_sized(ui, icon, size);
    if on {
        let accent = theme::c().accent;
        let p = ui.painter();
        p.rect_filled(r.rect, 2.0, accent.gamma_multiply(0.28));
        p.hline(
            (r.rect.left() + 3.0)..=(r.rect.right() - 3.0),
            r.rect.bottom() - 1.5,
            Stroke::new(2.0, accent),
        );
    }
    r
}

/// A title-bar button like Chatterino's: no frame, a light square when hovered.
pub fn bar_icon(ui: &mut egui::Ui, icon: BarIcon) -> egui::Response {
    bar_icon_sized(ui, icon, Vec2::new(28.0, 24.0))
}

/// A window button (minimize, maximize, close) as wide as Windows' own: 46 x 30;
/// close turns red when hovered.
pub fn window_button(ui: &mut egui::Ui, icon: BarIcon) -> egui::Response {
    bar_icon_sized(ui, icon, Vec2::new(46.0, 30.0))
}

pub fn bar_icon_sized(ui: &mut egui::Ui, icon: BarIcon, size: Vec2) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    let window_button = size.x >= 40.0;
    if resp.hovered() {
        let bg = if icon == BarIcon::Close {
            Color32::from_rgb(0xe8, 0x11, 0x23)
        } else {
            theme::c().shade(41)
        };
        p.rect_filled(rect, if window_button { 0.0 } else { 2.0 }, bg);
    }
    let col = if resp.hovered() && icon == BarIcon::Close {
        Color32::WHITE
    } else if resp.hovered() {
        theme::c().strong_text
    } else {
        theme::c().tab_text
    };
    let c = rect.center();
    let thin = Stroke::new(1.0, col);
    match icon {
        BarIcon::Minimize => {
            p.hline((c.x - 5.0)..=(c.x + 5.0), c.y + 0.5, thin);
        }
        BarIcon::Maximize => {
            p.rect_stroke(
                egui::Rect::from_center_size(c, Vec2::splat(10.0)),
                0.0,
                thin,
                egui::StrokeKind::Inside,
            );
        }
        BarIcon::Restore => {
            p.rect_stroke(
                egui::Rect::from_center_size(c + Vec2::new(-1.0, 1.0), Vec2::splat(8.0)),
                0.0,
                thin,
                egui::StrokeKind::Inside,
            );
            p.line_segment([c + Vec2::new(-2.0, -4.0), c + Vec2::new(5.0, -4.0)], thin);
            p.line_segment([c + Vec2::new(5.0, -4.0), c + Vec2::new(5.0, 3.0)], thin);
        }
        BarIcon::Close => {
            p.line_segment([c + Vec2::new(-5.0, -5.0), c + Vec2::new(5.0, 5.0)], thin);
            p.line_segment([c + Vec2::new(5.0, -5.0), c + Vec2::new(-5.0, 5.0)], thin);
        }
        BarIcon::People => {
            // two heads and shoulders
            let s = Stroke::new(1.4, col);
            p.circle_stroke(c + Vec2::new(-2.5, -3.0), 2.6, s);
            p.circle_stroke(c + Vec2::new(4.0, -2.0), 2.1, s);
            let body = |cx: f32, w: f32, top: f32| {
                let pts: Vec<egui::Pos2> = (0..=10)
                    .map(|i| {
                        let a = std::f32::consts::PI * i as f32 / 10.0;
                        c + Vec2::new(cx - w * a.cos(), top - w * a.sin() * 0.9 + w)
                    })
                    .collect();
                pts
            };
            p.add(egui::Shape::line(body(-2.5, 4.5, 2.0), s));
            p.add(egui::Shape::line(body(4.5, 3.4, 2.4), s));
        }
        BarIcon::Dots => {
            for dy in [-4.5, 0.0, 4.5] {
                p.circle_filled(c + Vec2::new(0.0, dy), 1.3, col);
            }
        }
        BarIcon::Live => {
            p.circle_filled(c, 2.4, theme::c().live);
            for (r, a) in [(5.0f32, 0.9f32), (8.0, 0.8)] {
                for side in [-1.0f32, 1.0] {
                    let pts: Vec<egui::Pos2> = (0..=8)
                        .map(|i| {
                            let t = -a + 2.0 * a * i as f32 / 8.0;
                            c + Vec2::new(side * r * t.cos(), r * t.sin())
                        })
                        .collect();
                    p.add(egui::Shape::line(pts, Stroke::new(1.3, col)));
                }
            }
        }
        BarIcon::Smiley => {
            let s = Stroke::new(1.4, col);
            p.circle_stroke(c, 7.5, s);
            p.circle_filled(c + Vec2::new(-2.7, -2.2), 1.2, col);
            p.circle_filled(c + Vec2::new(2.7, -2.2), 1.2, col);
            let pts: Vec<egui::Pos2> = (0..=10)
                .map(|i| {
                    let a = std::f32::consts::PI * (0.15 + 0.7 * i as f32 / 10.0);
                    c + Vec2::new(-4.2 * a.cos(), 0.8 + 3.4 * a.sin())
                })
                .collect();
            p.add(egui::Shape::line(pts, s));
        }
        BarIcon::Plus => {
            let s = Stroke::new(1.4, col);
            p.line_segment([c + Vec2::new(-5.0, 0.0), c + Vec2::new(5.0, 0.0)], s);
            p.line_segment([c + Vec2::new(0.0, -5.0), c + Vec2::new(0.0, 5.0)], s);
        }
        BarIcon::Sword => {
            // blade to the top right, crossguard and grip at the bottom left
            let guard = c + Vec2::new(-2.5, 2.5);
            let s = Stroke::new(1.5, col);
            p.line_segment([guard, c + Vec2::new(6.0, -6.0)], s);
            p.line_segment(
                [guard + Vec2::new(-3.0, -3.0), guard + Vec2::new(3.0, 3.0)],
                s,
            );
            p.line_segment([guard, c + Vec2::new(-6.0, 6.0)], Stroke::new(2.2, col));
        }
        BarIcon::Gear => {
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let d = Vec2::new(a.cos(), a.sin());
                p.line_segment([c + d * 4.5, c + d * 7.5], Stroke::new(2.6, col));
            }
            p.circle_stroke(c, 5.0, Stroke::new(2.0, col));
            p.circle_filled(c, 1.6, col);
        }
    }
    resp
}

/// A small platform badge (Twitch, Kick, YouTube, TikTok) in the platform's colour:
/// Twitch's speech bubble, Kick's "K", YouTube's play button, TikTok's note.
pub fn platform_badge(
    ui: &mut egui::Ui,
    p: crate::platforms::Platform,
    size: f32,
) -> egui::Response {
    use crate::platforms::Platform;
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let painter = ui.painter();
    let r = rect.shrink(0.5);
    let (bg, fg) = match p {
        Platform::Twitch => (Color32::from_rgb(0x91, 0x46, 0xff), Color32::WHITE),
        Platform::Kick => (Color32::from_rgb(0x53, 0xfc, 0x18), Color32::BLACK),
        Platform::YouTube => (Color32::from_rgb(0xff, 0x00, 0x33), Color32::WHITE),
        Platform::TikTok => (Color32::from_rgb(0x11, 0x11, 0x11), Color32::WHITE),
    };
    painter.rect_filled(r, size * 0.22, bg);
    let c = r.center();
    let u = size / 16.0;
    match p {
        Platform::Twitch => {
            // the glitch: a bubble with two eyes
            let b = egui::Rect::from_center_size(
                c + Vec2::new(0.0, -0.5 * u),
                Vec2::new(8.5 * u, 7.5 * u),
            );
            painter.rect_stroke(
                b,
                0.5 * u,
                Stroke::new(1.3 * u, fg),
                egui::StrokeKind::Inside,
            );
            painter.line_segment(
                [
                    c + Vec2::new(-1.5 * u, -2.0 * u),
                    c + Vec2::new(-1.5 * u, 0.5 * u),
                ],
                Stroke::new(1.3 * u, fg),
            );
            painter.line_segment(
                [
                    c + Vec2::new(1.5 * u, -2.0 * u),
                    c + Vec2::new(1.5 * u, 0.5 * u),
                ],
                Stroke::new(1.3 * u, fg),
            );
            painter.line_segment(
                [
                    b.left_bottom() + Vec2::new(2.0 * u, 0.0),
                    b.left_bottom() + Vec2::new(2.0 * u, 2.2 * u),
                ],
                Stroke::new(1.3 * u, fg),
            );
        }
        Platform::Kick => {
            painter.text(
                c + Vec2::new(0.0, 0.5 * u),
                egui::Align2::CENTER_CENTER,
                "K",
                egui::FontId::new(11.0 * u, bold()),
                fg,
            );
        }
        Platform::YouTube => {
            let pts = vec![
                c + Vec2::new(-2.2 * u, -3.0 * u),
                c + Vec2::new(3.4 * u, 0.0),
                c + Vec2::new(-2.2 * u, 3.0 * u),
            ];
            painter.add(egui::Shape::convex_polygon(pts, fg, Stroke::NONE));
        }
        Platform::TikTok => {
            // a note in TikTok's cyan and pink, then white
            let note = |off: Vec2, col: Color32| {
                let s = Stroke::new(1.6 * u, col);
                painter.line_segment(
                    [
                        c + off + Vec2::new(0.8 * u, -4.5 * u),
                        c + off + Vec2::new(0.8 * u, 2.2 * u),
                    ],
                    s,
                );
                painter.circle_stroke(c + off + Vec2::new(-1.0 * u, 2.4 * u), 1.8 * u, s);
                painter.line_segment(
                    [
                        c + off + Vec2::new(0.8 * u, -4.5 * u),
                        c + off + Vec2::new(3.4 * u, -2.6 * u),
                    ],
                    s,
                );
            };
            note(
                Vec2::new(-0.6 * u, 0.4 * u),
                Color32::from_rgb(0x25, 0xf4, 0xee),
            );
            note(
                Vec2::new(0.6 * u, -0.4 * u),
                Color32::from_rgb(0xfe, 0x2c, 0x55),
            );
            note(Vec2::ZERO, fg);
        }
    }
    resp
}

/// One emote button in the emote menu; returns true when clicked.
/// Wide emotes keep their shape (at most 28 px tall, 84 px wide) instead of being squeezed
/// into a square. Returns the button (click: add; right-click: the emote's menu).
pub fn emote_button(ui: &mut egui::Ui, e: &emotes::Emote) -> egui::Response {
    let img = egui::Image::new(e.url.as_str())
        .fit_to_original_size(1.0)
        .max_height(28.0)
        .max_width(84.0);
    let r = ui.add(
        egui::Button::image(img)
            .frame(false)
            .min_size(Vec2::new(30.0, 30.0)),
    );
    r.on_hover_text(format!("{}\n{}", e.name, e.provider.label()))
}
