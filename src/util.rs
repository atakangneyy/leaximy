//! Small helpers shared across the application.

use std::borrow::Cow;

use crate::i18n::{Locale, gettext, ngettext, pgettext};

/// `3:45` for track lengths, `1:02:03` past an hour.
pub fn format_duration_ms(ms: u32) -> String {
    let total = ms / 1000;
    let hours = total / 3600;
    let minutes = (total / 60) % 60;
    let seconds = total % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// `2 hr 13 min` for playlist totals, `45 min 12 sec` under an hour.
pub fn format_total_ms(locale: Locale, ms: u64) -> String {
    let total = ms / 1000;
    let hours = total / 3600;
    let minutes = (total / 60) % 60;
    let seconds = total % 60;
    if hours > 0 {
        // Translators: A length of time, abbreviated. Keep {hours} and {minutes}.
        gettext(locale, "{hours} hr {minutes} min")
            .replace("{hours}", &hours.to_string())
            .replace("{minutes}", &minutes.to_string())
    } else if minutes > 0 {
        // Translators: A length of time, abbreviated. Keep {minutes} and {seconds}.
        gettext(locale, "{minutes} min {seconds} sec")
            .replace("{minutes}", &minutes.to_string())
            .replace("{seconds}", &seconds.to_string())
    } else {
        // Translators: A length of time, abbreviated. Keep {seconds}.
        gettext(locale, "{seconds} sec").replace("{seconds}", &seconds.to_string())
    }
}

/// Episode lengths read as `1 hr 12 min` or `38 min`.
pub fn format_episode_ms(locale: Locale, ms: u32) -> String {
    let minutes = ms / 60_000;
    let hours = minutes / 60;
    if hours > 0 {
        gettext(locale, "{hours} hr {minutes} min")
            .replace("{hours}", &hours.to_string())
            .replace("{minutes}", &(minutes % 60).to_string())
    } else {
        // Translators: A length of time, abbreviated. Keep {minutes}.
        gettext(locale, "{minutes} min").replace("{minutes}", &minutes.max(1).to_string())
    }
}

pub fn format_count(count: u64) -> String {
    let digits = count.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, character) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(character);
    }
    out
}

/// `Jan 5, 2024` from an ISO-8601 timestamp or a bare date.
pub fn format_date(locale: Locale, iso: &str) -> String {
    let date = iso.get(..10).unwrap_or(iso);
    let mut parts = date.split('-');
    let (Some(year), Some(month)) = (parts.next(), parts.next()) else {
        return iso.to_string();
    };
    let day = parts.next();
    let month_name = match month {
        "01" => pgettext(locale, "month", "Jan"),
        "02" => pgettext(locale, "month", "Feb"),
        "03" => pgettext(locale, "month", "Mar"),
        "04" => pgettext(locale, "month", "Apr"),
        "05" => pgettext(locale, "month", "May"),
        "06" => pgettext(locale, "month", "Jun"),
        "07" => pgettext(locale, "month", "Jul"),
        "08" => pgettext(locale, "month", "Aug"),
        "09" => pgettext(locale, "month", "Sep"),
        "10" => pgettext(locale, "month", "Oct"),
        "11" => pgettext(locale, "month", "Nov"),
        "12" => pgettext(locale, "month", "Dec"),
        _ => return iso.to_string(),
    };
    let dated = match day.and_then(|day| day.trim_start_matches('0').parse::<u8>().ok()) {
        Some(day) => {
            // Translators: A date. {month} is an abbreviated month name; reorder as your language writes dates.
            gettext(locale, "{month} {day}, {year}").replace("{day}", &day.to_string())
        }
        // Translators: A month and year. {month} is an abbreviated month name.
        None => gettext(locale, "{month} {year}").into_owned(),
    };
    dated
        .replace("{month}", &month_name)
        .replace("{year}", year)
}

