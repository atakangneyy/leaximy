//! The words of the playing track, in a side panel that follows the song.

use egui::{Align, Color32, Frame, Layout, Margin, Rect, Sense, UiBuilder, pos2, vec2};

use crate::app::App;
use crate::i18n::{gettext, pgettext};
use crate::model::{Action, Loadable};
use crate::theme::{self, Icon};

use super::widgets;

const LINE_SIZE: f32 = 19.0;
const LINE_GAP: f32 = 10.0;
/// Where the line being sung sits, as a fraction of the visible lyrics from
/// the top: high up, so the lines to come fill most of the view.
const SUNG_LINE_AT: f32 = 0.2;

/// Scrolls so the middle of `line` sits `SUNG_LINE_AT` of the way down the
/// visible lyrics.
fn show_sung_line(ui: &egui::Ui, line: Rect, animation: Option<egui::style::ScrollAnimation>) {
    let above = (ui.clip_rect().height() * SUNG_LINE_AT - line.height() / 2.0).max(0.0);
    let target = Rect::from_min_max(pos2(line.left(), line.top() - above), line.max);
    match animation {
        Some(animation) => ui.scroll_to_rect_animation(target, Some(Align::Min), animation),
        None => ui.scroll_to_rect(target, Some(Align::Min)),
    }
}
/// How long a line takes to light up or fade.
const LIGHT_UP_SECONDS: f32 = 0.22;

fn blend(from: egui::Color32, to: egui::Color32, t: f32) -> egui::Color32 {
    let t = t.clamp(0.0, 1.0);
    egui::Color32::from(egui::Rgba::from(from) * (1.0 - t) + egui::Rgba::from(to) * t)
}

pub fn side_panel(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    let panel = egui::Panel::right("lyrics-panel")
        .resizable(true)
        .default_size(app.settings.lyrics_width)
        .size_range(theme::SIDE_PANEL_MIN_WIDTH..=640.0)
        .show_separator_line(false)
        .frame(
            Frame::new()
                .fill(palette.panel)
                .inner_margin(Margin::symmetric(12, 12)),
        );
    let response = panel.show(ui, |ui| {
        let window_controls = super::window_controls_reservation(
            ui.ctx(),
            app.show_queue_panel,
            app.show_lyrics_panel,
            ui.available_width(),
        );
        ui.add_space(window_controls.lyrics_top);
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            theme::text(
                ui,
                gettext(app.locale, "Lyrics"),
                theme::bold(18.0),
                palette.text,
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if theme::icon_button(
                    ui,
                    Icon::X,
                    18.0,
                    palette.secondary,
                    palette.text,
                    &gettext(app.locale, "Close"),
                )
                .clicked()
                {
                    app.actions.push(Action::ToggleLyricsPanel);
                }
                if theme::icon_button(
                    ui,
                    Icon::Expand,
                    18.0,
                    palette.secondary,
                    palette.text,
                    &gettext(app.locale, "Full screen lyrics"),
                )
                .clicked()
                {
                    app.actions.push(Action::SetLyricsFullscreen(true));
                }
                let loaded = matches!(&app.lyrics, Loadable::Loaded(Some(_)));
                if loaded
                    && !app.lyrics_following
                    && theme::pill_button(
                        ui,
                        &palette,
                        &pgettext(app.locale, "lyrics", "Follow"),
                        false,
                    )
                    .clicked()
                {
                    app.lyrics_following = true;
                    app.lyrics_line_shown = None;
                }
            });
        });
        ui.add_space(8.0);
        contents(app, ui);
    });
    let current_width = response.response.rect.width();
    if (app.settings.lyrics_width - current_width).abs() > 1.0 {
        app.settings.lyrics_width = current_width;
        app.actions.push(Action::SettingsChanged);
    }
}

