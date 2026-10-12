//! System font loading and fallback setup for multi-language (CJK/Japanese) support.

#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;

/// Candidate font paths to look for on different operating systems. Distributions lay their
/// font packages out differently - Fedora, Arch and openSUSE do not use the Debian paths below
/// at all - so these are only the first try and the search falls back to the font directories.
#[cfg(not(target_arch = "wasm32"))]
const FONT_CANDIDATES: &[&str] = &[
    // Windows
    "C:\\Windows\\Fonts\\meiryo.ttc",
    "C:\\Windows\\Fonts\\YuGothM.ttc",
    "C:\\Windows\\Fonts\\YuGothR.ttc",
    "C:\\Windows\\Fonts\\msgothic.ttc",
    "C:\\Windows\\Fonts\\BIZ-UDGothicR.ttc",
    // macOS
    "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc",
    "/System/Library/Fonts/Hiragino Sans GB.ttc",
    "/System/Library/Fonts/PingFang.ttc",
    "/Library/Fonts/Arial Unicode.ttf",
    // Linux (Debian/Ubuntu package layout)
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
];

/// Font directories searched when none of the fixed paths exists.
#[cfg(not(target_arch = "wasm32"))]
const FONT_DIRS: &[&str] = &["/usr/share/fonts", "/usr/local/share/fonts", "/System/Library/Fonts", "/Library/Fonts", "C:\\Windows\\Fonts"];

/// Font directories under the user's home directory, searched after the system ones.
#[cfg(not(target_arch = "wasm32"))]
const HOME_FONT_DIRS: &[&str] = &[".local/share/fonts", ".fonts"];

/// Japanese families by file name, most preferred first. A plain single-face file beats a
/// collection: Noto Sans JP covers the UI at a fraction of the CJK collection's size.
#[cfg(not(target_arch = "wasm32"))]
const JAPANESE_FAMILIES: &[&str] = &[
    "NotoSansJP",
    "NotoSansCJK",
    "NotoSansMonoCJK",
    "SourceHanSans",
    "NotoSerifJP",
    "NotoSerifCJK",
    "SourceHanSerif",
    "DroidSansFallback",
    "Takao",
    "VLGothic",
    "IPAGothic",
    "IPAPGothic",
    "Migu",
    "WenQuanYi",
    "unifont",
];

/// Code points a fallback font has to cover: hiragana, katakana and a common kanji.
#[cfg(not(target_arch = "wasm32"))]
const JAPANESE_PROBES: [char; 3] = ['あ', 'ア', '日'];

/// Deepest directory level searched and most files looked at, so a font tree with tens of
/// thousands of files cannot hold up startup.
#[cfg(not(target_arch = "wasm32"))]
const MAX_FONT_DEPTH: usize = 4;
#[cfg(not(target_arch = "wasm32"))]
const MAX_FONT_FILES: usize = 4096;

/// How well a font file name matches a Japanese family: lower is better, `None` when the name
/// does not look like one. Case and separators are ignored, so "Noto Sans JP Medium.otf"
/// matches "NotoSansJP".
#[cfg(not(target_arch = "wasm32"))]
fn family_rank(name: &str) -> Option<usize> {
    let flat: String = name.chars().filter(char::is_ascii_alphanumeric).collect::<String>().to_ascii_lowercase();
    JAPANESE_FAMILIES.iter().position(|family| flat.contains(&family.to_ascii_lowercase()))
}

/// The first face of a font file that covers Japanese (a collection holds several faces).
#[cfg(not(target_arch = "wasm32"))]
fn japanese_face(bytes: &[u8]) -> Option<u32> {
    let faces = ttf_parser::fonts_in_collection(bytes).unwrap_or(1).clamp(1, 32);
    (0..faces)
        .find(|index| ttf_parser::Face::parse(bytes, *index).is_ok_and(|face| JAPANESE_PROBES.iter().all(|probe| face.glyph_index(*probe).is_some())))
}

/// Font files under `dir` whose name looks like a Japanese family, as (rank, path). The walk is
/// bounded in depth and in files examined; `files` carries the budget across directories.
#[cfg(not(target_arch = "wasm32"))]
fn named_fonts(dir: &Path, depth: usize, files: &mut usize, out: &mut Vec<(usize, PathBuf)>) {
    if depth > MAX_FONT_DEPTH || *files >= MAX_FONT_FILES {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        if *files >= MAX_FONT_FILES {
            return;
        }
        *files += 1;
        let path = entry.path();
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            named_fonts(&path, depth + 1, files, out);
        } else if let Some(rank) = path.file_name().and_then(|name| name.to_str()).and_then(family_rank) {
            out.push((rank, path));
        }
    }
}

