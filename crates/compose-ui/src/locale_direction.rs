//! Whether a locale's text runs right to left, as Android's
//! `TextUtils.getLayoutDirectionFromLocale` tells it: ICU's `ULocale.isRightToLeft`, which asks the
//! locale's script, or its likely script when it names none (CLDR's likely subtags), whether that
//! script is written right to left (D2049).
//!
//! ICU's likely subtags cover every language; this covers those whose likely script is written
//! right to left (and the regions that change a language's script), from CLDR's likely subtags:
//! any other language is taken to be written left to right, as nearly all are.

use crate::intl::Locale;

/// Whether `locale`'s text runs right to left, as ICU's `ULocale.isRightToLeft`.
pub(crate) fn is_right_to_left(locale: &Locale) -> bool {
    let script = locale.script();
    if !script.is_empty() {
        return is_right_to_left_script(&script);
    }
    let language = locale.language();
    // ICU's fast path: the likely scripts of a few common languages, whatever the region.
    match language.as_str() {
        "" | "root" | "en" | "es" | "pt" | "zh" | "ja" | "ko" | "de" | "fr" | "it" | "ru"
        | "nl" | "pl" | "th" | "tr" => return false,
        "ar" | "he" | "fa" => return true,
        _ => {}
    }
    let region = locale.region();
    match likely_script_in_region(&language, &region) {
        Some(script) => is_right_to_left_script(script),
        None => likely_script(&language).is_some_and(is_right_to_left_script),
    }
}

/// The scripts ICU's `UScript.isRightToLeft` says are written right to left.
fn is_right_to_left_script(script: &str) -> bool {
    matches!(
        script,
        "Adlm"
            | "Arab"
            | "Aran"
            | "Armi"
            | "Avst"
            | "Chrs"
            | "Cprt"
            | "Elym"
            | "Gara"
            | "Hatr"
            | "Hebr"
            | "Hung"
            | "Khar"
            | "Lydi"
            | "Mand"
            | "Mani"
            | "Mend"
            | "Merc"
            | "Mero"
            | "Narb"
            | "Nbat"
            | "Nkoo"
            | "Orkh"
            | "Ougr"
            | "Palm"
            | "Phli"
            | "Phlp"
            | "Phlv"
            | "Phnx"
            | "Prti"
            | "Rohg"
            | "Samr"
            | "Sarb"
            | "Sogd"
            | "Sogo"
            | "Syrc"
            | "Syre"
            | "Syrj"
            | "Syrn"
            | "Thaa"
            | "Yezi"
    )
}

/// The likely script of `language` in `region` where the region changes it (CLDR's
/// `lang_REGION` likely subtags), for the languages it changes between a right-to-left script
/// and another; `und` is a locale naming only its region.
fn likely_script_in_region(language: &str, region: &str) -> Option<&'static str> {
    Some(match (language, region) {
        ("pa", "PK") | ("az", "IR" | "IQ") | ("uz", "AF") | ("tk", "AF") => "Arab",
        ("kk", "CN" | "AF" | "IR") | ("ky", "CN") | ("tg", "PK") => "Arab",
        ("ha", "CM" | "SD") | ("ms", "CC") => "Arab",
        ("man", "GN") => "Nkoo",
        ("sd", "IN") => "Deva",
        ("ug", "KZ" | "MN") => "Cyrl",
        (
            "und",
            "AE" | "AF" | "BH" | "DZ" | "EG" | "EH" | "IQ" | "IR" | "JO" | "KM" | "KW" | "LB"
            | "LY" | "MA" | "MR" | "OM" | "PK" | "PS" | "QA" | "SA" | "SD" | "SS" | "SY" | "TN"
            | "YE",
        ) => "Arab",
        ("und", "IL") => "Hebr",
        ("und", "MV") => "Thaa",
        _ => return None,
    })
}

/// The likely script of `language` when it's written right to left (CLDR's likely subtags);
/// `None` for the others.
fn likely_script(language: &str) -> Option<&'static str> {
    Some(match language {
        // Arabic script: Arabic's varieties, Persian's and Kurdish's of Iran and Iraq, the
        // languages of Pakistan, Afghanistan and western China written in it.
        "ar" | "aao" | "abv" | "acm" | "acq" | "aeb" | "afb" | "ajp" | "apc" | "apd" | "arq"
        | "ars" | "ary" | "arz" | "auz" | "avl" | "ayh" | "ayl" | "ayp" | "shu" | "ssh" => "Arab",
        "fa" | "prs" | "haz" | "glk" | "mzn" | "lrc" | "luz" | "bqi" | "sdh" | "ckb" | "lki"
        | "hac" | "gbz" | "rmt" | "prd" => "Arab",
        "ur" | "ps" | "pbt" | "sd" | "ks" | "ug" | "bal" | "bcc" | "bgn" | "brh" | "skr"
        | "pnb" | "lah" | "hnd" | "hno" | "khw" | "bft" | "gjk" | "gju" | "kvx" | "kxp" | "mki"
        | "mvy" | "odk" | "trw" | "wne" | "dcc" => "Arab",
        "bej" | "fia" | "mfa" | "swb" | "cja" | "ota" => "Arab",
        // Hebrew script: Hebrew, Yiddish, Ladino and the Judeo- languages.
        "he" | "iw" | "yi" | "ji" | "lad" | "jrb" | "jpr" => "Hebr",
        "syr" | "aii" | "cld" => "Syrc",
        "dv" => "Thaa",
        "nqo" => "Nkoo",
        "rhg" => "Rohg",
        "mid" | "myz" => "Mand",
        "smp" | "sam" => "Samr",
        // Languages known by their historic scripts.
        "arc" => "Armi",
        "phn" => "Phnx",
        "ae" => "Avst",
        "pal" => "Phli",
        "xpr" => "Prti",
        "pra" => "Khar",
        "sog" => "Sogd",
        "xmn" => "Mani",
        "xna" => "Narb",
        "xsa" => "Sarb",
        "xld" => "Lydi",
        "xmr" => "Merc",
        "otk" => "Orkh",
        "xco" => "Chrs",
        "oui" => "Ougr",
        _ => return None,
    })
}