fn contents(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    let Some(now) = app.now_playing() else {
        widgets::empty_state(
            ui,
            &palette,
            Icon::Mic,
            &gettext(app.locale, "Nothing playing"),
            &gettext(app.locale, "Play a song to see its lyrics."),
        );
        return;
    };
    let lyrics = match &app.lyrics {
        Loadable::NotLoaded | Loadable::Loading => {
            widgets::loading_row(ui, &palette, app.locale);
            return;
        }
        Loadable::Failed(error) => {
            let message = gettext(
                app.locale,
                // Translators: Keep {error} unchanged. It is the original failure detail.
                "Couldn't fetch the lyrics: {error}",
            )
            .replace("{error}", error);
            ui.add_space(8.0);
            theme::text(ui, message, theme::regular(13.0), palette.secondary);
            ui.add_space(8.0);
            if theme::pill_button(ui, &palette, &gettext(app.locale, "Try again"), false).clicked()
            {
                app.request_lyrics();
            }
            return;
        }
        Loadable::Loaded(None) => {
            widgets::empty_state(
                ui,
                &palette,
                Icon::Mic,
                &gettext(app.locale, "No lyrics"),
                &gettext(app.locale, "No lyrics found for this track."),
            );
            return;
        }
        Loadable::Loaded(Some(lyrics)) if lyrics.instrumental => {
            widgets::empty_state(
                ui,
                &palette,
                Icon::Music,
                &gettext(app.locale, "Instrumental"),
                &gettext(app.locale, "No timed lyrics for this track."),
            );
            return;
        }
        Loadable::Loaded(Some(lyrics)) => lyrics.clone(),
    };

    let active = lyrics.active_line(now.position_ms);
    let follow = app.lyrics_following && app.lyrics_line_shown != Some(active);
    // The line being sung is bold and in the accent colour; every other
    // line is quiet, regular text, the same before and after it has been
    // sung. A line takes 220 ms to light up or fade, as in omarchy-lyrics.
    let quiet = palette.text.gamma_multiply(0.45);
    let scroll = crate::autoscroll::show(
        ui,
        egui::ScrollArea::vertical()
            .id_salt("lyrics-scroll")
            .auto_shrink([false, false]),
        egui::Vec2b::new(false, true),
        |ui| {
            // Before the first line there is nothing to highlight, so the
            // panel sits at the top rather than wherever it was left.
            if follow && lyrics.synced && active.is_none() {
                let top = ui.cursor().min;
                ui.scroll_to_rect(
                    egui::Rect::from_min_size(top, egui::vec2(1.0, 1.0)),
                    Some(Align::Min),
                );
            }
            ui.add_space(12.0);
            for (index, line) in lyrics.lines.iter().enumerate() {
                let is_active = active == Some(index);
                let lit = ui.ctx().animate_bool_with_time(
                    egui::Id::new("lyric-line").with(index),
                    is_active,
                    LIGHT_UP_SECONDS,
                );
                let color = blend(quiet, palette.accent, lit);
                let font = if lit > 0.5 {
                    theme::bold(LINE_SIZE)
                } else {
                    theme::regular(LINE_SIZE)
                };
                // A timed line with no words is the band playing on.
                let text = if line.text.is_empty() && lyrics.synced {
                    "\u{266a}"
                } else {
                    line.text.as_str()
                };
                let sense = if lyrics.synced {
                    Sense::click()
                } else {
                    Sense::hover()
                };
                let response = if crate::bidi::is_rtl(text) {
                    let galley = crate::bidi::layout(
                        ui.painter(),
                        text,
                        font,
                        color,
                        ui.available_width(),
                        usize::MAX,
                        None,
                    );
                    ui.add(egui::Label::new(galley).sense(sense))
                } else {
                    ui.add(
                        egui::Label::new(egui::RichText::new(text).font(font).color(color))
                            .sense(sense),
                    )
                };
                crate::autoscroll::row(ui, &response);
                let rect = response.rect;
                if lyrics.synced
                    && response.clicked()
                    && let Some(at_ms) = line.at_ms
                {
                    app.actions.push(Action::Seek(at_ms));
                    app.lyrics_following = true;
                }
                if is_active && follow {
                    show_sung_line(ui, rect, None);
                }
                ui.add_space(LINE_GAP);
            }
            // Words without timing can only be followed by the clock: sit
            // at the part of the text the song is probably at.
            if app.lyrics_following && !lyrics.synced && now.duration_ms > 0 {
                let fraction =
                    (f64::from(now.position_ms) / f64::from(now.duration_ms)).clamp(0.0, 1.0);
                let content = ui.min_rect();
                let y = content.top() + content.height() * fraction as f32;
                ui.scroll_to_rect(
                    egui::Rect::from_min_max(
                        egui::pos2(content.left(), y),
                        egui::pos2(content.right(), y + 1.0),
                    ),
                    Some(Align::Center),
                );
            }
            // Room for the last line to rise to where a sung line sits.
            ui.add_space((ui.clip_rect().height() * (1.0 - SUNG_LINE_AT)).max(60.0));
        },
    );
    crate::autoscroll::lyrics(ui, scroll.id);
    // Scrolling by hand means the reader wants to look elsewhere; the
    // Follow button in the header picks the song back up.
    if ui.rect_contains_pointer(scroll.inner_rect)
        && ui.input(|input| input.smooth_scroll_delta.y != 0.0)
    {
        app.lyrics_following = false;
    }
    app.lyrics_line_shown = Some(active);
}

