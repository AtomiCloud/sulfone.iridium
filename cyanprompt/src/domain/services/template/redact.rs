//! Shared redaction primitives for the headless error surface.
//!
//! Headless errors (a coordinator validation rejection or a transport error) must never
//! echo a supplied answer value — especially a secret (`Password`) value (the no-leak
//! contract). Two call sites enforce this: the in-flight redactor in
//! [`engine`](super::engine) (for `prompt_template` transport errors during the walk) and
//! the validate-path redactor in [`validate`](super::validate) (for `validate_template`
//! rejections + transport errors). They previously each rendered an [`Answer`] to its
//! echoable string forms with their own copy of the logic — and drifted (one tracked
//! booleans, the other did not). This module is the single source of truth for BOTH:
//! - [`answer_renderings`] / [`answer_map_renderings`]: an answer's echoable string forms,
//!   with ONE boolean policy.
//! - [`value_echoed`]: whether an error message may be echoing a value, matching the raw
//!   value AND its common encoded renderings (JSON-string escaping and percent/form-encoding)
//!   so an encoded echo cannot slip past a naive substring check.

use std::collections::HashMap;

use crate::domain::models::answer::Answer;

/// Render an answer's value(s) into the string forms a coordinator/transport error might
/// echo. The single source of truth shared by the in-flight redactor
/// ([`engine`](super::engine)) and the validate-path sibling capture
/// ([`validate`](super::validate)) so the two cannot drift.
///
/// Boolean answers are intentionally NOT tracked: a leaked `true`/`false` reveals nothing
/// (a `Confirm` carries no private value), while those substrings are common enough in
/// error text that redacting on them would blank otherwise-useful, non-leaking messages.
/// The secrecy contract forbids echoing a supplied answer VALUE, and a boolean has none.
/// Empty strings are skipped — the substring check treats `""` as matching everything and
/// it leaks nothing, so tracking it would only force spurious redaction.
pub(crate) fn answer_renderings(answer: &Answer) -> Vec<String> {
    match answer {
        Answer::String(value) => non_empty(value),
        Answer::StringArray(arr) => arr.iter().flat_map(|s| non_empty(s)).collect(),
        // Booleans are intentionally NOT tracked — see the doc comment above.
        Answer::Bool(_) => Vec::new(),
    }
}

/// Every supplied answer value across a map, rendered to its echoable string forms via
/// [`answer_renderings`]. Used by the validate path to capture sibling values (the
/// coordinator validator receives the full sibling map, so its rejection can echo any
/// sibling value, including an earlier `Password`).
pub(crate) fn answer_map_renderings(answers: &HashMap<String, Answer>) -> Vec<String> {
    answers.values().flat_map(answer_renderings).collect()
}

fn non_empty(s: &str) -> Vec<String> {
    if s.is_empty() {
        Vec::new()
    } else {
        vec![s.to_string()]
    }
}

/// True when `message` may be echoing `value` — directly OR in a common encoded form.
///
/// Coordinator/transport errors frequently embed request/response bodies as JSON, so a
/// value containing quotes, backslashes, or control characters appears ESCAPED in the
/// error text (`pa"ss` → `pa\"ss`). They may also embed a payload that has been
/// URL/form-encoded (`pa@ss/word` → `pa%40ss%2Fword`, a space as `%20`/`+`, lowercase hex such
/// as `%2f`, or a form-only rendering such as `~` → `%7E`) when a request travels over a URL,
/// form body, or query string, or passes through a proxy that re-encodes its body. A raw
/// `message.contains(value)` check would miss any encoded rendering and leak the value
/// (a secrecy bypass — an error must never echo a supplied answer value). Matching the
/// value's encoded variants in addition to the raw form closes both bypasses.
///
/// Redaction is NOT gated on value length — a short value (a 2-char PIN, region code, or
/// replica id) echoed verbatim is just as much a leak as a long one. The only guard is the
/// empty string (which `contains` always matches and which leaks nothing anyway).
pub(crate) fn value_echoed(message: &str, value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    encoded_variants(value)
        .iter()
        .any(|variant| message.contains(variant))
}