/// A system font that covers Japanese: where it is, which face of the file to use, and its
/// bytes. `None` when the system has no Japanese font (the UI then shows blanks for CJK text).
#[cfg(not(target_arch = "wasm32"))]
fn find_japanese_font() -> Option<(PathBuf, u32, Vec<u8>)> {
    for path in FONT_CANDIDATES {
        let Ok(bytes) = std::fs::read(path) else { continue };
        if let Some(index) = japanese_face(&bytes) {
            return Some((PathBuf::from(path), index, bytes));
        }
    }
    let mut dirs: Vec<PathBuf> = FONT_DIRS.iter().map(PathBuf::from).collect();
    if let Some(home) = std::env::var_os("HOME") {
        dirs.extend(HOME_FONT_DIRS.iter().map(|dir| Path::new(&home).join(dir)));
    }
    let mut found = Vec::new();
    let mut files = 0;
    for dir in &dirs {
        named_fonts(dir, 0, &mut files, &mut found);
    }
    // Preference first, then path, so the choice does not depend on directory order.
    found.sort();
    for (_, path) in found {
        let Ok(bytes) = std::fs::read(&path) else { continue };
        if let Some(index) = japanese_face(&bytes) {
            return Some((path, index, bytes));
        }
    }
    None
}

/// Whether a locale draws CJK text and therefore needs the fallback font.
pub fn needs_cjk(language: &str) -> bool {
    language == "ja"
}

/// Configure font definitions with system CJK fallback if available. The fallback is loaded
/// only for a locale that draws CJK text: an English session does not pay for a font it never
/// uses.
#[cfg(not(target_arch = "wasm32"))]
pub fn setup_fonts(ctx: &egui::Context, language: &str) {
    if !needs_cjk(language) {
        return;
    }
    let Some((path, index, bytes)) = find_japanese_font() else {
        log::warn!("no Japanese-capable system font found: CJK text will show as blanks");
        return;
    };
    log::info!("Japanese fallback font: {} (face {index})", path.display());

    let mut data = egui::FontData::from_owned(bytes);
    data.index = index;
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert("cjk_fallback".to_owned(), Arc::new(data));

    fonts.families.entry(egui::FontFamily::Proportional).or_default().push("cjk_fallback".to_owned());

    fonts.families.entry(egui::FontFamily::Monospace).or_default().push("cjk_fallback".to_owned());

    ctx.set_fonts(fonts);
}

/// Browsers cannot read system font files. A web host can supply licensed fonts through egui.
#[cfg(target_arch = "wasm32")]
pub fn setup_fonts(_ctx: &egui::Context, _language: &str) {}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn japanese_family_names_rank_in_preference_order() {
        assert_eq!(family_rank("NotoSansJP-Regular.otf"), Some(0));
        assert!(family_rank("NotoSansCJK-VF.ttc") > family_rank("NotoSansJP-Regular.otf"));
        assert!(family_rank("Noto Sans JP Medium.ttf").is_some(), "separators and case are ignored");
        assert!(family_rank("DroidSansFallbackFull.ttf").is_some());
        assert_eq!(family_rank("DejaVuSans.ttf"), None);
        assert_eq!(family_rank(""), None);
    }

    #[test]
    fn something_that_is_not_a_font_has_no_japanese_face() {
        assert_eq!(japanese_face(b""), None);
        assert_eq!(japanese_face(b"not a font at all"), None);
        assert_eq!(japanese_face(&[0; 64]), None);
    }

    #[test]
    fn the_search_keeps_japanese_families_and_ignores_the_rest() {
        let root = std::env::temp_dir().join(format!("solvecraft-fonts-{}", std::process::id()));
        let nested = root.join("deeper");
        let _ = std::fs::create_dir_all(&nested);
        let _ = std::fs::write(root.join("DejaVuSans.ttf"), b"x");
        let _ = std::fs::write(root.join("NotoSansCJK-VF.ttc"), b"x");
        let _ = std::fs::write(nested.join("NotoSansJP-Regular.otf"), b"x");
        let mut found = Vec::new();
        let mut files = 0;
        named_fonts(&root, 0, &mut files, &mut found);
        let _ = std::fs::remove_dir_all(&root);
        let names: Vec<String> = found.iter().map(|(_, path)| path.file_name().unwrap_or_default().to_string_lossy().to_string()).collect();
        assert_eq!(found.len(), 2, "only the Japanese families: {names:?}");
        assert!(names.contains(&"NotoSansCJK-VF.ttc".to_string()));
        assert!(names.contains(&"NotoSansJP-Regular.otf".to_string()), "the walk descends into subdirectories");
    }

    /// The regression the hard-coded Debian paths caused: a machine with a Japanese font
    /// installed must find one. Skipped only when the machine really has none.
    #[test]
    fn an_installed_japanese_font_is_found() {
        let mut found = Vec::new();
        let mut files = 0;
        for dir in FONT_DIRS {
            named_fonts(Path::new(dir), 0, &mut files, &mut found);
        }
        if found.is_empty() {
            return;
        }
        let (path, index, bytes) = find_japanese_font().expect("a Japanese font is installed");
        assert_eq!(Some(index), japanese_face(&bytes));
        assert!(path.is_file());
    }
}