pub fn fullscreen(app: &mut App, ui: &mut egui::Ui) {
    egui::CentralPanel::default()
        .frame(Frame::new().fill(theme::Palette::dark().window))
        .show(ui, |ui| {
            let rect = ui.max_rect();
            background(app, ui, rect);
            let top = theme::titlebar_inset(ui.ctx()) + 24.0;
            if app.now_playing().is_some() && rect.width() >= COVER_BESIDE_MIN_WIDTH {
                with_cover(app, ui, rect, top);
                return;
            }
            let width = fullscreen_content_width(rect.width());
            let region = Rect::from_min_max(
                pos2(rect.center().x - width / 2.0, rect.top() + top),
                pos2(rect.center().x + width / 2.0, rect.bottom()),
            );
            let mut content = ui.new_child(UiBuilder::new().max_rect(region));
            fullscreen_header(app, &mut content);
            content.add_space(20.0);
            track_heading(app, &mut content);
            content.add_space(16.0);
            fullscreen_contents(app, &mut content);
        });
}

/// The words of the line being sung, in capitals, and how far through them
/// the song is at `position_ms`, counted in words (2.5 is halfway through
/// the third). Lyrics are timed by line, so the words
/// are spread over the line's time: an estimate of when each is sung.
fn sung_words(lyrics: &crate::lyrics::Lyrics, position_ms: u32) -> Option<(Vec<String>, f32)> {
    let index = lyrics.active_line(position_ms)?;
    let line = lyrics.lines.get(index)?;
    let start = line.at_ms?;
    let words: Vec<&str> = line.text.split_whitespace().collect();
    if words.is_empty() {
        return None;
    }
    let until_next = lyrics
        .lines
        .get(index + 1)
        .and_then(|next| next.at_ms)
        .map_or(4000, |next| next.saturating_sub(start));
    // A line is sung in about half a second a word, however long the gap
    // before the next one.
    let span = until_next.clamp(400, 450 * words.len() as u32 + 600);
    let progress = position_ms.saturating_sub(start) as f32 / span as f32;
    let reached = (progress * words.len() as f32).min(words.len() as f32);
    Some((words.iter().map(|word| shout(word)).collect(), reached))
}

/// Whether `word` (in capitals) is a small connecting word, drawn small
/// between the large ones.
fn minor_word(word: &str) -> bool {
    const MINOR: &[&str] = &[
        "I", "A", "AN", "THE", "IN", "ON", "MY", "IS", "AM", "OF", "TO", "AT", "IT", "ME", "WE",
        "BE", "SO", "OR", "IF", "AS", "BY", "DO", "UP", "US", "HE", "VE", "BİR", "BU", "DE", "DA",
        "Kİ", "Mİ", "MI", "MU", "MÜ", "İLE", "BEN", "SEN", "O", "NE", "YA",
    ];
    let bare: String = word.chars().filter(|c| c.is_alphanumeric()).collect();
    MINOR.contains(&bare.as_str())
}

/// `text` in capitals, with Turkish dotted and dotless i kept apart when the
/// line reads as Turkish.
fn shout(text: &str) -> String {
    let turkish = text.chars().any(|c| "ğşıİĞŞ".contains(c));
    if turkish {
        text.replace('i', "İ").replace('ı', "I").to_uppercase()
    } else {
        text.to_uppercase()
    }
}