/// `5 minutes ago` for recent ISO-8601 timestamps, otherwise the usual date.
///
/// Dates are shown relatively for their first 30 days, matching the playlist
/// table's compact, time-aware presentation. `now` is an argument so callers
/// can render against one instant and the boundary behaviour stays testable.
pub fn format_relative_date(locale: Locale, iso: &str, now: jiff::Timestamp) -> String {
    let Ok(added) = iso.parse::<jiff::Timestamp>() else {
        return format_date(locale, iso);
    };
    let seconds = added.duration_until(now).as_secs_f64().floor() as i64;
    if !(0..30 * 24 * 60 * 60).contains(&seconds) {
        return format_date(locale, iso);
    }

    let (count, text) = if seconds < 60 {
        let count = seconds;
        let text = ngettext(
            locale,
            // Translators: How long ago a song was added. Keep {count}.
            "{count} second ago",
            "{count} seconds ago",
            count as u32,
        );
        (count, text)
    } else if seconds < 60 * 60 {
        let count = seconds / 60;
        let text = ngettext(
            locale,
            // Translators: How long ago a song was added. Keep {count}.
            "{count} minute ago",
            "{count} minutes ago",
            count as u32,
        );
        (count, text)
    } else if seconds < 24 * 60 * 60 {
        let count = seconds / (60 * 60);
        let text = ngettext(
            locale,
            // Translators: How long ago a song was added. Keep {count}.
            "{count} hour ago",
            "{count} hours ago",
            count as u32,
        );
        (count, text)
    } else if seconds < 7 * 24 * 60 * 60 {
        let count = seconds / (24 * 60 * 60);
        // Translators: How long ago a song was added. Keep {count}.
        let text = ngettext(locale, "{count} day ago", "{count} days ago", count as u32);
        (count, text)
    } else {
        let count = seconds / (7 * 24 * 60 * 60);
        let text = ngettext(
            locale,
            // Translators: How long ago a song was added. Keep {count}.
            "{count} week ago",
            "{count} weeks ago",
            count as u32,
        );
        (count, text)
    };
    text.replace("{count}", &count.to_string())
}

/// Tears the id out of `spotify:track:abc` and friends.
pub fn uri_id(uri: &str) -> Option<&str> {
    uri.rsplit(':').next().filter(|id| !id.is_empty())
}

pub fn uri_kind(uri: &str) -> Option<&str> {
    let mut parts = uri.split(':');
    parts.next()?;
    parts.next()
}

/// Spotify's radio station seeded by a song, playlist, album, or artist.
pub fn station_uri(seed: &str) -> Option<String> {
    let kind = uri_kind(seed)?;
    let id = uri_id(seed)?;
    (seed == format!("spotify:{kind}:{id}")
        && matches!(kind, "track" | "playlist" | "album" | "artist"))
    .then(|| format!("spotify:station:{kind}:{id}"))
}

/// The song, playlist, album, or artist a radio station is seeded by.
pub fn station_seed(station: &str) -> Option<String> {
    let seed = format!("spotify:{}", station.strip_prefix("spotify:station:")?);
    station_uri(&seed).is_some().then_some(seed)
}

pub fn open_spotify_url(uri: &str) -> Option<String> {
    let kind = uri_kind(uri)?;
    let id = uri_id(uri)?;
    Some(format!("https://open.spotify.com/{kind}/{id}"))
}

/// The menu-bar shape for macOS: the square with the letters punched
/// out. macOS template images use only the alpha channel and paint the
/// shape themselves, black in a light menu bar and white in a dark one.
pub fn tray_template_rgba(size: usize) -> Vec<u8> {
    let mut rgba = mark_rgba(size, false);
    for pixel in rgba.as_chunks_mut::<4>().0 {
        // The letters are the dark colour; make them holes instead.
        if pixel[1] < 128 {
            pixel[3] = 0;
        }
        pixel[0] = 0;
        pixel[1] = 0;
        pixel[2] = 0;
    }
    rgba
}

/// The mark rasterised to pixels: the window icon, the trays and the logo
/// drawn in the app (`theme::logo`) all use this one picture.
///
/// It is the polished mark of `packaging/icons` at every size.
pub fn app_icon_rgba(size: usize) -> Vec<u8> {
    mark_rgba(size, true)
}

/// Mixes two colours, `t` of the way from `a` to `b`.
fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// How far `p` is from the segment `a`–`b`.
fn segment_distance(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (ex, ey) = (b.0 - a.0, b.1 - a.1);
    let (px, py) = (p.0 - a.0, p.1 - a.1);
    let along = ((px * ex + py * ey) / (ex * ex + ey * ey)).clamp(0.0, 1.0);
    ((px - ex * along).powi(2) + (py - ey * along).powi(2)).sqrt()
}

