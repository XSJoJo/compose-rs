//! `Locale` and `LocaleList`, as Compose's `intl/LocaleTest` and `intl/LocaleListTest`.

use compose_ui::{Locale, LocaleList};

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/intl/LocaleTest.kt, forLanguageTag
#[test]
fn for_language_tag() {
    let locale = Locale::new("en-US");
    assert_eq!(locale.language(), "en");
    assert_eq!(locale.script(), "");
    assert_eq!(locale.region(), "US");
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/intl/LocaleTest.kt, forLanguageTag_with_script
#[test]
fn for_language_tag_with_script() {
    let locale = Locale::new("sr-Latn-SR");
    assert_eq!(locale.language(), "sr");
    assert_eq!(locale.script(), "Latn");
    assert_eq!(locale.region(), "SR");
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/intl/LocaleTest.kt, equals_language
#[test]
fn equals_language() {
    assert_eq!(Locale::new("ja"), Locale::new("ja"));
    assert_ne!(Locale::new("ja"), Locale::new("en"));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/intl/LocaleTest.kt, equals_language_region
#[test]
fn equals_language_region() {
    assert_eq!(Locale::new("en-US"), Locale::new("en-US"));
    assert_ne!(Locale::new("en-US"), Locale::new("en-GB"));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/intl/LocaleTest.kt, equals_script
#[test]
fn equals_script() {
    assert_eq!(Locale::new("sr-Latn-SR"), Locale::new("sr-Latn-SR"));
    assert_ne!(Locale::new("sr-Latn-SR"), Locale::new("sr-Cyrl-SR"));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/intl/LocaleListTest.kt, forLanguageTag_single
#[test]
fn for_language_tag_single() {
    let list = LocaleList::from_language_tags("en-US");
    assert_eq!(list.len(), 1);
    assert_eq!(list.get(0), Some(&Locale::new("en-US")));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/intl/LocaleListTest.kt, forLanguageTag_multiple
#[test]
fn for_language_tag_multiple() {
    let list = LocaleList::from_language_tags("en-US,ja-JP");
    assert_eq!(list.len(), 2);
    assert_eq!(list.get(0), Some(&Locale::new("en-US")));
    assert_eq!(list.get(1), Some(&Locale::new("ja-JP")));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/intl/LocaleListTest.kt, equals_order_matters
#[test]
fn equals_order_matters() {
    assert_ne!(
        LocaleList::from_language_tags("ja-JP,en-US"),
        LocaleList::from_language_tags("en-US,ja-JP")
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/intl/LocaleListTest.kt, equals
#[test]
fn equals() {
    assert_eq!(
        LocaleList::from_language_tags("en-US,ja-JP"),
        LocaleList::from_language_tags("en-US,ja-JP")
    );
    assert_ne!(
        LocaleList::from_language_tags("en-US,ja-JP"),
        LocaleList::from_language_tags("en-US,es-ES")
    );
}