/// The desktop visualizer, behind the desktop icons: the user's own
/// wallpaper, the line being sung in large capitals on the left, appearing
/// word by word, the round cover with its ring of bars right of the middle,
/// and the songs up next in the lower left. Nothing in it takes input.
pub fn wallpaper(app: &mut App, ui: &mut egui::Ui) {
    egui::CentralPanel::default()
        .frame(Frame::new().fill(Color32::BLACK))
        .show(ui, |ui| {
            let rect = ui.max_rect();
            if let Some(texture) = app.desktop_backdrop(ui.ctx()) {
                theme::apply_local(ui, &theme::Palette::dark());
                let painter = ui.painter().with_clip_rect(rect);
                painter.image(
                    texture.id(),
                    rect,
                    cover_uv(rect.size(), texture.size_vec2()),
                    Color32::WHITE,
                );
                // A light veil so white words read on a bright picture.
                painter.rect_filled(rect, 0.0, Color32::from_black_alpha(50));
            } else {
                background(app, ui, rect);
            }
            let Some(now) = app.now_playing() else {
                return;
            };

            // The disc, right of the middle.
            let side = (rect.height() * 0.46)
                .min(rect.width() * 0.3)
                .clamp(200.0, 640.0);
            let centre = pos2(rect.left() + rect.width() * 0.66, rect.center().y);
            let column = Rect::from_center_size(centre, vec2(side, side + 90.0));
            big_cover(app, ui, column, Align::Center);

            // The words, left of the disc.
            let left = rect.left() + rect.width() * 0.06;
            let wrap = (column.left() - 48.0 - left).max(200.0);
            let words = match &app.lyrics {
                Loadable::Loaded(Some(lyrics)) => sung_words(lyrics, now.position_ms),
                _ => None,
            };
            if let Some((words, _reached)) = words {
                // Stacked rows centred on one point, one large word to a
                // row with the small connecting words tucked beside it.
                let big = theme::bold((rect.height() * 0.075).clamp(32.0, 92.0));
                let small = theme::semibold(big.size * 0.46);
                let accent = theme::Palette::dark().accent;
                let painter = ui.painter().with_clip_rect(rect);
                let laid: Vec<_> = words
                    .iter()
                    .map(|word| {
                        let minor = minor_word(word);
                        let galley = if minor {
                            painter.layout_no_wrap(word.clone(), small.clone(), accent)
                        } else {
                            painter.layout_no_wrap(word.clone(), big.clone(), Color32::WHITE)
                        };
                        (galley, minor)
                    })
                    .collect();
                let gap = big.size * 0.22;
                let mut rows: Vec<Vec<usize>> = vec![Vec::new()];
                let (mut width, mut majors) = (0.0_f32, 0);
                for (index, (galley, minor)) in laid.iter().enumerate() {
                    let wide = galley.size().x;
                    let full = !rows.last().is_some_and(Vec::is_empty)
                        && (width + gap + wide > wrap || (!*minor && majors >= 1));
                    if full {
                        rows.push(Vec::new());
                        width = 0.0;
                        majors = 0;
                    }
                    width += wide + if width > 0.0 { gap } else { 0.0 };
                    if !*minor {
                        majors += 1;
                    }
                    if let Some(row) = rows.last_mut() {
                        row.push(index);
                    }
                }
                let row_height = big.size * 1.08;
                let middle = left + wrap.min(rect.width() * 0.36) / 2.0;
                let top = rect.center().y - row_height * rows.len() as f32 / 2.0;
                for (line, row) in rows.iter().enumerate() {
                    let total: f32 = row.iter().map(|index| laid[*index].0.size().x).sum::<f32>()
                        + gap * row.len().saturating_sub(1) as f32;
                    let mut x = middle - total / 2.0;
                    let y = top + row_height * line as f32;
                    for index in row {
                        let (galley, minor) = &laid[*index];
                        let size = galley.size();
                        // The whole line shows at once, fully lit.
                        let colour = if *minor { accent } else { Color32::WHITE };
                        let drop = if *minor { (big.size - small.size) * 0.82 } else { 0.0 };
                        let at = pos2(x, y + drop);
                        painter.galley_with_override_text_color(
                            at + vec2(2.0, 3.0),
                            galley.clone(),
                            Color32::from_black_alpha(170),
                        );
                        painter.galley_with_override_text_color(at, galley.clone(), colour);
                        x += size.x + gap;
                    }
                }
            }

            // Up next, in the lower left.
            let upcoming: Vec<(String, String, Option<String>)> = app
                .queue
                .get()
                .map(|queue| {
                    queue
                        .queue
                        .iter()
                        .take(4)
                        .map(|item| {
                            (
                                item.name().to_owned(),
                                item.subtitle(),
                                item.image(64).map(str::to_owned),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            if !upcoming.is_empty() {
                let row = 58.0;
                let top = rect.bottom() - 96.0 - row * upcoming.len() as f32;
                let zone = Rect::from_min_max(
                    pos2(left, top - 28.0),
                    pos2(left + wrap.min(420.0), rect.bottom()),
                );
                let painter = ui.painter().with_clip_rect(zone);
                painter.text(
                    pos2(left, top - 26.0),
                    egui::Align2::LEFT_TOP,
                    gettext(app.locale, "Next up"),
                    theme::semibold(12.0),
                    theme::Palette::dark().accent,
                );
                for (index, (name, artists, art)) in upcoming.iter().enumerate() {
                    let y = top + row * index as f32;
                    let cover = Rect::from_min_size(pos2(left, y), vec2(46.0, 46.0));
                    widgets::paint_cover(
                        ui,
                        &theme::Palette::dark(),
                        art.as_deref(),
                        cover,
                        6.0,
                        Icon::Music,
                        Some(app.backend.art()),
                    );
                    painter.text(
                        pos2(cover.right() + 12.0, y + 4.0),
                        egui::Align2::LEFT_TOP,
                        name,
                        theme::semibold(15.0),
                        Color32::WHITE,
                    );
                    painter.text(
                        pos2(cover.right() + 12.0, y + 25.0),
                        egui::Align2::LEFT_TOP,
                        artists,
                        theme::regular(12.0),
                        Color32::from_gray(215),
                    );
                }
            }
            // The words move with the song, whoever plays it.
            if now.playing {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(120));
            }
        });
}

/// The widest the lyrics get beside the cover, so lines stay easy to read.
const LYRICS_BESIDE_WIDTH: f32 = 640.0;

/// The narrowest window that shows the cover beside the lyrics; narrower
/// ones keep a single column with a small cover in the heading.
const COVER_BESIDE_MIN_WIDTH: f32 = 900.0;

/// Full screen with the cover large: beside the lyrics when there are
/// words to follow, and alone in the middle when there are none, a calm
/// view of what is playing.
fn with_cover(app: &mut App, ui: &mut egui::Ui, rect: Rect, top: f32) {
    let outer = Rect::from_min_max(
        pos2(rect.left() + 48.0, rect.top() + top),
        pos2(rect.right() - 48.0, rect.bottom() - 40.0),
    );
    let mut header = ui.new_child(UiBuilder::new().max_rect(outer));
    fullscreen_header(app, &mut header);
    let below = Rect::from_min_max(
        pos2(outer.left(), header.min_rect().bottom() + 24.0),
        outer.max,
    );
    // The cover moves aside only for words to read. While they load it
    // stays in the middle, so a song that turns out to have none never
    // moves at all.
    let words = matches!(&app.lyrics, Loadable::Loaded(Some(lyrics)) if !lyrics.instrumental);
    if words {
        // The cover and the lyrics are one group, centred in the window.
        let gap = 64.0;
        let side = (below.width() * 0.38)
            .min(below.height() - 90.0)
            .clamp(200.0, 520.0);
        let lyrics_width = (below.width() - side - gap).min(LYRICS_BESIDE_WIDTH);
        let left = below.center().x - (side + gap + lyrics_width) / 2.0;
        let column = Rect::from_min_size(
            pos2(left, below.center().y - (side + 90.0) / 2.0),
            vec2(side, side + 90.0),
        );
        big_cover(app, ui, column, Align::Min);
        let lyrics = Rect::from_min_max(
            pos2(column.right() + gap, below.top()),
            pos2(column.right() + gap + lyrics_width, below.bottom()),
        );
        let mut content = ui.new_child(UiBuilder::new().max_rect(lyrics));
        fullscreen_contents(app, &mut content);
    } else {
        let side = (below.height() - 140.0)
            .min(below.width() * 0.5)
            .clamp(200.0, 560.0);
        let column = Rect::from_center_size(below.center(), vec2(side, side + 90.0));
        big_cover(app, ui, column, Align::Center);
        // Why there are no words, quietly, under the song, or that they
        // are still being fetched.
        let (heading, detail) = match &app.lyrics {
            Loadable::Loaded(Some(_)) => (
                gettext(app.locale, "Instrumental"),
                gettext(app.locale, "No timed lyrics for this track."),
            ),
            Loadable::Loaded(None) => (
                gettext(app.locale, "No lyrics"),
                gettext(app.locale, "No lyrics found for this track."),
            ),
            Loadable::Failed(error) => (
                gettext(
                    app.locale,
                    // Translators: Keep {error} unchanged. It is the original failure detail.
                    "Couldn't fetch the lyrics: {error}",
                )
                .replace("{error}", error)
                .into(),
                Default::default(),
            ),
            Loadable::NotLoaded | Loadable::Loading => {
                (gettext(app.locale, "Loading…"), Default::default())
            }
        };
        let heading = ui.painter().text(
            pos2(column.center().x, column.bottom() + 8.0),
            egui::Align2::CENTER_TOP,
            heading,
            theme::semibold(13.0),
            Color32::from_gray(200),
        );
        if matches!(app.lyrics, Loadable::Failed(_)) {
            let retry = Rect::from_center_size(
                pos2(column.center().x, heading.bottom() + 24.0),
                vec2(column.width(), 32.0),
            );
            let mut retry_ui = ui.new_child(
                UiBuilder::new()
                    .max_rect(retry)
                    .layout(Layout::top_down(Align::Center)),
            );
            let label = gettext(app.locale, "Try again");
            if theme::pill_button(&mut retry_ui, &theme::Palette::dark(), &label, false).clicked() {
                app.actions.push(Action::RetryLyrics);
            }
            return;
        }
        ui.painter().text(
            pos2(column.center().x, heading.bottom() + 4.0),
            egui::Align2::CENTER_TOP,
            detail,
            theme::regular(13.0),
            Color32::from_gray(170),
        );
    }
}

/// How much of the cover's square the round cover fills; the rest is room
/// for the ring of bars around it.
const DISC_FILL: f32 = 0.7;

/// How often the ring of bars is redrawn while a song sounds.
const RING_FRAME: std::time::Duration = std::time::Duration::from_micros(16_667);

/// The playing song's cover as a round disc in the top of `column`, ringed
/// by the spectrum of the sound (bass at the top, mirrored down both sides)
/// and swelling a little with the bass, with its title and artists beneath,
/// aligned to its left edge or centred. Like every visualizer it shows the
/// sound after the equalizer and before the volume, and only moves for a
/// song playing on this computer.
fn big_cover(app: &mut App, ui: &mut egui::Ui, column: Rect, align: Align) {
    use crate::vis;
    let Some(now) = app.now_playing() else {
        return;
    };
    let side = column.width();
    let square = Rect::from_min_size(column.min, vec2(side, side));

    let sounding = (now.playing || now.loading) && now.local;
    let moving = sounding || !app.cover_ring_analyser.settled();
    let levels = if moving {
        let samples = if sounding {
            app.winamp.tap.window(vis::WIDE_SAMPLES, vis::LAG)
        } else {
            Vec::new()
        };
        app.cover_ring_analyser
            .step(&samples, std::time::Instant::now())
    } else {
        [0.0; vis::WIDE_BANDS]
    };
    if moving {
        ui.ctx().request_repaint_after(RING_FRAME);
    }
    let low = (levels.len() / 8).max(1);
    let bass = levels.iter().take(low).sum::<f32>() / low as f32;

    let centre = square.center();
    let disc = side * DISC_FILL * (1.0 + 0.035 * bass);
    let cover = Rect::from_center_size(centre, vec2(disc, disc));
    let radius = disc / 2.0;

    // The ring: one bar per band on each side, from the top round to the
    // bottom, amber at rest and whitening as it rises.
    let accent = theme::Palette::dark().accent;
    let reach = (side / 2.0 - radius - 6.0).max(0.0);
    let bands = levels.len();
    let thickness = (std::f32::consts::PI * radius / bands as f32 * 0.55).clamp(1.5, 5.0);
    let painter = ui.painter().with_clip_rect(square.expand(4.0));
    for (index, level) in levels.iter().enumerate() {
        let length = 2.0 + level * reach;
        let colour = accent
            .lerp_to_gamma(Color32::WHITE, *level)
            .gamma_multiply(0.55 + 0.45 * level);
        let turn = (index as f32 + 0.5) / bands as f32 * std::f32::consts::PI;
        for side_sign in [-1.0_f32, 1.0] {
            let angle = side_sign * turn - std::f32::consts::FRAC_PI_2;
            let direction = vec2(angle.cos(), angle.sin());
            painter.line_segment(
                [
                    centre + direction * (radius + 4.0),
                    centre + direction * (radius + 4.0 + length),
                ],
                egui::Stroke::new(thickness, colour),
            );
        }
    }

    ui.painter().add(
        egui::epaint::Shadow {
            offset: [0, 18],
            blur: 48,
            spread: 0,
            color: Color32::from_black_alpha(140),
        }
        .as_shape(cover, radius),
    );
    widgets::paint_cover(
        ui,
        &theme::Palette::dark(),
        now.art_url.as_deref().or(now.art_small.as_deref()),
        cover,
        radius,
        Icon::Music,
        Some(app.backend.art()),
    );
    // A thin amber edge, brighter with the bass.
    ui.painter().circle_stroke(
        centre,
        radius,
        egui::Stroke::new(1.5, accent.gamma_multiply(0.35 + 0.65 * bass)),
    );
    let cover = square;
    let words = Rect::from_min_max(pos2(column.left(), cover.bottom() + 18.0), column.max);
    let mut text = ui.new_child(
        UiBuilder::new()
            .max_rect(words)
            .layout(Layout::top_down(align)),
    );
    text.spacing_mut().item_spacing.y = 4.0;
    text.add(
        egui::Label::new(
            egui::RichText::new(&now.title)
                .font(theme::semibold(22.0))
                .color(Color32::WHITE),
        )
        .truncate(),
    );
    text.add(
        egui::Label::new(
            egui::RichText::new(&now.subtitle)
                .font(theme::regular(14.0))
                .color(Color32::from_gray(225)),
        )
        .truncate(),
    );
}

fn fullscreen_content_width(viewport_width: f32) -> f32 {
    let available = (viewport_width - 48.0).max(0.0);
    (viewport_width * 0.72).clamp(400.0, 960.0).min(available)
}

fn preferred_backdrop_art(small: Option<String>, large: Option<String>) -> Option<String> {
    small.or(large)
}

fn background(app: &mut App, ui: &mut egui::Ui, rect: Rect) {
    theme::apply_local(ui, &theme::Palette::dark());
    let art = app
        .now_playing()
        .and_then(|now| preferred_backdrop_art(now.art_small, now.art_url));
    let painter = ui.painter().with_clip_rect(rect);
    if let Some(texture) = app
        .lyrics_backdrop
        .texture(ui.ctx(), app.backend.art(), art.as_deref())
    {
        painter.image(
            texture.id(),
            rect,
            cover_uv(rect.size(), texture.size_vec2()),
            Color32::from_gray(180),
        );
    }
    painter.rect_filled(rect, 0.0, Color32::from_black_alpha(120));
    widgets::paint_vertical_gradient(
        ui,
        rect,
        Color32::from_black_alpha(0),
        Color32::from_black_alpha(95),
    );
}

fn cover_uv(view: egui::Vec2, image: egui::Vec2) -> Rect {
    let ratio = (view.x / view.y.max(1.0)) / (image.x / image.y.max(1.0));
    let size = if ratio > 1.0 {
        vec2(1.0, 1.0 / ratio)
    } else {
        vec2(ratio, 1.0)
    };
    Rect::from_center_size(pos2(0.5, 0.5), size)
}

fn fullscreen_header(app: &mut App, ui: &mut egui::Ui) {
    let palette = theme::Palette::dark();
    ui.horizontal(|ui| {
        theme::text(
            ui,
            gettext(app.locale, "Lyrics"),
            theme::bold(18.0),
            palette.text,
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if theme::icon_button(
                ui,
                Icon::Shrink,
                18.0,
                palette.text,
                palette.text,
                &gettext(app.locale, "Leave full screen (Esc)"),
            )
            .clicked()
            {
                app.actions.push(Action::SetLyricsFullscreen(false));
            }
            let loaded = matches!(&app.lyrics, Loadable::Loaded(Some(_)));
            if loaded
                && !app.lyrics_following
                && theme::pill_button(
                    ui,
                    &palette,
                    &pgettext(app.locale, "lyrics", "Follow"),
                    false,
                )
                .clicked()
            {
                app.actions.push(Action::FollowLyrics);
            }
        });
    });
}

fn track_heading(app: &App, ui: &mut egui::Ui) {
    if let Some(now) = app.now_playing() {
        ui.horizontal(|ui| {
            let size = 52.0;
            let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
            widgets::paint_cover(
                ui,
                &theme::Palette::dark(),
                now.art_small.as_deref().or(now.art_url.as_deref()),
                rect,
                4.0,
                Icon::Music,
                Some(app.backend.art()),
            );
            ui.vertical(|ui| {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(&now.title)
                            .font(theme::semibold(22.0))
                            .color(Color32::WHITE),
                    )
                    .truncate(),
                );
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(&now.subtitle)
                            .font(theme::regular(13.0))
                            .color(Color32::from_gray(235)),
                    )
                    .truncate(),
                );
            });
        });
    }
}