/// How far `p` is outside the mark's rounded square (negative inside): a
/// 480-unit square centred in the 512-unit field, corners rounded by 116.
fn square_distance(p: (f32, f32)) -> f32 {
    let qx = (p.0 - 256.0).abs() - (240.0 - 116.0);
    let qy = (p.1 - 256.0).abs() - (240.0 - 116.0);
    qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - 116.0
}

/// The strokes of the mark on a 512-unit square: each glyph's segments, its
/// half width, and whether it is the accent: a white L with an amber X at
/// its corner.
const GLYPHS: [(&[((f32, f32), (f32, f32))], f32, bool); 2] = [
    (&[((150.0, 92.0), (150.0, 322.0)), ((150.0, 322.0), (296.0, 322.0))], 26.0, false),
    (&[((268.0, 164.0), (352.0, 248.0)), ((352.0, 164.0), (268.0, 248.0))], 20.0, true),
];

/// IFY in Permanent Marker (Font Diner, Apache License 2.0), its outlines
/// flattened to polygons on the mark's 512-unit square, tilted 4° up.
#[rustfmt::skip]
const IFY: [&[(f32, f32)]; 3] = [
    &[(218.26, 427.73), (217.16, 426.83), (216.26, 425.34), (215.03, 421.61), (215.11, 420.79), (215.91, 419.8), (224.0, 418.29), (228.53, 405.26), (230.2, 396.51), (230.25, 395.81), (225.38, 397.31), (224.16, 397.18), (223.12, 396.63), (221.21, 393.64), (221.15, 391.94), (220.5, 390.94), (221.02, 390.38), (221.07, 389.26), (226.75, 388.15), (228.24, 387.49), (239.41, 386.23), (244.51, 386.41), (246.68, 387.08), (247.83, 388.66), (249.39, 389.32), (249.79, 391.8), (248.14, 392.8), (245.46, 393.68), (238.57, 394.69), (235.73, 409.91), (233.41, 416.78), (236.98, 416.53), (237.39, 417.22), (239.12, 417.94), (239.51, 418.68), (240.71, 419.22), (241.0, 420.71), (239.91, 422.94), (239.17, 423.12), (238.3, 424.05), (232.58, 424.66), (232.31, 425.32), (230.07, 425.05), (227.8, 425.31), (220.74, 427.31)],
    &[(248.66, 414.24), (247.19, 413.25), (245.77, 411.36), (244.99, 409.33), (245.06, 407.7), (246.11, 405.94), (247.89, 404.8), (249.29, 404.31), (251.26, 405.33), (251.56, 405.15), (252.84, 401.42), (253.0, 399.92), (250.89, 399.18), (248.88, 396.75), (248.03, 396.2), (248.39, 395.37), (248.01, 394.13), (248.08, 393.08), (249.09, 391.69), (259.54, 387.42), (261.4, 387.05), (268.4, 384.6), (273.27, 383.6), (276.49, 383.37), (277.93, 383.64), (282.56, 387.18), (282.99, 388.4), (282.94, 389.64), (282.54, 390.28), (280.93, 391.1), (276.17, 391.51), (270.59, 392.6), (265.26, 394.22), (262.12, 395.85), (262.0, 398.61), (261.28, 400.1), (261.16, 401.78), (260.59, 403.23), (260.92, 404.39), (268.59, 402.82), (273.26, 402.37), (272.82, 402.87), (273.4, 402.88), (273.14, 403.75), (274.07, 404.11), (274.82, 404.88), (275.31, 405.98), (272.65, 410.29), (270.83, 410.32), (257.56, 413.77), (256.37, 416.45), (256.49, 417.39), (255.98, 417.58), (255.57, 419.51), (256.0, 421.14), (255.83, 422.57), (255.12, 424.0), (254.01, 425.02), (252.92, 425.08), (250.86, 424.55), (250.19, 424.07), (248.21, 421.4), (247.83, 418.77)],
    &[(288.03, 423.96), (286.57, 423.75), (284.2, 422.68), (283.3, 421.82), (282.06, 419.46), (283.0, 415.33), (283.85, 414.0), (287.54, 405.54), (285.62, 403.3), (283.17, 398.34), (281.47, 392.12), (281.63, 390.57), (280.61, 386.74), (280.86, 384.55), (282.46, 382.9), (288.53, 385.76), (288.67, 386.74), (289.36, 386.97), (289.58, 388.92), (290.15, 390.03), (291.49, 394.87), (291.59, 396.27), (292.69, 395.3), (296.02, 390.04), (295.87, 389.42), (298.18, 386.96), (300.64, 383.59), (302.58, 382.15), (305.75, 380.96), (306.7, 382.64), (309.52, 384.29), (310.2, 386.41), (309.87, 387.36), (309.09, 388.68), (304.7, 393.61), (304.02, 395.56), (303.14, 395.68), (301.56, 398.22), (298.87, 403.54), (299.01, 404.11), (298.14, 405.11), (294.49, 412.04), (293.61, 414.73), (291.87, 417.66), (290.41, 421.67), (288.9, 423.58)],
];

