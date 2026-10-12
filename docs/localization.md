# UI localization

SolidCraft embeds its translations and works offline. English and Japanese are currently
registered in `crates/ui-egui/src/i18n.rs`. Adding a language requires an embedded catalog and
one registration there; the preferences selector and validation use the same registry.

## Messages

`crates/ui-egui/src/locales/en.json` defines stable message IDs and English fallback text.
Keep IDs when changing wording. Other catalogs use the same IDs; missing messages fall back
to English. Use `app.tr("ui.preferences")` for static messages and
`Locale::format` for full sentences with named placeholders. Translate the entire sentence,
including punctuation, so a translator can reorder its arguments. Placeholder values are
inserted literally, even if a document name contains braces. A doubled brace stands for the
brace itself, so a message can show literal JSON: `Extrude {{\"distance\": 20}}` reads as
`Extrude {"distance": 20}`. Use `Locale::format` (not `Locale::text`) for a message written
that way, because only formatting unescapes it.

The English-label adapter (`app.tr_label`) supports existing engine and UI tables. It maps
display text to a message ID, then uses the same catalog lookup. New UI messages should use
stable IDs directly. The adapter does not change command IDs, model names, or scenario handles.
When a table's English wording changes, update the corresponding English catalog entry too.
Generated browser labels use named templates while their underlying model names stay intact.

Dialog row labels, dialog titles and pick hints name their English source text rather than a
message ID, so they go through the adapter (`row_label`, `title`, `hint` in
`crates/ui-egui/src/dialogs.rs`, and the combo box options). Adding a row label to a dialog
therefore means adding one catalog entry, not an ID lookup at the call site. A value with
placeholders cannot travel that way: the adapter skips template text, so write those through
`tr` or `format` with the ID.

Catalogs are parsed once and looked up through ordered maps. This is offline localization,
not a claim of zero runtime overhead. The catalog tests check registered locale IDs,
translation keys, and placeholder consistency. Run `cargo xtask ci` before submitting changes.

## Rendering and coverage

One immutable `Locale` type handles all translations. Table widgets without an app reference
use a render scope that restores the previous locale when it exits, including nested renders.
Standalone translated panels enter the same scope. Measure rendered text to size custom
widgets; UTF-8 byte lengths do not describe their displayed width.

The desktop host finds a Japanese-capable system font by looking for the known Meiryo, Hiragino
and Debian Noto paths first, then searching the platform font directories for a family name
that covers Japanese (Noto Sans JP, Noto Sans CJK, Source Han, Droid Sans Fallback and the
common free families). A candidate counts only when a face of the file actually holds hiragana,
katakana and a kanji glyph, and the face index is passed to egui so a `.ttc` collection works.
The search is bounded in depth and in files read, so a large font tree cannot hold up startup.
No system fonts are redistributed. Without such a font the UI shows blanks for CJK text and
logs a warning; browser hosts cannot load local system fonts and must provide suitably licensed
font assets separately.

The current messages support named substitution, not locale-specific plural selection,
number/date formatting, or right-to-left layout and shaping. Add those capabilities when
introducing messages or languages that need them; use an established message formatter for
plural rules instead of adding language branches to panels. The catalog covers every registered
command label, toolbar tab and panel, every dialog row label, title and pick hint, and the
user-visible literals of the panels listed above; engine error text and command parameter
documentation are still English. `every_command_label_is_translated` fails when a command,
tab or panel is added without a catalog entry, and `one_english_label_has_one_translation`
fails when two IDs share an English label but disagree about the translation.