/// The forms a tracked value can take in error text: the raw value plus its common encoded
/// renderings — JSON-string escaping (how the value looks embedded inside a JSON string in a
/// serialized request/response body) and URL/form encoding (how the value looks embedded in a
/// URL path, query string, or form body). The URL-style renderings vary on independent axes,
/// and a coordinator/proxy/transport error can echo any combination, so every combination is
/// produced and matched:
/// - **encoding standard (space byte + safe character set):** RFC 3986 percent-encoding and
///   `application/x-www-form-urlencoded` differ on the space byte (`%20` vs `+`) AND on which
///   bytes are left literal. RFC 3986 leaves `-._~` literal and percent-encodes `*` (`%2A`);
///   form-urlencoding leaves `*-._` literal and percent-encodes `~` (`%7E`). So a secret such
///   as `a*b~c` renders as `a%2Ab~c` under RFC 3986 but `a*b%7Ec` under form encoding — two
///   distinct strings, both produced (see [`UrlEncoding`]).
/// - **hex case:** RFC 3986 §2.1 prefers UPPERCASE hex (`%2F`) but explicitly treats lowercase
///   (`%2f`) as equivalent, and real encoders/proxies emit either — so both `%HH` casings are
///   produced (the literal/unreserved bytes and the value's own characters keep their original
///   case, which is why the whole string cannot simply be lower-cased).
///
/// Each candidate is added only when it is not already present, so a value with no special
/// characters contributes a single variant, a value whose two encoding standards render
/// identically (no space, no `*`/`~`) collapses to one URL form, and a value with no `%HH`
/// letters collapses the two hex casings — only a value whose encoding genuinely differs on an
/// axis contributes the extra variant.
fn encoded_variants(value: &str) -> Vec<String> {
    let mut variants = vec![value.to_string()];
    let candidates = [
        json_string_escape(value),
        url_encode(value, UrlEncoding::Rfc3986, HexCase::Upper),
        url_encode(value, UrlEncoding::Rfc3986, HexCase::Lower),
        url_encode(value, UrlEncoding::Form, HexCase::Upper),
        url_encode(value, UrlEncoding::Form, HexCase::Lower),
    ];
    for candidate in candidates {
        if !candidate.is_empty() && !variants.contains(&candidate) {
            variants.push(candidate);
        }
    }
    variants
}

/// The body of `value` as JSON encodes it inside a string literal (escaping `"`, `\`,
/// control characters, etc.), WITHOUT the surrounding quotes — i.e. the substring that
/// would appear if `value` were embedded in a larger JSON document.
///
/// `serde_json::to_string` of a `&str` cannot fail and always yields a quoted string `"…"`,
/// so both the serialization and the quote-strip are guaranteed for any `&str`. These are
/// serde_json invariants, not reachable input failures, so they are asserted with `expect`
/// rather than plumbed through an impossible `None`/error fallback.
fn json_string_escape(value: &str) -> String {
    let encoded =
        serde_json::to_string(value).expect("serde_json::to_string of a &str is infallible");
    encoded
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .expect("serde_json always encodes a &str as a quoted string")
        .to_string()
}

/// Which URL-encoding standard [`url_encode`] follows. URL path/query encoding (RFC 3986) and
/// `application/x-www-form-urlencoded` differ on TWO things, both of which change the rendered
/// string, so a value is produced under both and an echo in either form is detected:
/// - **the space byte:** RFC 3986 emits `%20`, form-urlencoding emits `+`.
/// - **the safe (left-literal) character set:** RFC 3986's unreserved set is `-._~` (and it
///   percent-encodes `*` → `%2A`); form-urlencoding leaves `*-._` literal (and percent-encodes
///   `~` → `%7E`). Common form encoders (e.g. WHATWG `URLSearchParams` / Node) use the latter,
///   so a secret containing `~` or `*` renders differently under the two standards.
///
/// Every other non-safe byte is `%HH` in both.
enum UrlEncoding {
    /// RFC 3986 (URL path / query): space → `%20`, unreserved `-._~` left literal, `*` → `%2A`.
    Rfc3986,
    /// `application/x-www-form-urlencoded` (WHATWG / Node `URLSearchParams`): space → `+`,
    /// safe set `*-._` left literal, `~` → `%7E`.
    Form,
}

