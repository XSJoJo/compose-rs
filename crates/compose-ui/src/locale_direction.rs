//! Whether a locale's text runs right to left, as Android's
//! `TextUtils.getLayoutDirectionFromLocale` tells it: ICU's `ULocale.isRightToLeft`, which asks the
//! locale's script, or its likely script when it names none (CLDR's likely subtags), whether that
//! script is written right to left (D2049, D2061).
//!
//! The tables are ICU 68.2's (androidx-main's `external/icu`, CLDR 38), generated from its data
//! (D2061): the scripts with ICU's `RTL` property, the languages whose likely script has it, and
//! the language and region pairs whose likely script differs in direction from the language's.
//! Every other language is left to right, as ICU says of a language it has no likely subtags for.

use crate::intl::Locale;

/// Whether `locale`'s text runs right to left, as ICU's `ULocale.isRightToLeft`.
pub(crate) fn is_right_to_left(locale: &Locale) -> bool {
    // A language longer than a subtag can be: no likely subtags.
    let Some(tag) = Tag::of(locale.to_language_tag()) else {
        return false;
    };
    if let Some(script) = tag.script.as_str() {
        return RIGHT_TO_LEFT_SCRIPTS.binary_search(&script).is_ok();
    }
    let language = tag.language.as_str().unwrap_or("");
    // ICU's fast path: a few common languages' likely scripts, whatever the region.
    match language {
        "root" | "en" | "es" | "pt" | "zh" | "ja" | "ko" | "de" | "fr" | "it" | "ru" | "nl"
        | "pl" | "th" | "tr" => return false,
        "ar" | "he" | "fa" => return true,
        _ => {}
    }
    // Else the likely subtags of `language_REGION`, else of `language` (`und` for none).
    let language = if language.is_empty() { "und" } else { language };
    if let Some(region) = tag.region.as_str() {
        let found = REGIONS_CHANGING_DIRECTION.binary_search_by(|&(entry, entry_region, _)| {
            (entry, entry_region).cmp(&(language, region))
        });
        if let Ok(found) = found {
            return REGIONS_CHANGING_DIRECTION[found].2;
        }
    }
    RIGHT_TO_LEFT_LANGUAGES.binary_search(&language).is_ok()
}

/// A subtag in its canonical case, held inline: reading a locale allocates nothing.
#[derive(Default)]
struct Subtag {
    bytes: [u8; 8],
    len: usize,
}

impl Subtag {
    /// `subtag` lower case, its first letter upper case if `title` (a script's); `None` if it's
    /// longer than a subtag can be (eight letters).
    fn of(subtag: &str, title: bool) -> Option<Subtag> {
        let mut bytes = [0; 8];
        bytes.get_mut(..subtag.len())?.copy_from_slice(subtag.as_bytes());
        bytes.make_ascii_lowercase();
        if title {
            bytes[0].make_ascii_uppercase();
        }
        Some(Subtag { bytes, len: subtag.len() })
    }

    /// Upper case, as a region.
    fn upper(mut self) -> Subtag {
        self.bytes.make_ascii_uppercase();
        self
    }

    fn as_str(&self) -> Option<&str> {
        (self.len > 0).then(|| std::str::from_utf8(&self.bytes[..self.len]).unwrap_or(""))
    }
}

/// A tag's language, script and region, as [`Locale`]'s `language`, `script` and `region` read
/// them.
struct Tag {
    language: Subtag,
    script: Subtag,
    region: Subtag,
}

impl Tag {
    fn of(tag: &str) -> Option<Tag> {
        // The subtags before any extension (`-u-nu-arab`), whose short ones aren't regions.
        let mut subtags = tag.split('-').take_while(|subtag| subtag.len() != 1);
        let language = Subtag::of(subtags.next().unwrap_or(""), false)?;
        let (mut script, mut region) = (None, None);
        for subtag in subtags {
            let alphabetic = subtag.bytes().all(|b| b.is_ascii_alphabetic());
            if script.is_none() && subtag.len() == 4 && alphabetic {
                script = Subtag::of(subtag, true);
            } else if region.is_none()
                && ((subtag.len() == 2 && alphabetic)
                    || (subtag.len() == 3 && subtag.bytes().all(|b| b.is_ascii_digit())))
            {
                region = Subtag::of(subtag, false).map(Subtag::upper);
            }
        }
        Some(Tag {
            language,
            script: script.unwrap_or_default(),
            region: region.unwrap_or_default(),
        })
    }
}