/// The box IFY's outlines sit in, on the mark's square.
const IFY_BOX: (f32, f32, f32, f32) = (215.03, 380.96, 310.2, 427.73);

/// Whether `p` is inside IFY's outlines (nonzero winding).
fn inside_ify(p: (f32, f32)) -> bool {
    let mut winding = 0;
    for contour in IFY {
        for (index, a) in contour.iter().enumerate() {
            let b = contour[(index + 1) % contour.len()];
            let side = (b.0 - a.0) * (p.1 - a.1) - (p.0 - a.0) * (b.1 - a.1);
            if a.1 <= p.1 {
                if b.1 > p.1 && side > 0.0 {
                    winding += 1;
                }
            } else if b.1 <= p.1 && side < 0.0 {
                winding -= 1;
            }
        }
    }
    winding != 0
}

/// The Leaximy mark at `size` pixels: a navy rounded square, lighter at the
/// top, with the white L, the amber X and an amber hand-lettered IFY
/// beneath. `polished` false gives the
/// plain mark, a light square with every stroke dark, which
/// `tray_template_rgba` relies on to punch the strokes out.
fn mark_rgba(size: usize, polished: bool) -> Vec<u8> {
    const PLAIN: [f32; 3] = [246.0, 248.0, 252.0];
    const INK: [f32; 3] = [10.0, 20.0, 46.0];
    const WHITE: [f32; 3] = [255.0, 255.0, 255.0];
    const AMBER: [f32; 3] = [245.0, 179.0, 53.0];
    let mut rgba = vec![0u8; size * size * 4];
    // The square keeps two pixels of margin, so its edge is never clipped.
    let unit = (size as f32 / 2.0 - 2.0) / 240.0;
    let origin = size as f32 / 2.0 - 256.0 * unit;
    for y in 0..size {
        for x in 0..size {
            // The pixel's centre in the mark's own units.
            let p = (
                (x as f32 + 0.5 - origin) / unit,
                (y as f32 + 0.5 - origin) / unit,
            );
            let coverage = (-square_distance(p) * unit + 0.5).clamp(0.0, 1.0);
            if coverage <= 0.0 {
                continue;
            }
            let mut colour = if polished {
                mix([36.0, 70.0, 160.0], [10.0, 22.0, 58.0], (p.1 - 16.0) / 480.0)
            } else {
                PLAIN
            };
            for (segments, half, accent) in GLYPHS {
                let distance = segments
                    .iter()
                    .map(|(a, b)| segment_distance(p, *a, *b))
                    .fold(f32::MAX, f32::min);
                let ink = ((half - distance) * unit + 0.5).clamp(0.0, 1.0);
                if ink > 0.0 {
                    let stroke = match (polished, accent) {
                        (false, _) => INK,
                        (true, true) => AMBER,
                        (true, false) => WHITE,
                    };
                    colour = mix(colour, stroke, ink);
                }
            }
            // IFY is outlines, not strokes: sixteen samples a pixel near it.
            let margin = 1.0 / unit;
            if p.0 >= IFY_BOX.0 - margin
                && p.0 <= IFY_BOX.2 + margin
                && p.1 >= IFY_BOX.1 - margin
                && p.1 <= IFY_BOX.3 + margin
            {
                let mut hits = 0;
                for i in 0..4 {
                    for j in 0..4 {
                        let sample = (
                            (x as f32 + (i as f32 + 0.5) / 4.0 - origin) / unit,
                            (y as f32 + (j as f32 + 0.5) / 4.0 - origin) / unit,
                        );
                        if inside_ify(sample) {
                            hits += 1;
                        }
                    }
                }
                if hits > 0 {
                    let stroke = if polished { AMBER } else { INK };
                    colour = mix(colour, stroke, hits as f32 / 16.0);
                }
            }
            let index = (y * size + x) * 4;
            rgba[index] = colour[0].round() as u8;
            rgba[index + 1] = colour[1].round() as u8;
            rgba[index + 2] = colour[2].round() as u8;
            rgba[index + 3] = (coverage * 255.0) as u8;
        }
    }
    rgba
}