/// The hex-digit case [`url_encode`] uses for its `%HH` byte escapes. RFC 3986 §2.1 prefers
/// uppercase (`%2F`) but explicitly defines lowercase (`%2f`) as equivalent, and real
/// encoders/proxies emit either — so a value is rendered under both casings and an echo in
/// either is detected.
enum HexCase {
    /// RFC 3986 §2.1 preferred form: `%2F`.
    Upper,
    /// Equivalent lowercase form many encoders emit: `%2f`.
    Lower,
}

/// `value` URL-encoded the way a coordinator/proxy/transport error would echo a request that
/// carried it in a URL path, query string, or form body: every byte outside the `encoding`'s
/// safe (left-literal) set becomes `%HH`, except the space, which becomes `%20` (RFC 3986) or
/// `+` (form) per `encoding` (see [`UrlEncoding`]). The safe set itself depends on `encoding`:
/// RFC 3986 leaves `-._~` literal and encodes `*`; form-urlencoding leaves `*-._` literal and
/// encodes `~`. The `%HH` hex digits are emitted in upper- or lower-case per `hex_case` (see
/// [`HexCase`]); the literal/safe bytes keep their original case regardless. This is the
/// substring that would appear if such a payload were echoed back in an error.
///
/// Implemented inline rather than pulling in the `percent-encoding`/`form_urlencoded` crates:
/// the redactor must produce BOTH encoding standards (each with its own space rendering AND
/// safe character set) AND both hex casings to match every echo a proxy might emit — coverage
/// neither crate offers (they emit a single canonical uppercase form for one standard). A
/// small, total, fully-tested byte mapping that emits every rendering is therefore both more
/// capable here and a zero-new-dependency change to the secrecy primitive.
fn url_encode(value: &str, encoding: UrlEncoding, hex_case: HexCase) -> String {
    // The safe (left-literal) set differs by standard: RFC 3986's unreserved set is `-._~`
    // (and `*` is encoded); form-urlencoding leaves `*-._` literal (and `~` is encoded).
    // ALPHA / DIGIT are safe in both. Every other byte is `%HH`, except a space under form
    // encoding, which is `+`.
    let safe: &[u8] = match encoding {
        UrlEncoding::Rfc3986 => b"-._~",
        UrlEncoding::Form => b"*-._",
    };
    const HEX_UPPER: &[u8; 16] = b"0123456789ABCDEF";
    const HEX_LOWER: &[u8; 16] = b"0123456789abcdef";
    let hex = match hex_case {
        HexCase::Upper => HEX_UPPER,
        HexCase::Lower => HEX_LOWER,
    };
    let mut out = String::with_capacity(value.len());
    for &byte in value.as_bytes() {
        if byte.is_ascii_alphanumeric() || safe.contains(&byte) {
            out.push(byte as char);
        } else if byte == b' ' && matches!(encoding, UrlEncoding::Form) {
            out.push('+');
        } else {
            out.push('%');
            out.push(hex[(byte >> 4) as usize] as char);
            out.push(hex[(byte & 0x0f) as usize] as char);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // The single boolean policy: String/StringArray values are tracked, Bool is not, and
    // empty strings are skipped. Both redaction surfaces now share this one renderer, so
    // the two cannot drift (the prior duplicate-with-contradictory-bool-policy hazard).
    #[test]
    fn answer_renderings_tracks_strings_skips_bools_and_empties() {
        assert_eq!(
            answer_renderings(&Answer::String("s3cr3t".to_string())),
            vec!["s3cr3t".to_string()]
        );
        assert!(answer_renderings(&Answer::String(String::new())).is_empty());
        assert_eq!(
            answer_renderings(&Answer::StringArray(vec![
                "a".to_string(),
                String::new(),
                "b".to_string(),
            ])),
            vec!["a".to_string(), "b".to_string()]
        );
        // A boolean carries no private value and is never tracked.
        assert!(answer_renderings(&Answer::Bool(true)).is_empty());
        assert!(answer_renderings(&Answer::Bool(false)).is_empty());
    }

    #[test]
    fn answer_map_renderings_flattens_every_value() {
        let mut map = HashMap::new();
        map.insert("a".to_string(), Answer::String("x".to_string()));
        map.insert("b".to_string(), Answer::Bool(true));
        let mut got = answer_map_renderings(&map);
        got.sort();
        assert_eq!(got, vec!["x".to_string()]);
    }

    // A raw-substring echo is caught.
    #[test]
    fn value_echoed_matches_raw_value() {
        assert!(value_echoed(
            "validator failed for value s3cr3t-token",
            "s3cr3t-token"
        ));
        assert!(!value_echoed(
            "validator failed for some other reason",
            "s3cr3t-token"
        ));
    }

    // FR11 bypass guard: a value containing a quote appears JSON-escaped (`pa\"ss`) when a
    // coordinator/transport error embeds the request body as JSON. The raw value (`pa"ss`)
    // is NOT a substring of the escaped text, but `value_echoed` still catches it via the
    // encoded variant — closing the escaped-echo leak.
    #[test]
    fn value_echoed_matches_json_escaped_value() {
        let secret = "pa\"ss";
        // The error text embeds the value the way serde_json would inside a JSON string.
        let error_text = r#"coordinator returned: {"token":"pa\"ss"}"#;
        assert!(
            !error_text.contains(secret),
            "raw value is NOT present verbatim — only the escaped form is"
        );
        assert!(
            value_echoed(error_text, secret),
            "the JSON-escaped rendering of the value must still be detected"
        );
    }

    // A backslash in the value is likewise escaped (`a\\b`) in JSON error bodies.
    #[test]
    fn value_echoed_matches_json_escaped_backslash() {
        let secret = "a\\b";
        let error_text = r#"{"path":"a\\b"}"#;
        assert!(value_echoed(error_text, secret));
    }

    // Empty value never matches (it would otherwise match everything).
    #[test]
    fn value_echoed_empty_value_never_matches() {
        assert!(!value_echoed("anything at all", ""));
    }

    // A value needing no escaping contributes exactly one variant (no spurious dup).
    #[test]
    fn encoded_variants_no_escape_is_single() {
        assert_eq!(encoded_variants("plain"), vec!["plain".to_string()]);
    }

    // A value with only special characters that are NOT percent-encoded (none here — a plain
    // ASCII value has no encoded form beyond itself) still yields just the raw variant.
    #[test]
    fn encoded_variants_alphanumeric_only_has_one() {
        // alphanumerics are unreserved in both JSON (no escaping) and percent-encoding
        // (no encoding), so no extra variant is produced.
        assert_eq!(encoded_variants("abc123"), vec!["abc123".to_string()]);
    }

    // FR11 bypass guard: a value containing a reserved character (e.g. `@`, `/`) can appear
    // PERCENT-ENCODED (`pa@ss/word` → `pa%40ss%2Fword`) when a coordinator/transport error
    // echoes a request that carried the value in a URL path, query string, or form body
    // (or through a proxy that re-encodes its body). The raw value is NOT a substring of the
    // encoded text, but `value_echoed` still catches it via the encoded variant — closing
    // the percent-encoded-echo leak on BOTH the validate-sibling and in-flight surfaces.
    #[test]
    fn value_echoed_matches_percent_encoded_value() {
        let secret = "pa@ss/word";
        // The error text embeds the value the way a URL/form-encoded payload would.
        let error_text = "validator failed: token=pa%40ss%2Fword is not allowed";
        assert!(
            !error_text.contains(secret),
            "raw value is NOT present verbatim — only the percent-encoded form is"
        );
        assert!(
            value_echoed(error_text, secret),
            "the percent-encoded rendering of the value must still be detected"
        );
    }

    // RFC 3986 percent-encoding renders a space as `%20`; a URL path/query echo is caught.
    #[test]
    fn value_echoed_matches_percent_encoded_space() {
        let secret = "two words";
        let error_text = "rejected value two%20words";
        assert!(
            !error_text.contains(secret),
            "raw value with space is NOT present verbatim"
        );
        assert!(
            value_echoed(error_text, secret),
            "the percent-encoded space must still be detected"
        );
    }

    // FR11 form-encoded-echo bypass: `application/x-www-form-urlencoded` renders a space as
    // `+` (NOT `%20`), so a value travelling in an HTML form body / proxy that re-encodes to
    // form format appears as `pa+ss%2Fword`. This form is distinct from the RFC 3986 `%20`
    // rendering, so it needs its own variant; `value_echoed` now matches it. This closes the
    // gap where a form-encoded echo (space → `+`) slipped past the `%20`-only encoding.
    #[test]
    fn value_echoed_matches_form_encoded_space() {
        let secret = "pa ss/word";
        // Form-encoded: space → `+`, `/` → `%2F`.
        let error_text = "rejected token=pa+ss%2Fword is not allowed";
        assert!(
            !error_text.contains(secret),
            "raw value is NOT present verbatim — only the form-encoded form is"
        );
        assert!(
            !error_text.contains("pa%20ss%2Fword"),
            "the message uses the `+` form, NOT the `%20` form"
        );
        assert!(
            value_echoed(error_text, secret),
            "the form-encoded (`+`-for-space) rendering of the value must be detected"
        );
    }

    // The URL/form encoder follows each standard's safe set: RFC 3986 leaves `-_.~` literal
    // (and encodes `*`), form-urlencoding leaves `*-._` literal (and encodes `~`); every other
    // byte is `%HH`. The space diverges by standard (`%20` vs `+`) and the hex digits diverge
    // by case (`%2F` vs `%2f`).
    #[test]
    fn url_encode_uses_per_standard_safe_set_and_space_modes() {
        // RFC 3986 unreserved: alphanumerics + `-_.~` pass through; `*` is encoded (`%2A`).
        assert_eq!(
            url_encode("AZaz09-_.~*", UrlEncoding::Rfc3986, HexCase::Upper),
            "AZaz09-_.~%2A"
        );
        // form-urlencoded safe set: alphanumerics + `*-._` pass through; `~` is encoded (`%7E`).
        assert_eq!(
            url_encode("AZaz09-_.~*", UrlEncoding::Form, HexCase::Upper),
            "AZaz09-_.%7E*"
        );
        // `@`/`/` → `%40`/`%2F` under both standards.
        assert_eq!(
            url_encode("pa@ss/word", UrlEncoding::Rfc3986, HexCase::Upper),
            "pa%40ss%2Fword"
        );
        assert_eq!(
            url_encode("pa@ss/word", UrlEncoding::Form, HexCase::Upper),
            "pa%40ss%2Fword"
        );
        // The space diverges by standard: `%20` (RFC 3986) vs `+` (form).
        assert_eq!(
            url_encode("a b", UrlEncoding::Rfc3986, HexCase::Upper),
            "a%20b"
        );
        assert_eq!(url_encode("a b", UrlEncoding::Form, HexCase::Upper), "a+b");
        // Uppercase vs lowercase hex on the SAME value — the literal letters keep their case.
        assert_eq!(url_encode("=", UrlEncoding::Rfc3986, HexCase::Upper), "%3D");
        assert_eq!(url_encode("=", UrlEncoding::Rfc3986, HexCase::Lower), "%3d");
        assert_eq!(
            url_encode("Pa@ss", UrlEncoding::Rfc3986, HexCase::Lower),
            "Pa%40ss",
            "safe letters keep their original case; only the %HH hex is lowered"
        );
    }

    // FR11 form-charset bypass: `application/x-www-form-urlencoded` (e.g. WHATWG
    // `URLSearchParams` / Node) leaves `*` literal but encodes `~` → `%7E`, the opposite of
    // RFC 3986 (which leaves `~` literal and encodes `*` → `%2A`). A secret `a*b~c` echoed as
    // `a*b%7Ec` by a form encoder is NOT matched by any RFC 3986 rendering (`a%2Ab~c`), so the
    // form variant must be produced. `value_echoed` now matches it. This closes the gap where a
    // form-encoded echo of a value containing `~`/`*` slipped past the RFC-3986-only charset.
    #[test]
    fn value_echoed_matches_form_encoded_unreserved_charset() {
        let secret = "a*b~c";
        // Form-encoded (Node `URLSearchParams`): `*` stays literal, `~` → `%7E`.
        let error_text = "rejected token=a*b%7Ec is not allowed";
        assert!(
            !error_text.contains(secret),
            "raw value is NOT present verbatim — only the form-encoded form is"
        );
        assert!(
            !error_text.contains("a%2Ab~c"),
            "the message uses the form rendering (`~`→`%7E`, `*` literal), NOT the RFC 3986 one"
        );
        assert!(
            value_echoed(error_text, secret),
            "the form-encoded (`~`→`%7E`) rendering of the value must be detected"
        );
    }

    // The two encoding standards render `*` and `~` to DIFFERENT strings, so both renderings
    // are present as variants and an echo in either is matched.
    #[test]
    fn encoded_variants_includes_both_charset_renderings() {
        let variants = encoded_variants("a*b~c");
        assert!(
            variants.contains(&"a%2Ab~c".to_string()),
            "RFC 3986 rendering (`*`→`%2A`, `~` literal) present"
        );
        assert!(
            variants.contains(&"a*b%7Ec".to_string()),
            "form rendering (`*` literal, `~`→`%7E`) present"
        );
    }

    // FR11 lowercase-hex bypass guard: RFC 3986 treats `%2f` as equivalent to `%2F`, and many
    // encoders/proxies emit lowercase hex. A value echoed with lowercase percent-encoding
    // (`pa%40ss%2fword`) must still be detected even though the canonical RFC 3986 form is
    // uppercase. This closes the gap where only the uppercase rendering was matched.
    #[test]
    fn value_echoed_matches_lowercase_percent_encoded_value() {
        let secret = "pa@ss/word";
        let error_text = "validator failed: token=pa%40ss%2fword is not allowed";
        assert!(
            !error_text.contains(secret),
            "raw value is NOT present verbatim — only the lowercase percent-encoded form is"
        );
        assert!(
            !error_text.contains("pa%40ss%2Fword"),
            "the message uses lowercase `%2f`, NOT the uppercase `%2F` form"
        );
        assert!(
            value_echoed(error_text, secret),
            "the lowercase-hex percent-encoded rendering of the value must be detected"
        );
    }

    // Both hex casings of the encoded value are present as variants so an echo in either is
    // matched; alongside the raw, JSON, and both space forms covered by other tests.
    #[test]
    fn encoded_variants_includes_both_hex_cases() {
        // `/` → `%2F` / `%2f`: the hex digit `F`/`f` differs by case, so both must appear.
        let variants = encoded_variants("a/b");
        assert!(
            variants.contains(&"a%2Fb".to_string()),
            "uppercase-hex form present"
        );
        assert!(
            variants.contains(&"a%2fb".to_string()),
            "lowercase-hex form present"
        );
        // A value whose only encoded bytes are digits (no A-F hex letters) collapses the cases.
        let no_hex_letters = encoded_variants("a b"); // space → %20 / + ; no A-F hex digits
        assert_eq!(
            no_hex_letters
                .iter()
                .filter(|v| v.as_str() == "a%20b")
                .count(),
            1,
            "with no A-F hex digit, the upper/lower percent renderings are identical and de-duped"
        );
    }

    // A value containing a space contributes BOTH URL renderings (the `%20` and the `+` form)
    // in addition to the raw value, so an echo in either is matched. A value with no space
    // contributes the percent form once (the `+` form is identical and de-duplicated).
    #[test]
    fn encoded_variants_includes_both_space_forms() {
        let variants = encoded_variants("a b");
        assert!(variants.contains(&"a b".to_string()), "raw form present");
        assert!(
            variants.contains(&"a%20b".to_string()),
            "percent form present"
        );
        assert!(
            variants.contains(&"a+b".to_string()),
            "form-encoded form present"
        );
        // No space → the two URL renderings collapse to one (no duplicate).
        let no_space = encoded_variants("pa@ss");
        assert_eq!(
            no_space.iter().filter(|v| v.as_str() == "pa%40ss").count(),
            1,
            "the percent form appears exactly once when there is no space to diverge on"
        );
    }

    // A value that needs percent-encoding but no JSON-escaping contributes the raw + the
    // percent-encoded variants (and no JSON variant, since it equals the raw).
    #[test]
    fn encoded_variants_includes_percent_form() {
        let variants = encoded_variants("pa@ss");
        assert!(variants.contains(&"pa@ss".to_string()));
        assert!(variants.contains(&"pa%40ss".to_string()));
        // No JSON-escape variant: the value has no quotes/backslashes/control chars.
        assert_eq!(
            variants
                .iter()
                .filter(|v| v.contains('"') || v.contains('\\'))
                .count(),
            0
        );
    }
}