// ---- Generated once from ICU 68.2's data in androidx-main's external/icu (CLDR 38):
// `icu4c/source/common/uscript_props.cpp`'s scripts with `RTL`, and
// `icu4c/source/data/misc/likelySubtags.txt`'s entries without a script, as D2061 says;
// for another ICU, generated again the same way, not edited. ----

/// The scripts ICU's `UScript.isRightToLeft` says are written right to left (their
/// properties have `RTL`).
const RIGHT_TO_LEFT_SCRIPTS: &[&str] = &[
    "Adlm", "Arab", "Armi", "Avst", "Chrs", "Cprt", "Elym", "Hatr", "Hebr", "Hung", "Khar", "Lydi",
    "Mand", "Mani", "Mend", "Merc", "Mero", "Narb", "Nbat", "Nkoo", "Orkh", "Palm", "Phli", "Phlp",
    "Phnx", "Prti", "Rohg", "Samr", "Sarb", "Sogd", "Sogo", "Syrc", "Thaa", "Yezi",
];

/// The languages whose likely script (their own entry's) is written right to left.
const RIGHT_TO_LEFT_LANGUAGES: &[&str] = &[
    "ae", "aeb", "apc", "apd", "ar", "arc", "arq", "ars", "ary", "arz", "avl", "bal", "bej", "bft",
    "bgn", "bqi", "brh", "cja", "ckb", "dcc", "dgl", "dv", "fa", "fia", "fub", "gbz", "gjk", "gju",
    "glk", "grc", "gwc", "gwt", "haz", "he", "hnd", "hno", "iw", "kby", "kdh", "khw", "ks", "kvx",
    "kxp", "kzh", "lad", "lah", "lki", "lrc", "luz", "mde", "mfa", "mis", "mki", "mvy", "myz",
    "mzn", "nqo", "oru", "ota", "otk", "pal", "phl", "phn", "pra", "prd", "ps", "rhg", "rmt",
    "scl", "sd", "sdh", "shu", "skr", "smp", "sog", "swb", "syr", "trw", "ug", "ur", "wni", "xco",
    "xld", "xmn", "xmr", "xna", "xpr", "xsa", "yi", "zdj",
];

/// The language and region entries whose likely script's direction isn't the language's own
/// (`und`: a locale naming only its region): whether they're right to left.
const REGIONS_CHANGING_DIRECTION: &[(&str, &str, bool)] = &[
    ("az", "IQ", true),
    ("az", "IR", true),
    ("ha", "CM", true),
    ("ha", "SD", true),
    ("kk", "AF", true),
    ("kk", "CN", true),
    ("kk", "IR", true),
    ("kk", "MN", true),
    ("ku", "IR", true),
    ("ku", "LB", true),
    ("ky", "CN", true),
    ("man", "GN", true),
    ("ms", "CC", true),
    ("pa", "PK", true),
    ("tg", "PK", true),
    ("ug", "KZ", false),
    ("ug", "MN", false),
    ("und", "015", true),
    ("und", "145", true),
    ("und", "AE", true),
    ("und", "AF", true),
    ("und", "BH", true),
    ("und", "DZ", true),
    ("und", "EG", true),
    ("und", "EH", true),
    ("und", "IL", true),
    ("und", "IQ", true),
    ("und", "IR", true),
    ("und", "JO", true),
    ("und", "KM", true),
    ("und", "KW", true),
    ("und", "LB", true),
    ("und", "LY", true),
    ("und", "MA", true),
    ("und", "MR", true),
    ("und", "MV", true),
    ("und", "OM", true),
    ("und", "PK", true),
    ("und", "PS", true),
    ("und", "QA", true),
    ("und", "SA", true),
    ("und", "SD", true),
    ("und", "SY", true),
    ("und", "TN", true),
    ("und", "YE", true),
    ("uz", "AF", true),
];
