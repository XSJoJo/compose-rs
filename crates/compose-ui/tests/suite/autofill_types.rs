//! Autofill's types (D1680): each content type's Android hint (`AndroidContentTypeTest`), each
//! data type's Android type (`AndroidContentDataTypeTest`), a value's typed accessors
//! (`AndroidFillableDataTest`) and the text kept to the platform's maximum
//! (`AutofillUtilsTest`).

use compose_ui::platform::{MAX_AUTOFILL_TEXT_LENGTH, trim_to_safe_length};
use compose_ui::{ContentDataType, ContentType, FillableData};

/// Each test checks a predefined content type has the hint `androidx.autofill.HintConstants`
/// names, as `ContentType(hint)` would.
macro_rules! content_type_tests {
    ($($(#[$doc:meta])* $name:ident: $constant:ident = $hint:literal;)*) => {$(
        $(#[$doc])*
        #[test]
        fn $name() {
            assert_eq!(
                ContentType::$constant.content_hints(),
                ContentType::new($hint).content_hints()
            );
        }
    )*};
}

content_type_tests! {
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, emailAddress
    email_address: EMAIL_ADDRESS = "emailAddress";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, username
    username: USERNAME = "username";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, password
    password: PASSWORD = "password";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, newUsername
    new_username: NEW_USERNAME = "newUsername";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, newPassword
    new_password: NEW_PASSWORD = "newPassword";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, postalAddress
    postal_address: POSTAL_ADDRESS = "postalAddress";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, postalCode
    postal_code: POSTAL_CODE = "postalCode";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, creditCardNumber
    credit_card_number: CREDIT_CARD_NUMBER = "creditCardNumber";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, creditCardSecurityCode
    credit_card_security_code: CREDIT_CARD_SECURITY_CODE = "creditCardSecurityCode";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, creditCardExpirationDate
    credit_card_expiration_date: CREDIT_CARD_EXPIRATION_DATE = "creditCardExpirationDate";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, creditCardExpirationMonth
    credit_card_expiration_month: CREDIT_CARD_EXPIRATION_MONTH = "creditCardExpirationMonth";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, creditCardExpirationYear
    credit_card_expiration_year: CREDIT_CARD_EXPIRATION_YEAR = "creditCardExpirationYear";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, creditCardExpirationDay
    credit_card_expiration_day: CREDIT_CARD_EXPIRATION_DAY = "creditCardExpirationDay";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, addressCountry
    address_country: ADDRESS_COUNTRY = "addressCountry";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, addressRegion
    address_region: ADDRESS_REGION = "addressRegion";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, addressLocality
    address_locality: ADDRESS_LOCALITY = "addressLocality";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, addressStreet
    address_street: ADDRESS_STREET = "streetAddress";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, addressAuxiliaryDetails
    address_auxiliary_details: ADDRESS_AUXILIARY_DETAILS = "extendedAddress";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, postalCodeExtended
    postal_code_extended: POSTAL_CODE_EXTENDED = "extendedPostalCode";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, personFullName
    person_full_name: PERSON_FULL_NAME = "personName";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, personFirstName
    person_first_name: PERSON_FIRST_NAME = "personGivenName";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, personLastName
    person_last_name: PERSON_LAST_NAME = "personFamilyName";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, personMiddleName
    person_middle_name: PERSON_MIDDLE_NAME = "personMiddleName";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, personMiddleInitial
    person_middle_initial: PERSON_MIDDLE_INITIAL = "personMiddleInitial";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, personNamePrefix
    person_name_prefix: PERSON_NAME_PREFIX = "personNamePrefix";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, personNameSuffix
    person_name_suffix: PERSON_NAME_SUFFIX = "personNameSuffix";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, phoneNumber
    phone_number: PHONE_NUMBER = "phoneNumber";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, phoneNumberDevice
    phone_number_device: PHONE_NUMBER_DEVICE = "phoneNumberDevice";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, phoneCountryCode
    phone_country_code: PHONE_COUNTRY_CODE = "phoneCountryCode";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, phoneNumberNational
    phone_number_national: PHONE_NUMBER_NATIONAL = "phoneNational";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, gender
    gender: GENDER = "gender";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, birthDateFull
    birth_date_full: BIRTH_DATE_FULL = "birthDateFull";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, birthDateDay
    birth_date_day: BIRTH_DATE_DAY = "birthDateDay";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, birthDateMonth
    birth_date_month: BIRTH_DATE_MONTH = "birthDateMonth";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, birthDateYear
    birth_date_year: BIRTH_DATE_YEAR = "birthDateYear";
    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentTypeTest.kt, smsOTPCode
    sms_otp_code: SMS_OTP_CODE = "smsOTPCode";
}

/// compose-rs's own: `plus` keeps both sets of hints, each once, the first's first; content
/// types are equal as sets.
#[test]
fn plus_unites_the_hints() {
    let hints = |content_type: &ContentType| -> Vec<String> {
        content_type.content_hints().iter().map(|hint| hint.to_string()).collect()
    };
    let both = ContentType::USERNAME.plus(&ContentType::EMAIL_ADDRESS);
    assert_eq!(hints(&both), ["username", "emailAddress"]);
    assert_eq!(hints(&both.plus(&ContentType::USERNAME)), ["username", "emailAddress"]);
    assert_eq!(ContentType::USERNAME + ContentType::USERNAME, ContentType::USERNAME);
    assert_eq!(both, ContentType::EMAIL_ADDRESS + ContentType::USERNAME);
    assert_ne!(both, ContentType::USERNAME);
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentDataTypeTest.kt, assertTextTypeEquals
#[test]
fn text_data_type() {
    assert_eq!(ContentDataType::TEXT.data_type(), ContentDataType::new(1).data_type());
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentDataTypeTest.kt, assertListTypeEquals
#[test]
fn list_data_type() {
    assert_eq!(ContentDataType::LIST.data_type(), ContentDataType::new(3).data_type());
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentDataTypeTest.kt, assertDateTypeEquals
#[test]
fn date_data_type() {
    assert_eq!(ContentDataType::DATE.data_type(), ContentDataType::new(4).data_type());
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentDataTypeTest.kt, assertToggleTypeEquals
#[test]
fn toggle_data_type() {
    assert_eq!(ContentDataType::TOGGLE.data_type(), ContentDataType::new(2).data_type());
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AndroidContentDataTypeTest.kt, assertNoneTypeEquals
#[test]
fn none_data_type() {
    assert_eq!(ContentDataType::NONE.data_type(), ContentDataType::new(0).data_type());
}

// ---- Fillable data -----------------------------------------------------------------------------

const TEST_STRING: &str = "TEST_STRING";
const TEST_BOOLEAN: bool = true;
const TEST_INT: i32 = 123;
/// January 1, 2000, in milliseconds.
const TEST_LONG: i64 = 946_684_800_000;

fn text() -> FillableData {
    FillableData::from_platform_text(TEST_STRING)
}

fn toggle() -> FillableData {
    FillableData::create_from_boolean(TEST_BOOLEAN).unwrap()
}

fn list() -> FillableData {
    FillableData::create_from_list_index(TEST_INT).unwrap()
}

fn date() -> FillableData {
    FillableData::create_from_date_millis(TEST_LONG).unwrap()
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidFillableDataTest.kt, textValue_whenValueIsText_returnsText
#[test]
fn a_text_s_text() {
    assert_eq!(text().text_value().map(|it| it.as_str()), Some(TEST_STRING));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidFillableDataTest.kt, booleanValue_whenValueIsText_returnsNull
#[test]
fn a_text_has_no_boolean() {
    assert_eq!(text().boolean_value(), None);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidFillableDataTest.kt, listIndexValue_whenValueIsText_returnsNull
#[test]
fn a_text_has_no_list_index() {
    assert_eq!(text().list_index_value(), None);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidFillableDataTest.kt, booleanValue_whenValueIsToggle_returnsBoolean
#[test]
fn a_toggle_s_boolean() {
    assert_eq!(toggle().boolean_value(), Some(TEST_BOOLEAN));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidFillableDataTest.kt, textValue_whenValueIsToggle_returnsNull
#[test]
fn a_toggle_has_no_text() {
    assert_eq!(toggle().text_value(), None);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidFillableDataTest.kt, listIndexValue_whenValueIsToggle_returnsNull
#[test]
fn a_toggle_has_no_list_index() {
    assert_eq!(toggle().list_index_value(), None);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidFillableDataTest.kt, listIndexValue_whenValueIsList_returnsInt
#[test]
fn a_list_s_index() {
    assert_eq!(list().list_index_value(), Some(TEST_INT));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidFillableDataTest.kt, textValue_whenValueIsList_returnsNull
#[test]
fn a_list_has_no_text() {
    assert_eq!(list().text_value(), None);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidFillableDataTest.kt, booleanValue_whenValueIsList_returnsNull
#[test]
fn a_list_has_no_boolean() {
    assert_eq!(list().boolean_value(), None);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidFillableDataTest.kt, getListIndexOrDefault_whenValueIsList_returnsInt
#[test]
fn a_list_s_index_over_a_default() {
    assert_eq!(list().get_list_index_or_default(-1), TEST_INT);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidFillableDataTest.kt, getListIndexOrDefault_whenValueIsText_returnsDefault
#[test]
fn a_text_s_list_index_is_the_default() {
    assert_eq!(text().get_list_index_or_default(-1), -1);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidFillableDataTest.kt, dateMillisValue_whenValueIsList_returnsNull
#[test]
fn a_list_has_no_date() {
    assert_eq!(list().date_millis_value(), None);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidFillableDataTest.kt, textValue_whenValueIsDate_returnsNull
#[test]
fn a_date_has_no_text() {
    assert_eq!(date().text_value(), None);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidFillableDataTest.kt, booleanValue_whenValueIsDate_returnsNull
#[test]
fn a_date_has_no_boolean() {
    assert_eq!(date().boolean_value(), None);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidFillableDataTest.kt, listIndexValue_whenValueIsDate_returnsNull
#[test]
fn a_date_has_no_list_index() {
    assert_eq!(date().list_index_value(), None);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidFillableDataTest.kt, dateMillisValue_whenValueIsDate_returnsLong
#[test]
fn a_date_s_millis() {
    assert_eq!(date().date_millis_value(), Some(TEST_LONG));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidFillableDataTest.kt, getDateMillisOrDefault_whenValueIsDate_returnsLong
#[test]
fn a_date_s_millis_over_a_default() {
    assert_eq!(date().get_date_millis_or_default(-1), TEST_LONG);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidFillableDataTest.kt, getDateMillisOrDefault_whenValueIsText_returnsDefault
#[test]
fn a_text_s_date_is_the_default() {
    assert_eq!(text().get_date_millis_or_default(-1), -1);
}

// ---- The platform's maximum --------------------------------------------------------------------

fn utf16_length(text: &str) -> usize {
    text.encode_utf16().count()
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AutofillUtilsTest.kt, getAutofillTextValue_lessThanMax
#[test]
fn a_text_under_the_maximum_is_kept() {
    let text = "a".repeat(MAX_AUTOFILL_TEXT_LENGTH - 1);
    assert_eq!(trim_to_safe_length(&text), text);
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AutofillUtilsTest.kt, getAutofillTextValue_equalToMax
#[test]
fn a_text_at_the_maximum_is_kept() {
    let text = "a".repeat(MAX_AUTOFILL_TEXT_LENGTH);
    assert_eq!(trim_to_safe_length(&text), text);
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AutofillUtilsTest.kt, getAutofillTextValue_moreThanMax
#[test]
fn a_text_over_the_maximum_is_cut() {
    let text = "a".repeat(MAX_AUTOFILL_TEXT_LENGTH + 1);
    let kept = trim_to_safe_length(&text);
    assert_eq!(utf16_length(kept), MAX_AUTOFILL_TEXT_LENGTH);
    assert_eq!(kept, &text[..MAX_AUTOFILL_TEXT_LENGTH]);
}

/// A text whose surrogate pair straddles the maximum.
fn straddling() -> (String, String) {
    let prefix = "a".repeat(MAX_AUTOFILL_TEXT_LENGTH - 1);
    let text = format!("{prefix}\u{1F600}b");
    (prefix, text)
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AutofillUtilsTest.kt, getAutofillTextValue_surrogatePairAtMax
#[test]
fn a_surrogate_pair_at_the_maximum_is_left_out_whole() {
    let (prefix, text) = straddling();
    let kept = trim_to_safe_length(&text);
    assert_eq!(utf16_length(kept), MAX_AUTOFILL_TEXT_LENGTH - 1);
    assert_eq!(kept, prefix);
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AutofillUtilsTest.kt, createFromText_moreThanMax
#[test]
fn text_data_over_the_maximum_is_cut() {
    let text = "a".repeat(MAX_AUTOFILL_TEXT_LENGTH + 1);
    let data = FillableData::create_from_text(text.clone()).unwrap();
    let kept = data.text_value().unwrap().as_str();
    assert_eq!(utf16_length(kept), MAX_AUTOFILL_TEXT_LENGTH);
    assert_eq!(kept, &text[..MAX_AUTOFILL_TEXT_LENGTH]);
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/autofill/AutofillUtilsTest.kt, createFromText_surrogatePairAtMax
#[test]
fn text_data_with_a_surrogate_pair_at_the_maximum_leaves_it_out() {
    let (prefix, text) = straddling();
    let data = FillableData::create_from_text(text).unwrap();
    let kept = data.text_value().unwrap().as_str();
    assert_eq!(utf16_length(kept), MAX_AUTOFILL_TEXT_LENGTH - 1);
    assert_eq!(kept, prefix);
}

/// compose-rs's own: the maximum counts UTF-16 units, not bytes: a text of two-byte characters
/// under 5000 units is kept, though longer in bytes.
#[test]
fn the_maximum_counts_utf16_units() {
    let text = "é".repeat(MAX_AUTOFILL_TEXT_LENGTH);
    assert_eq!(trim_to_safe_length(&text), text);
    let longer = "é".repeat(MAX_AUTOFILL_TEXT_LENGTH + 1);
    assert_eq!(utf16_length(trim_to_safe_length(&longer)), MAX_AUTOFILL_TEXT_LENGTH);
}