pub fn greeting(locale: Locale) -> Cow<'static, str> {
    match local_hour() {
        5..=11 => gettext(locale, "Good morning"),
        12..=17 => gettext(locale, "Good afternoon"),
        _ => gettext(locale, "Good evening"),
    }
}

fn local_hour() -> u8 {
    jiff::Zoned::now().hour() as u8
}

/// Strips the HTML Spotify embeds in playlist descriptions.
pub fn strip_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_tag = false;
    for character in text.chars() {
        match character {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(character),
            _ => {}
        }
    }
    out.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&#x2F;", "/")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

/// Atomically replaces `path` with `temporary` on the current platform.
#[cfg(not(windows))]
pub(crate) fn replace_file(
    temporary: &std::path::Path,
    path: &std::path::Path,
) -> std::io::Result<()> {
    std::fs::rename(temporary, path)
}

/// Atomically replaces `path` with `temporary` on Windows.
#[cfg(windows)]
pub(crate) fn replace_file(
    temporary: &std::path::Path,
    path: &std::path::Path,
) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let temporary: Vec<u16> = temporary.as_os_str().encode_wide().chain(Some(0)).collect();
    let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let moved = unsafe {
        MoveFileExW(
            temporary.as_ptr(),
            path.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if moved == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(rgba: &[u8], size: usize, x: usize, y: usize) -> [u8; 4] {
        let index = (y * size + x) * 4;
        [
            rgba[index],
            rgba[index + 1],
            rgba[index + 2],
            rgba[index + 3],
        ]
    }

    /// The icon is the same mark at every size, and the tray template
    /// keeps its letters punched out.
    #[test]
    fn the_icon_is_the_mark_at_every_size() {
        // #given the icon at a dock size and at a tray size
        let (large, small) = (app_icon_rgba(128), app_icon_rgba(32));

        // #then the square is navy where nothing is written
        let face = pixel(&large, 128, 111, 28);
        assert!(face[2] > face[0], "face {face:?} should be navy");

        // #and it carries the white L and the amber X
        let l = pixel(&large, 128, 36, 49);
        assert!(l[..3].iter().all(|&c| c > 215), "L missing: {l:?}");
        let x = pixel(&large, 128, 77, 51);
        assert!(x[0] > 200 && x[0] > x[2], "X {x:?} should be amber");
        let small_l = pixel(&small, 32, 9, 12);
        assert!(small_l[..3].iter().all(|&c| c > 215), "L missing at 32: {small_l:?}");

        // #and the corners stay clear
        assert_eq!(pixel(&large, 128, 1, 1)[3], 0);

        // #and the menu-bar template is the square with the letters cut out
        let template = tray_template_rgba(44);
        assert_eq!(pixel(&template, 44, 13, 17)[3], 0);
        assert_eq!(pixel(&template, 44, 37, 10), [0, 0, 0, 255]);
    }

    #[test]
    fn radio_stations_map_to_their_seeds_and_back() {
        for kind in ["track", "playlist", "album", "artist"] {
            let seed = format!("spotify:{kind}:4uLU6hMCjMI75M1A2tKUQC");
            let station = station_uri(&seed).expect("a station");
            assert_eq!(
                station,
                format!("spotify:station:{kind}:4uLU6hMCjMI75M1A2tKUQC")
            );
            assert_eq!(station_seed(&station).as_deref(), Some(seed.as_str()));
        }
        for seed in [
            "spotify:show:abc",
            "spotify:episode:abc",
            "spotify:user:me:collection",
            "spotify:track:",
            "track:abc",
        ] {
            assert_eq!(station_uri(seed), None, "{seed}");
        }
        assert_eq!(station_seed("spotify:playlist:abc"), None);
    }

    #[test]
    fn durations() {
        assert_eq!(format_duration_ms(225_000), "3:45");
        assert_eq!(format_duration_ms(3_723_000), "1:02:03");
        assert_eq!(format_total_ms(Locale::English, 7_980_000), "2 hr 13 min");
        assert_eq!(format_total_ms(Locale::English, 2_712_000), "45 min 12 sec");
        assert_eq!(format_episode_ms(Locale::English, 4_320_000), "1 hr 12 min");
    }

    #[test]
    fn counts_and_dates() {
        assert_eq!(format_count(1_234_567), "1,234,567");
        assert_eq!(format_count(12), "12");
        assert_eq!(
            format_date(Locale::English, "2024-01-05T10:00:00Z"),
            "Jan 5, 2024"
        );
        assert_eq!(format_date(Locale::English, "2024-03"), "Mar 2024");
        assert_eq!(format_date(Locale::English, "2024"), "2024");
    }

    #[test]
    fn recent_dates_are_relative_for_the_first_month() {
        let now: jiff::Timestamp = "2026-08-31T12:00:00Z".parse().unwrap();
        assert_eq!(
            format_relative_date(Locale::English, "2026-08-31T11:59:30Z", now),
            "30 seconds ago"
        );
        assert_eq!(
            format_relative_date(Locale::English, "2026-08-31T11:59:00Z", now),
            "1 minute ago"
        );
        assert_eq!(
            format_relative_date(Locale::English, "2026-08-31T11:00:00Z", now),
            "1 hour ago"
        );
        assert_eq!(
            format_relative_date(Locale::English, "2026-08-30T12:00:00Z", now),
            "1 day ago"
        );
        assert_eq!(
            format_relative_date(Locale::English, "2026-08-17T12:00:00Z", now),
            "2 weeks ago"
        );
        assert_eq!(
            format_relative_date(Locale::English, "2026-08-01T12:00:00Z", now),
            "Aug 1, 2026"
        );
    }

    #[test]
    fn dates_and_lengths_follow_the_interface_language() {
        let now: jiff::Timestamp = "2026-08-31T12:00:00Z".parse().unwrap();
        assert_eq!(format_date(Locale::Spanish, "2024-01-05"), "5 ene 2024");
        assert_eq!(format_date(Locale::Spanish, "2024-09"), "sept 2024");
        assert_eq!(format_total_ms(Locale::Spanish, 7_980_000), "2 h 13 min");
        assert_eq!(format_episode_ms(Locale::Spanish, 2_280_000), "38 min");
        for (added, expected) in [
            ("2026-08-31T11:59:59Z", "hace 1 segundo"),
            ("2026-08-31T11:58:00Z", "hace 2 minutos"),
            ("2026-08-30T12:00:00Z", "hace 1 día"),
            ("2026-08-17T12:00:00Z", "hace 2 semanas"),
        ] {
            assert_eq!(format_relative_date(Locale::Spanish, added, now), expected);
        }
        // Each language orders the date its own way.
        assert_eq!(format_date(Locale::Swedish, "2024-01-05"), "5 jan. 2024");
        assert_eq!(format_date(Locale::English, "2024-01-05"), "Jan 5, 2024");
        assert_eq!(
            format_relative_date(Locale::Swedish, "2026-08-30T12:00:00Z", now),
            "för 1 dag sedan"
        );
    }

    #[test]
    fn relative_dates_fall_back_for_future_and_invalid_timestamps() {
        let now: jiff::Timestamp = "2026-08-31T12:00:00Z".parse().unwrap();
        assert_eq!(
            format_relative_date(Locale::English, "2026-09-01T12:00:00Z", now),
            "Sep 1, 2026"
        );
        assert_eq!(
            format_relative_date(Locale::English, "not-a-date", now),
            "not-a-date"
        );
    }

    #[test]
    fn uris() {
        assert_eq!(uri_id("spotify:track:abc"), Some("abc"));
        assert_eq!(uri_kind("spotify:playlist:x"), Some("playlist"));
        assert_eq!(
            open_spotify_url("spotify:album:z").as_deref(),
            Some("https://open.spotify.com/album/z")
        );
    }

    #[test]
    fn html_is_stripped() {
        assert_eq!(
            strip_html("Hi <a href=\"x\">there</a> &amp; you"),
            "Hi there & you"
        );
        assert_eq!(strip_html("ONE&#x2F;TWO&#x2F;THREE"), "ONE/TWO/THREE");
    }
}