fn fullscreen_contents(app: &mut App, ui: &mut egui::Ui) {
    let palette = theme::Palette::dark();
    let Some(now) = app.now_playing() else {
        widgets::empty_state(
            ui,
            &palette,
            Icon::Mic,
            &gettext(app.locale, "Nothing playing"),
            &gettext(app.locale, "Play a song to see its lyrics."),
        );
        return;
    };
    let lyrics = match &app.lyrics {
        Loadable::NotLoaded | Loadable::Loading => {
            widgets::loading_row(ui, &palette, app.locale);
            return;
        }
        Loadable::Failed(error) => {
            let message = gettext(
                app.locale,
                // Translators: Keep {error} unchanged. It is the original failure detail.
                "Couldn't fetch the lyrics: {error}",
            )
            .replace("{error}", error);
            ui.add_space(8.0);
            theme::text(ui, message, theme::regular(13.0), palette.text);
            ui.add_space(8.0);
            if theme::pill_button(ui, &palette, &gettext(app.locale, "Try again"), false).clicked()
            {
                app.actions.push(Action::RetryLyrics);
            }
            return;
        }
        Loadable::Loaded(None) => {
            widgets::empty_state(
                ui,
                &palette,
                Icon::Mic,
                &gettext(app.locale, "No lyrics"),
                &gettext(app.locale, "No lyrics found for this track."),
            );
            return;
        }
        Loadable::Loaded(Some(lyrics)) if lyrics.instrumental => {
            widgets::empty_state(
                ui,
                &palette,
                Icon::Music,
                &gettext(app.locale, "Instrumental"),
                &gettext(app.locale, "No timed lyrics for this track."),
            );
            return;
        }
        Loadable::Loaded(Some(lyrics)) => lyrics.clone(),
    };

    let active = lyrics.active_line(now.position_ms);
    // A Follow click resets the remembered line after drawing. Other frames
    // record the shown line before any line-click action restores following.
    if !app
        .actions
        .iter()
        .any(|action| matches!(action, Action::FollowLyrics))
    {
        app.actions.push(Action::LyricsLineShown(active));
    }
    let viewport = ui.available_rect_before_wrap();
    let manual_scroll = ui.rect_contains_pointer(viewport)
        && ui.input(|input| {
            input.smooth_scroll_delta.y != 0.0
                || (input.pointer.primary_down() && input.pointer.delta().y != 0.0)
        });
    let following = app.lyrics_following && !manual_scroll;
    let follow = following && app.lyrics_line_shown != Some(active);
    let animation = egui::style::ScrollAnimation::duration(0.45);
    let size = (ui.available_width() * 0.046).clamp(28.0, 42.0);
    // The line being sung brightens; all lines keep the same font metrics
    // so highlighting cannot rewrap the words during a transition.
    // A line takes 300 ms to light up or fade.
    let quiet = palette.text.gamma_multiply(0.68);
    ui.spacing_mut().scroll.fade.strength = 0.0;
    egui::ScrollArea::vertical()
        .id_salt(("fullscreen-lyrics-scroll", &now.uri))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // Before the first line there is nothing to highlight, so the
            // panel sits at the top rather than wherever it was left.
            if follow && lyrics.synced && active.is_none() {
                let top = ui.cursor().min;
                ui.scroll_to_rect_animation(
                    egui::Rect::from_min_size(top, egui::vec2(1.0, 1.0)),
                    Some(Align::Min),
                    animation,
                );
            }
            // Room for the first line to sit where a sung line sits, and for
            // the last to rise to it.
            let (padding, below) = if lyrics.synced {
                (
                    (viewport.height() * SUNG_LINE_AT - size).max(12.0),
                    viewport.height() * (1.0 - SUNG_LINE_AT),
                )
            } else {
                (12.0, 60.0)
            };
            ui.add_space(padding);
            for (index, line) in lyrics.lines.iter().enumerate() {
                let is_active = active == Some(index);
                let lit = ui.ctx().animate_bool_with_time(
                    egui::Id::new("lyric-line").with(("fullscreen", &now.uri, index)),
                    is_active,
                    0.3,
                );
                let color = if lyrics.synced {
                    blend(quiet, palette.text, lit)
                } else {
                    palette.text
                };
                let font = theme::bold(size);
                // A timed line with no words is the band playing on.
                let text = if line.text.is_empty() && lyrics.synced {
                    "\u{266a}"
                } else {
                    line.text.as_str()
                };
                let sense = if lyrics.synced {
                    Sense::click()
                } else {
                    Sense::hover()
                };
                let galley = crate::bidi::layout(
                    ui.painter(),
                    text,
                    font,
                    color,
                    ui.available_width(),
                    usize::MAX,
                    None,
                );
                let center = ui.cursor().top() + galley.size().y * 0.5;
                let edge = ((center - viewport.top()).min(viewport.bottom() - center)
                    / (size * 2.0))
                    .clamp(0.0, 1.0);
                let response = ui
                    .scope(|ui| {
                        ui.multiply_opacity(edge * edge * (3.0 - 2.0 * edge));
                        ui.add(egui::Label::new(galley).sense(sense))
                    })
                    .inner;
                let rect = response.rect;
                if lyrics.synced
                    && response.clicked()
                    && let Some(at_ms) = line.at_ms
                {
                    app.actions.push(Action::Seek(at_ms));
                    app.actions.push(Action::FollowLyrics);
                }
                if is_active && follow {
                    show_sung_line(ui, rect, Some(animation));
                }
                ui.add_space(27.0);
            }
            // Words without timing can only be followed by the clock: sit
            // at the part of the text the song is probably at.
            if following && !lyrics.synced && now.duration_ms > 0 {
                let fraction =
                    (f64::from(now.position_ms) / f64::from(now.duration_ms)).clamp(0.0, 1.0);
                let content = ui.min_rect();
                let y = content.top() + content.height() * fraction as f32;
                ui.scroll_to_rect_animation(
                    egui::Rect::from_min_max(
                        egui::pos2(content.left(), y),
                        egui::pos2(content.right(), y + 1.0),
                    ),
                    Some(Align::Center),
                    animation,
                );
            }
            ui.add_space(below.max(60.0));
        });
    // Scrolling by hand means the reader wants to look elsewhere; the
    // Follow button in the header picks the song back up.
    if manual_scroll && app.lyrics_following {
        app.actions.push(Action::PauseLyricsFollow);
    }
    if now.playing
        && lyrics.synced
        && let Some(next) = lyrics
            .lines
            .iter()
            .filter_map(|line| line.at_ms)
            .find(|at| *at > now.position_ms)
    {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(u64::from(
                next - now.position_ms,
            )));
    }
}

#[cfg(test)]
mod tests {
    use super::{fullscreen_content_width, preferred_backdrop_art};

    #[test]
    fn fullscreen_backdrop_prefers_small_art_with_large_art_as_fallback() {
        let small = "small".to_string();
        let large = "large".to_string();
        assert_eq!(
            preferred_backdrop_art(Some(small.clone()), Some(large.clone())),
            Some(small)
        );
        assert_eq!(
            preferred_backdrop_art(None, Some(large.clone())),
            Some(large)
        );
        assert_eq!(preferred_backdrop_art(None, None), None);
    }

    #[test]
    fn fullscreen_content_width_never_inverts_a_narrow_viewport() {
        for viewport_width in [0.0, 24.0, 47.0, 48.0, 64.0, 760.0, 2_000.0] {
            let width = fullscreen_content_width(viewport_width);
            assert!(width >= 0.0);
            assert!(width <= (viewport_width - 48.0).max(0.0));
        }
        assert_eq!(fullscreen_content_width(47.0), 0.0);
        assert_eq!(fullscreen_content_width(2_000.0), 960.0);
    }
}
