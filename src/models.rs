use serde::{de::Error as _, Deserialize, Deserializer};
use serde_json::Value;

/// Declares a response model and its `Deserialize`.
///
/// Each model keeps the untouched response body in `raw`, so a field added to
/// the API later is reachable without waiting for a client release. Serde
/// cannot both fill a struct and hand back the document it read, so the
/// generated impl reads a `Value` first and populates a private mirror from
/// it. The mirror is generated from the same field list rather than written
/// out, which is the point: two hand-kept lists would drift the first time
/// someone added a field to one of them.
macro_rules! model {
    (
        $(#[$meta:meta])*
        pub struct $name:ident {
            $($(#[$field_meta:meta])* pub $field:ident : $ty:ty),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq)]
        pub struct $name {
            $($(#[$field_meta])* pub $field: $ty,)*

            /// The untouched response body.
            pub raw: Value,
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                // Every field is read as an Option and then unwrapped to its
                // default. A missing key and an explicit null are different
                // things to serde, and the API sends both: `branch_code` comes
                // back as null when a country's format has no branch. Reading
                // the field as its own type would take the null as a type
                // error and fail the whole response.
                #[derive(Deserialize)]
                struct Fields {
                    $(
                        $(#[$field_meta])*
                        #[serde(default)]
                        $field: Option<$ty>,
                    )*
                }

                let raw = Value::deserialize(deserializer)?;
                let fields: Fields = serde_json::from_value(raw.clone()).map_err(D::Error::custom)?;

                Ok($name {
                    $($field: fields.$field.unwrap_or_default(),)*
                    raw,
                })
            }
        }
    };
}

// Tri-state fields are Option<bool> on purpose. The API distinguishes false
// from absent, and None means "not known for this IBAN", which is not the same
// answer as Some(false).

model! {
    /// The result of validating one IBAN.
    ///
    /// `valid` is the primary flag. When it is false only `iban`, `formatted`,
    /// `country`, `country_name`, `error` and `error_code` are populated.
    pub struct ValidationResult {
        /// Whether the IBAN passes the ISO 13616 check digit, the country's
        /// length and the country's BBAN structure.
        pub valid: bool,
        /// The IBAN as submitted, with spaces removed and letters uppercased.
        pub iban: String,
        /// The IBAN in groups of four, the way it is written on paper.
        pub formatted: String,
        /// The two ISO 13616 check digits, positions three and four.
        pub check_digits: String,
        /// The Basic Bank Account Number: everything after the check digits.
        pub bban: String,
        /// The ISO 3166-1 alpha-2 country code, positions one and two.
        pub country: String,
        /// The country's name in English.
        pub country_name: String,

        /// The institution the bank code resolves to, empty when it is not in
        /// the directory.
        pub bank_name: String,
        /// How the institution is classified, such as `private` or `central`.
        pub bank_type: String,
        /// The institution's BIC, at head-office level.
        pub bic: String,
        /// The city on record for the institution.
        pub bank_city: String,
        /// The domestic bank identifier carried inside the BBAN.
        pub bank_code: String,
        /// The branch identifier, where the country's format has one.
        pub branch_code: String,
        /// The account number carried inside the BBAN.
        pub account_number: String,

        /// A domestic account check digit run on top of the ISO 13616
        /// checksum, such as Germany's per-bank Prüfziffer or the UK
        /// sort-code and account modulus check.
        ///
        /// It is advisory: an IBAN with `valid` true is a valid IBAN whatever
        /// this says. `Some(false)` usually means a transcription error in the
        /// account number. `None` where the country has no such scheme.
        pub national_check_valid: Option<bool>,

        /// The country's ISO 4217 currency code.
        pub currency: String,
        /// The currency's name in English.
        pub currency_name: String,
        /// Which rails the country can be paid over, such as `SEPA+SWIFT`.
        pub transfer_type: String,

        /// Whether the IBAN's country is in the SEPA zone. `None` when the API
        /// did not say.
        pub sepa: Option<bool>,
        /// The country's flag, as an emoji.
        pub flag: String,

        /// Why `valid` is false, in words.
        pub error: String,
        /// Why `valid` is false, as a code such as `INVALID_COUNTRY`.
        pub error_code: String,
    }
}

model! {
    /// The result of a bulk validation or a text extraction. Results come back
    /// in the same order as the input.
    pub struct BatchResult {
        /// How many IBANs came back.
        pub count: u32,
        /// How many of them are valid.
        pub valid_count: u32,
        /// How many of them are not.
        pub invalid_count: u32,
        /// One result per IBAN, in input order.
        pub results: Vec<ValidationResult>,
    }
}

model! {
    /// One segment of a country's BBAN, in the order it appears in the IBAN.
    pub struct BbanField {
        /// What the country calls this segment, such as `BLZ`.
        pub label: String,
        /// How many characters it occupies.
        pub length: u32,
        /// The character class it accepts, such as `numeric`.
        pub r#type: String,
        /// What the segment identifies, in words.
        pub description: String,
    }
}

model! {
    /// The IBAN format specification for one country.
    pub struct FormatSpec {
        /// The ISO 3166-1 alpha-2 country code.
        pub country_code: String,
        /// The country's name in English.
        pub country_name: String,
        /// The total IBAN length for this country.
        pub length: u32,
        /// The country's ISO 4217 currency code.
        pub currency: String,
        /// The currency's name in English.
        pub currency_name: String,
        /// Whether the country is in the SEPA zone.
        pub sepa: Option<bool>,
        /// Whether the country is in the SWIFT IBAN Registry.
        pub swift: Option<bool>,
        /// The format as a mask, such as `DEkk nnnn nnnn nnnn nnnn nn`.
        pub format_string: String,
        /// The registry's own sample IBAN. Structurally valid, not a real
        /// account, and its bank code often belongs to no institution.
        pub example: String,
        /// The BBAN broken into its segments, in order.
        pub bban_fields: Vec<BbanField>,
    }
}

model! {
    /// The institution behind a SWIFT/BIC code.
    pub struct BankRecord {
        /// The full BIC, 8 or 11 characters.
        pub bic: String,
        /// The first eight characters, which identify the institution.
        pub bic8: String,
        /// The institution code, characters one to four.
        pub bank_code: String,
        /// The country code, characters five and six.
        pub country_code: String,
        /// The location code, characters seven and eight.
        pub location_code: String,
        /// The branch code, characters nine to eleven, where the BIC has one.
        pub branch_code: String,
        /// The institution's name.
        pub bank_name: String,
        /// The city on record for it.
        pub city: String,
        /// The country's name in English.
        pub country_name: String,
        /// Whether the institution is registered for SEPA schemes.
        pub sepa: Option<bool>,
        /// How the institution is classified, such as `private` or `central`.
        pub r#type: String,
        /// Whether the institution is `active`, `merged` or `closed`.
        pub status: String,
    }
}
