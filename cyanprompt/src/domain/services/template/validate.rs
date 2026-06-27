use crate::domain::models::answer::Answer;
use crate::domain::models::prompt::Prompts;
use crate::domain::models::question::Question;
use crate::domain::models::question::QuestionTrait;
use crate::domain::models::template::input::TemplateValidateInput;
use crate::domain::services::repo::CyanRepo;
use crate::domain::services::template::redact;
use chrono::NaiveDate;
use inquire::CustomUserError;
use inquire::validator::{ErrorMessage, Validation};
use std::collections::HashMap;
use std::rc::Rc;

fn validate_template(
    result: &str,
    repo: Rc<dyn CyanRepo>,
    answers: HashMap<String, Answer>,
    deterministic_state: HashMap<String, String>,
) -> Result<Validation, CustomUserError> {
    let input = TemplateValidateInput {
        answers: answers.clone(),
        deterministic_state: deterministic_state.clone(),
        validate: result.to_string(),
    };
    repo.validate_template(input).map(|r| {
        r.map_or(Validation::Valid, |x| {
            Validation::Invalid(ErrorMessage::Custom(x))
        })
    })
}

/// Decide how a supplied [`Answer`] should be validated for a given [`Question`] kind.
///
/// Returns:
/// - `Ok(Some(value))` — a string to push through the coordinator validator (the same
///   value the interactive `inquire` validator would send): `Text`/`Password` raw
///   strings, and a `Date` rendered as `%Y-%m-%d`.
/// - `Ok(None)` — a type-aligned answer for a kind whose only constraint is one the
///   interactive prompt enforces purely structurally and that this function has already
///   checked (`Confirm`, a `Select`/`Checkbox` value that is a member of `options`, a
///   `Date` within `min_date`/`max_date`). The headless driver accepts it
///   unconditionally, mirroring the interactive path.
/// - `Err(id-only message)` — the answer's [`Answer`] discriminant does NOT match the
///   question kind (e.g. a `Text` question answered with `Bool`), a `Date` value does
///   not parse as `%Y-%m-%d` or falls outside `min_date`/`max_date`, or a
///   `Select`/`Checkbox` value is not among the question's `options`. Headless must
///   reject shapes the interactive prompt could never produce, rather than silently
///   accepting them. The message references the question id only — never the offending
///   value.
fn validateable_value(question: &Question, answer: &Answer) -> Result<Option<String>, String> {
    match (question, answer) {
        // Text/Password: a raw string is forwarded to the coordinator validator.
        (Question::Text(_), Answer::String(s)) | (Question::Password(_), Answer::String(s)) => {
            Ok(Some(s.clone()))
        }
        (Question::Date(date), Answer::String(s)) => {
            // The interactive Date validator formats the picked NaiveDate as %Y-%m-%d
            // before sending it to the coordinator. Round-trip the supplied value
            // through the same format so the coordinator sees identical input. A value
            // that does not parse as a date cannot reach the coordinator in this shape
            // interactively (the date picker always yields a valid NaiveDate), so reject
            // it rather than silently accepting a malformed string.
            let d = NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map_err(|_| format!("invalid answer for question '{}'", question.id()))?;
            // The interactive date picker enforces min_date/max_date through the UI — a
            // value outside that range can never be selected. Headless must enforce the
            // same constraint. Only compare against a bound that is itself a
            // well-formed date (a malformed min/max is ignored, matching how the
            // interactive mapper silently drops an unparseable bound). References ids
            // only, never the offending value.
            if let Some(min) = date
                .min_date
                .as_deref()
                .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            {
                if d < min {
                    return Err(format!("invalid answer for question '{}'", question.id()));
                }
            }
            if let Some(max) = date
                .max_date
                .as_deref()
                .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            {
                if d > max {
                    return Err(format!("invalid answer for question '{}'", question.id()));
                }
            }
            Ok(Some(d.format("%Y-%m-%d").to_string()))
        }
        // The interactive Select picker only ever yields one of the question's
        // `options` — a value outside that set can never be selected. Reject it here
        // (id-only message) before accepting.
        (Question::Select(select), Answer::String(s)) => {
            if select.options.iter().any(|opt| opt == s) {
                Ok(None)
            } else {
                Err(format!("invalid answer for question '{}'", question.id()))
            }
        }
        // The interactive Checkbox (MultiSelect) picker only ever yields a subset of
        // the question's `options` — a value outside that set can never be selected.
        // Reject any element that is not a member (id-only message).
        (Question::Checkbox(checkbox), Answer::StringArray(arr)) => {
            if arr
                .iter()
                .all(|s| checkbox.options.iter().any(|opt| opt == s))
            {
                Ok(None)
            } else {
                Err(format!("invalid answer for question '{}'", question.id()))
            }
        }
        // Confirm has no structural constraint beyond the Bool discriminant — accepted
        // unconditionally (mirrors the interactive path's `default => default` arm).
        (Question::Confirm(_), Answer::Bool(_)) => Ok(None),
        // Any other (question, answer) pairing is a type mismatch the interactive prompt
        // could never produce — reject with an id-only message.
        _ => Err(format!("invalid answer for question '{}'", question.id())),
    }
}

/// Validate a supplied answer for a question through the SAME coordinator endpoint
/// the interactive `inquire` validator uses.
///
/// Returns `Ok(())` when the answer is acceptable, or `Err(message)` when it fails
/// validation (the message references the question id, never the offending value)
/// or the coordinator rejects it. The structural constraints the interactive
/// picker enforces in the UI (Select/Checkbox option membership, Date min/max range)
/// are enforced locally before the coordinator is consulted; `Confirm` and
/// type-aligned valid Select/Checkbox/Date values are then accepted, matching the
/// interactive path's `default => default` arm.
pub fn validate_answer(
    question: &Question,
    answer: &Answer,
    repo: Rc<dyn CyanRepo>,
    answers: HashMap<String, Answer>,
    deterministic_state: HashMap<String, String>,
) -> Result<(), String> {
    let value = match validateable_value(question, answer) {
        // A value to push through the coordinator validator.
        Ok(Some(value)) => value,
        // No interactive validator for this kind — accept (mirrors `add_template_validator`).
        Ok(None) => return Ok(()),
        // Type mismatch / malformed date — reject before contacting the coordinator.
        // The message references the question id only.
        Err(msg) => return Err(msg),
    };
    // The coordinator validator receives the full SIBLING answer map (every answer the
    // walk has progressed past), so a cross-field validator's rejection message or a
    // transport error can echo a SIBLING value — including an earlier Password — not just
    // the submitted value for this question. Capture the rendered sibling values BEFORE
    // the map is moved into `validate_template` so the redaction below can drop any
    // message echoing one of them. Without this the offending value for the CURRENT
    // question is redacted but a sibling secret echoed by a cross-field validator reaches
    // the headless error envelope verbatim, leaking a secret the secrecy contract forbids.
    let sibling_values = redact::answer_map_renderings(&answers);
    match validate_template(&value, repo, answers, deterministic_state) {
        Ok(Validation::Valid) => Ok(()),
        Ok(Validation::Invalid(ErrorMessage::Custom(msg))) => {
            // never echo the offending ANSWER value into the error envelope on
            // stdout. For secret-typed questions (Password) the coordinator's custom
            // message is dropped ENTIRELY — a value-echoing coordinator could otherwise
            // leak the secret verbatim. For non-secret kinds, if the coordinator message
            // contains the submitted value OR any sibling value as a substring it is also
            // dropped (the message may be echoing the value), otherwise the coordinator's
            // wording is kept.
            Err(redact_coordinator_message(
                question,
                "invalid answer for question",
                &msg,
                &value,
                &sibling_values,
            ))
        }
        Ok(Validation::Invalid(_)) => {
            Err(format!("invalid answer for question '{}'", question.id()))
        }
        Err(e) => {
            // a coordinator/transport error is surfaced as an `error`
            // envelope. The raw coordinator error text can echo the offending answer
            // value (e.g. "validator failed for value s3cr3t-token"), so it MUST be
            // redacted exactly like the `Validation::Invalid` arm above. For
            // secret-typed questions (Password) the coordinator message is dropped
            // ENTIRELY — a value-echoing transport error would otherwise leak the
            // secret. For non-secret kinds, the message is kept unless it contains the
            // submitted value or any sibling value as a substring. Either way the message
            // references the question id and never the offending value.
            Err(redact_coordinator_message(
                question,
                "validation failed for question",
                &e.to_string(),
                &value,
                &sibling_values,
            ))
        }
    }
}

/// Build a secrecy-safe error message from a coordinator response (a validation
/// rejection reason or a transport-error string).
///
/// The coordinator text can echo the offending answer value, so for a secret-typed
/// question (`Password`) the coordinator message is dropped ENTIRELY; for non-secret
/// kinds the message is kept unless it contains the submitted value OR any sibling value
/// as a substring. Sibling values are covered because the coordinator validator receives
/// the full sibling answer map, so a cross-field validator's message (or a transport
/// error) can echo a SIBLING value — including an earlier Password held in the sibling
/// map — not just the value for this question. The returned message is
/// `"{prefix} '{question_id}'"` (message dropped) or
/// `"{prefix} '{question_id}': {coordinator_message}"` (message kept) — referencing the
/// question id and never the offending value. This shared helper keeps the
/// `Validation::Invalid` and transport-`Err` arms in [`validate_answer`] identical so the
/// redaction rule cannot drift between the two coordinator-response surfaces.
fn redact_coordinator_message(
    question: &Question,
    prefix: &str,
    coordinator_message: &str,
    submitted_value: &str,
    sibling_values: &[String],
) -> String {
    let keep = !matches!(question, Question::Password(_))
        && !redact::value_echoed(coordinator_message, submitted_value)
        && !sibling_values
            .iter()
            .any(|s| redact::value_echoed(coordinator_message, s));
    if keep {
        format!("{} '{}': {}", prefix, question.id(), coordinator_message)
    } else {
        format!("{} '{}'", prefix, question.id())
    }
}

pub fn add_template_validator(
    p: Prompts,
    repo: Rc<dyn CyanRepo>,
    answers: HashMap<String, Answer>,
    deterministic_state: HashMap<String, String>,
) -> Prompts {
    match p {
        Prompts::Text(text) => Prompts::Text(text.with_validator(move |v: &str| {
            validate_template(
                v,
                Rc::clone(&repo),
                answers.clone(),
                deterministic_state.clone(),
            )
        })),
        Prompts::Password(pw) => Prompts::Password(pw.with_validator(move |v: &str| {
            validate_template(
                v,
                Rc::clone(&repo),
                answers.clone(),
                deterministic_state.clone(),
            )
        })),
        Prompts::Date(d) => Prompts::Date(d.with_validator(move |v: NaiveDate| {
            validate_template(
                v.format("%Y-%m-%d").to_string().as_str(),
                Rc::clone(&repo),
                answers.clone(),
                deterministic_state.clone(),
            )
        })),
        default => default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::models::question::{
        CheckboxQuestion, ConfirmQuestion, DateQuestion, PasswordQuestion, SelectQuestion,
        TextQuestion,
    };
    use crate::domain::models::template::output::TemplateOutput;
    use std::error::Error;

    /// A `CyanRepo` whose `validate_template` is driven by a closure, used to exercise
    /// the coordinator-side branches of [`validate_answer`] (FR9). `prompt_template` is
    /// unused by these direct unit tests.
    struct ValidatorRepo {
        #[allow(clippy::type_complexity)]
        validate:
            Box<dyn Fn(&str) -> Result<Option<String>, Box<dyn Error + 'static + Send + Sync>>>,
    }

    impl CyanRepo for ValidatorRepo {
        fn prompt_template(
            &self,
            _: crate::domain::models::template::input::TemplateAnswerInput,
        ) -> Result<TemplateOutput, Box<dyn Error + Send>> {
            unreachable!("prompt_template is not exercised by validate_answer unit tests")
        }
        fn validate_template(
            &self,
            input: TemplateValidateInput,
        ) -> Result<Option<String>, Box<dyn Error + 'static + Send + Sync>> {
            (self.validate)(&input.validate)
        }
    }

    /// A repo whose coordinator validator always returns `Err` — exercising the
    /// transport-error arm of [`validate_answer`] (FR11 must still redact secrets there).
    struct TransportErrorRepo;
    impl CyanRepo for TransportErrorRepo {
        fn prompt_template(
            &self,
            _: crate::domain::models::template::input::TemplateAnswerInput,
        ) -> Result<TemplateOutput, Box<dyn Error + Send>> {
            unreachable!()
        }
        fn validate_template(
            &self,
            _: TemplateValidateInput,
        ) -> Result<Option<String>, Box<dyn Error + 'static + Send + Sync>> {
            Err(Box::new(std::io::Error::other(
                "coordinator validator unreachable",
            )))
        }
    }

    fn select_q(id: &str, opts: &[&str]) -> Question {
        Question::Select(SelectQuestion {
            message: "m".into(),
            desc: None,
            options: opts.iter().map(|s| s.to_string()).collect(),
            id: id.into(),
        })
    }

    fn checkbox_q(id: &str, opts: &[&str]) -> Question {
        Question::Checkbox(CheckboxQuestion {
            message: "m".into(),
            options: opts.iter().map(|s| s.to_string()).collect(),
            desc: None,
            id: id.into(),
        })
    }

    fn date_q(id: &str, min: Option<&str>, max: Option<&str>) -> Question {
        Question::Date(DateQuestion {
            message: "m".into(),
            desc: None,
            default: None,
            min_date: min.map(|s| s.to_string()),
            max_date: max.map(|s| s.to_string()),
            id: id.into(),
        })
    }

    fn text_q(id: &str) -> Question {
        Question::Text(TextQuestion {
            message: "m".into(),
            default: None,
            desc: None,
            initial: None,
            id: id.into(),
        })
    }

    fn confirm_q(id: &str) -> Question {
        Question::Confirm(ConfirmQuestion {
            message: "m".into(),
            desc: None,
            default: None,
            error_message: None,
            id: id.into(),
        })
    }

    fn password_q(id: &str) -> Question {
        Question::Password(PasswordQuestion {
            message: "m".into(),
            desc: None,
            confirmation: None,
            id: id.into(),
        })
    }

    /// A Select with an empty options list accepts NO string value — the structural
    /// option-membership guard rejects anything when options is empty.
    #[test]
    fn select_with_empty_options_rejects_any_value() {
        let res = validateable_value(&select_q("sel", &[]), &Answer::String("anything".into()));
        assert!(
            res.is_err(),
            "an empty-options Select must reject any value"
        );
    }

    /// A Select answered with the empty string is rejected when "" is not one of the
    /// options (the default case): structural membership fails.
    #[test]
    fn select_rejects_empty_string_value_when_not_an_option() {
        let res = validateable_value(
            &select_q("sel", &["dev", "prod"]),
            &Answer::String("".into()),
        );
        assert!(
            res.is_err(),
            "empty string must be rejected when not an option"
        );
    }

    /// A Select answered with the empty string IS accepted when "" is a listed option
    /// (membership holds) — but here it forwards the value to the coordinator (Some).
    #[test]
    fn select_accepts_empty_string_value_when_an_option() {
        let res = validateable_value(&select_q("sel", &["", "dev"]), &Answer::String("".into()));
        assert!(
            matches!(res, Ok(None)),
            "an empty-string option member is accepted as type-aligned: {res:?}"
        );
    }

    /// A Checkbox answered with an empty array is a valid (zero-selection) subset of
    /// any options — type-aligned, accepted.
    #[test]
    fn checkbox_empty_array_is_accepted() {
        let res = validateable_value(
            &checkbox_q("cb", &["auth", "billing"]),
            &Answer::StringArray(vec![]),
        );
        assert!(res.is_ok(), "an empty Checkbox selection is a valid subset");
    }

    /// A Checkbox answered with a `String` (not `StringArray`) is a type mismatch the
    /// interactive MultiSelect could never produce — rejected.
    #[test]
    fn checkbox_rejects_string_instead_of_array() {
        let res = validateable_value(&checkbox_q("cb", &["auth"]), &Answer::String("auth".into()));
        assert!(
            res.is_err(),
            "a Checkbox must reject a String answer (type mismatch)"
        );
    }

    /// A Select answered with a `StringArray` (not `String`) is a type mismatch the
    /// interactive Select picker could never produce — rejected.
    #[test]
    fn select_rejects_array_instead_of_string() {
        let res = validateable_value(
            &select_q("sel", &["dev"]),
            &Answer::StringArray(vec!["dev".into()]),
        );
        assert!(
            res.is_err(),
            "a Select must reject a StringArray answer (type mismatch)"
        );
    }

    /// A Date with only a min bound rejects a value below it.
    #[test]
    fn date_rejects_value_below_min() {
        let res = validateable_value(
            &date_q("d", Some("2026-01-01"), None),
            &Answer::String("2025-12-31".into()),
        );
        assert!(res.is_err(), "a Date below min_date must be rejected");
    }

    /// A malformed min bound is ignored (mirrors the interactive mapper silently dropping
    /// an unparseable bound); a well-formed max bound still constrains the value.
    #[test]
    fn date_ignores_malformed_min_but_enforces_max() {
        let res = validateable_value(
            &date_q("d", Some("not-a-date"), Some("2026-12-31")),
            &Answer::String("1900-01-01".into()),
        );
        // Malformed min is ignored; the value is well below the valid max, so accepted.
        assert!(res.is_ok(), "a malformed min bound is ignored: {res:?}");
    }

    /// When both bounds are malformed, neither constrains the value — any well-formed
    /// date is accepted.
    #[test]
    fn date_with_both_bounds_malformed_accepts_any_well_formed_date() {
        let res = validateable_value(
            &date_q("d", Some("bad"), Some("also-bad")),
            &Answer::String("1900-01-01".into()),
        );
        assert!(
            res.is_ok(),
            "both bounds malformed => no constraint applied"
        );
    }

    /// A Date value exactly at the min boundary is accepted (the range check is inclusive).
    #[test]
    fn date_accepts_value_at_exact_min_boundary() {
        let res = validateable_value(
            &date_q("d", Some("2026-01-01"), Some("2026-12-31")),
            &Answer::String("2026-01-01".into()),
        );
        assert!(
            res.is_ok(),
            "a Date at the exact min boundary must be accepted"
        );
    }

    /// A Confirm answered with a `String` (not `Bool`) is a type mismatch — rejected.
    #[test]
    fn confirm_rejects_string_instead_of_bool() {
        let res = validateable_value(&confirm_q("c"), &Answer::String("true".into()));
        assert!(
            res.is_err(),
            "a Confirm must reject a String answer (type mismatch)"
        );
    }

    /// A Text answered with an empty string forwards it to the coordinator validator
    /// (the structural guard passes for any string).
    #[test]
    fn text_forwards_empty_string_to_coordinator() {
        let res = validateable_value(&text_q("t"), &Answer::String("".into()));
        assert!(
            matches!(res, Ok(Some(ref v)) if v.is_empty()),
            "a Text empty string must forward to the coordinator: {res:?}"
        );
    }

    /// FR11: when the coordinator validator returns a TRANSPORT error for a
    /// password question, the surfaced message must reference the question id ONLY —
    /// never the offending secret value, even if the transport error echoes it. The
    /// `Err(e)` arm applies the same secret-aware redaction as the `Validation::Invalid`
    /// arm.
    #[test]
    fn password_transport_error_never_echoes_secret() {
        // A transport error whose message echoes the secret value verbatim.
        struct EchoingTransportRepo;
        impl CyanRepo for EchoingTransportRepo {
            fn prompt_template(
                &self,
                _: crate::domain::models::template::input::TemplateAnswerInput,
            ) -> Result<TemplateOutput, Box<dyn Error + Send>> {
                unreachable!()
            }
            fn validate_template(
                &self,
                _: TemplateValidateInput,
            ) -> Result<Option<String>, Box<dyn Error + 'static + Send + Sync>> {
                Err(Box::new(std::io::Error::other(
                    "validator failed for value s3cr3t-token",
                )))
            }
        }

        let res = validate_answer(
            &password_q("token"),
            &Answer::String("s3cr3t-token".into()),
            Rc::new(EchoingTransportRepo),
            HashMap::new(),
            HashMap::new(),
        );
        match res {
            Err(msg) => {
                assert!(
                    msg.contains("token"),
                    "transport error must reference the question id: {msg}"
                );
                assert!(
                    !msg.contains("s3cr3t-token"),
                    "transport error must never echo the secret value (FR11): {msg}"
                );
            }
            Ok(()) => panic!("a transport error must surface as Err, not Ok"),
        }
    }

    /// FR11 (non-secret control): a transport error for a non-secret (Text) question
    /// surfaces the coordinator error text (no redaction needed — the value is not a
    /// secret), and never panics.
    #[test]
    fn non_secret_transport_error_surfaces_message() {
        let res = validate_answer(
            &text_q("name"),
            &Answer::String("any".into()),
            Rc::new(TransportErrorRepo),
            HashMap::new(),
            HashMap::new(),
        );
        match res {
            Err(msg) => {
                assert!(msg.contains("name"), "error must reference the question id");
                assert!(
                    msg.contains("coordinator validator unreachable"),
                    "non-secret transport error keeps the message: {msg}"
                );
            }
            Ok(()) => panic!("a transport error must surface as Err, not Ok"),
        }
    }

    /// FR11 (AC6): when the coordinator REJECTS a password answer with a custom message
    /// that echoes the secret value, the surfaced error must reference the question id
    /// ONLY — the coordinator message is dropped entirely for secret-typed questions, so
    /// the secret never reaches the `error` envelope on stdout.
    #[test]
    fn password_coordinator_invalid_never_echoes_secret() {
        let repo = ValidatorRepo {
            // The coordinator's custom rejection echoes the submitted value verbatim.
            validate: Box::new(|v: &str| Ok(Some(format!("value '{v}' is not allowed")))),
        };
        let res = validate_answer(
            &password_q("token"),
            &Answer::String("s3cr3t-token".into()),
            Rc::new(repo),
            HashMap::new(),
            HashMap::new(),
        );
        match res {
            Err(msg) => {
                assert!(
                    msg.contains("token"),
                    "the error must reference the question id: {msg}"
                );
                assert!(
                    !msg.contains("s3cr3t-token"),
                    "a password rejection must never echo the secret value (FR11): {msg}"
                );
            }
            Ok(()) => panic!("a rejected answer must surface as Err, not Ok"),
        }
    }

    /// FR11 (AC6, non-secret control): a coordinator rejection for a NON-secret (Text)
    /// question whose message does NOT contain the submitted value keeps the
    /// coordinator's wording (it carries useful, non-leaking guidance).
    #[test]
    fn text_coordinator_invalid_keeps_non_echoing_message() {
        let repo = ValidatorRepo {
            // A useful message that does not echo the submitted value.
            validate: Box::new(|_v: &str| Ok(Some("must be lowercase".to_string()))),
        };
        let res = validate_answer(
            &text_q("name"),
            &Answer::String("UPPER".into()),
            Rc::new(repo),
            HashMap::new(),
            HashMap::new(),
        );
        match res {
            Err(msg) => {
                assert!(msg.contains("name"), "error must reference the question id");
                assert!(
                    msg.contains("must be lowercase"),
                    "a non-echoing coordinator message is kept: {msg}"
                );
            }
            Ok(()) => panic!("a rejected answer must surface as Err, not Ok"),
        }
    }

    /// FR11 (sibling secrecy): a cross-field coordinator validator receives the FULL
    /// sibling answer map, so its rejection message can echo a SIBLING value — including
    /// an earlier Password held in the sibling map — even though the value being
    /// validated is for a non-secret (Text) question. The surfaced error must reference
    /// the question id ONLY and must never echo the sibling secret. This is the bypass a
    /// single-value redactor misses: `redact_coordinator_message` is given the sibling
    /// values so a message echoing any of them is dropped to an id-only message.
    #[test]
    fn sibling_secret_in_coordinator_message_is_redacted() {
        let repo = ValidatorRepo {
            // A cross-field validator echoes an earlier Password sibling value verbatim
            // in its rejection of a later, non-secret answer.
            validate: Box::new(|_v: &str| {
                Ok(Some(
                    "name must not contain the token s3cr3t-token".to_string(),
                ))
            }),
        };
        // The walk has already recorded the Password answer for `token` as a sibling; the
        // coordinator validator for the `name` (Text) question echoes it.
        let mut siblings = HashMap::new();
        siblings.insert("token".to_string(), Answer::String("s3cr3t-token".into()));
        let res = validate_answer(
            &text_q("name"),
            &Answer::String("myapp".into()),
            Rc::new(repo),
            siblings,
            HashMap::new(),
        );
        match res {
            Err(msg) => {
                assert!(
                    msg.contains("name"),
                    "the error must reference the question id: {msg}"
                );
                assert!(
                    !msg.contains("s3cr3t-token"),
                    "a cross-field validator must never echo a sibling secret (FR11): {msg}"
                );
            }
            Ok(()) => panic!("a rejected answer must surface as Err, not Ok"),
        }
    }

    /// FR11 (sibling secrecy, transport error): the same sibling-secret leak can surface
    /// through a transport error whose message echoes a sibling Password value. The
    /// transport-`Err` arm applies the same sibling-aware redaction as the
    /// `Validation::Invalid` arm.
    #[test]
    fn sibling_secret_in_transport_error_is_redacted() {
        struct EchoingTransportRepo;
        impl CyanRepo for EchoingTransportRepo {
            fn prompt_template(
                &self,
                _: crate::domain::models::template::input::TemplateAnswerInput,
            ) -> Result<TemplateOutput, Box<dyn Error + Send>> {
                unreachable!()
            }
            fn validate_template(
                &self,
                _: TemplateValidateInput,
            ) -> Result<Option<String>, Box<dyn Error + 'static + Send + Sync>> {
                Err(Box::new(std::io::Error::other(
                    "name must not contain the token s3cr3t-token",
                )))
            }
        }
        let mut siblings = HashMap::new();
        siblings.insert("token".to_string(), Answer::String("s3cr3t-token".into()));
        let res = validate_answer(
            &text_q("name"),
            &Answer::String("myapp".into()),
            Rc::new(EchoingTransportRepo),
            siblings,
            HashMap::new(),
        );
        match res {
            Err(msg) => {
                assert!(
                    msg.contains("name"),
                    "transport error must reference the question id: {msg}"
                );
                assert!(
                    !msg.contains("s3cr3t-token"),
                    "a transport error must never echo a sibling secret (FR11): {msg}"
                );
            }
            Ok(()) => panic!("a transport error must surface as Err, not Ok"),
        }
    }

    // FR11 escaped-echo bypass: a cross-field validator that embeds the sibling secret as
    // JSON (so a quote in the value renders escaped, `pa\"ss`) must still be redacted. The
    // raw secret (`pa"ss`) is NOT a substring of the escaped message, so a naive
    // `contains` check would let it through; the encoding-aware `value_echoed` catches it.
    #[test]
    fn sibling_secret_escaped_in_coordinator_message_is_redacted() {
        let repo = ValidatorRepo {
            // The validator echoes the sibling Password value the way it appears inside a
            // serialized JSON body — the quote is backslash-escaped.
            validate: Box::new(|_v: &str| Ok(Some(r#"rejected: {"token":"pa\"ss"}"#.to_string()))),
        };
        let mut siblings = HashMap::new();
        siblings.insert("token".to_string(), Answer::String("pa\"ss".into()));
        let res = validate_answer(
            &text_q("name"),
            &Answer::String("myapp".into()),
            Rc::new(repo),
            siblings,
            HashMap::new(),
        );
        match res {
            Err(msg) => {
                assert!(
                    !msg.contains(r#"pa\"ss"#),
                    "the JSON-escaped sibling secret must be redacted (FR11): {msg}"
                );
                assert!(
                    !msg.contains("pa\"ss"),
                    "the raw sibling secret must not appear either: {msg}"
                );
                assert!(
                    msg.contains("name"),
                    "the error must still reference the question id: {msg}"
                );
            }
            Ok(()) => panic!("a rejected answer must surface as Err, not Ok"),
        }
    }

    // FR11 percent-encoded-echo bypass: a cross-field validator (or a proxy that re-encodes
    // the request body) may embed the sibling secret PERCENT-ENCODED (`pa@ss/word` →
    // `pa%40ss%2Fword`) when the value travelled in a URL path, query string, or form
    // payload. The raw secret is NOT a substring of the encoded message, so a naive
    // `contains` check would let it through; the encoding-aware `value_echoed` (which now
    // matches the percent-encoded variant) catches it. This is the validate-sibling surface
    // of the same fix; the in-flight transport surface inherits it via the shared renderer.
    #[test]
    fn sibling_secret_percent_encoded_in_coordinator_message_is_redacted() {
        let repo = ValidatorRepo {
            // The validator echoes the sibling Password value the way it appears in a
            // URL/form-encoded payload — `@`/`/` become `%40`/`%2F`.
            validate: Box::new(|_v: &str| Ok(Some("rejected: token=pa%40ss%2Fword".to_string()))),
        };
        let mut siblings = HashMap::new();
        siblings.insert("token".to_string(), Answer::String("pa@ss/word".into()));
        let res = validate_answer(
            &text_q("name"),
            &Answer::String("myapp".into()),
            Rc::new(repo),
            siblings,
            HashMap::new(),
        );
        match res {
            Err(msg) => {
                assert!(
                    !msg.contains("pa%40ss%2Fword"),
                    "the percent-encoded sibling secret must be redacted (FR11): {msg}"
                );
                assert!(
                    !msg.contains("pa@ss/word"),
                    "the raw sibling secret must not appear either: {msg}"
                );
                assert!(
                    msg.contains("name"),
                    "the error must still reference the question id: {msg}"
                );
            }
            Ok(()) => panic!("a rejected answer must surface as Err, not Ok"),
        }
    }

    // FR11 percent-encoded-echo bypass on the transport surface: a transport error whose
    // message echoes a sibling secret in percent-encoded form must be redacted too.
    #[test]
    fn sibling_secret_percent_encoded_in_transport_error_is_redacted() {
        struct EchoingTransportRepo;
        impl CyanRepo for EchoingTransportRepo {
            fn prompt_template(
                &self,
                _: crate::domain::models::template::input::TemplateAnswerInput,
            ) -> Result<TemplateOutput, Box<dyn Error + Send>> {
                unreachable!()
            }
            fn validate_template(
                &self,
                _: TemplateValidateInput,
            ) -> Result<Option<String>, Box<dyn Error + 'static + Send + Sync>> {
                Err(Box::new(std::io::Error::other(
                    "name must not match token=pa%40ss%2Fword",
                )))
            }
        }
        let mut siblings = HashMap::new();
        siblings.insert("token".to_string(), Answer::String("pa@ss/word".into()));
        let res = validate_answer(
            &text_q("name"),
            &Answer::String("myapp".into()),
            Rc::new(EchoingTransportRepo),
            siblings,
            HashMap::new(),
        );
        match res {
            Err(msg) => {
                assert!(
                    msg.contains("name"),
                    "transport error must reference the question id: {msg}"
                );
                assert!(
                    !msg.contains("pa%40ss%2Fword"),
                    "a transport error must never echo a percent-encoded sibling secret (FR11): {msg}"
                );
                assert!(
                    !msg.contains("pa@ss/word"),
                    "a transport error must never echo the raw sibling secret (FR11): {msg}"
                );
            }
            Ok(()) => panic!("a transport error must surface as Err, not Ok"),
        }
    }

    // FR11 form-encoded-echo bypass (validate-sibling surface): a cross-field validator, or a
    // proxy that re-encodes the request body to `application/x-www-form-urlencoded`, may echo
    // the sibling secret with a space rendered as `+` (NOT `%20`): `pa ss/word` → `pa+ss%2Fword`.
    // The `%20`-only encoding would miss this; the `+` variant catches it.
    #[test]
    fn sibling_secret_form_encoded_in_coordinator_message_is_redacted() {
        let repo = ValidatorRepo {
            // Form-encoded echo of the sibling Password: space → `+`, `/` → `%2F`.
            validate: Box::new(|_v: &str| Ok(Some("rejected token=pa+ss%2Fword".to_string()))),
        };
        let mut siblings = HashMap::new();
        siblings.insert("token".to_string(), Answer::String("pa ss/word".into()));
        let res = validate_answer(
            &text_q("name"),
            &Answer::String("myapp".into()),
            Rc::new(repo),
            siblings,
            HashMap::new(),
        );
        match res {
            Err(msg) => {
                assert!(
                    !msg.contains("pa+ss%2Fword"),
                    "the form-encoded sibling secret must be redacted (FR11): {msg}"
                );
                assert!(
                    !msg.contains("pa ss/word"),
                    "the raw sibling secret must not appear either: {msg}"
                );
                assert!(
                    msg.contains("name"),
                    "the error must still reference the question id: {msg}"
                );
            }
            Ok(()) => panic!("a rejected answer must surface as Err, not Ok"),
        }
    }

    // FR11 form-encoded-echo bypass on the transport surface: a transport error echoing the
    // sibling secret in form-encoded (`+`-for-space) form must be redacted too.
    #[test]
    fn sibling_secret_form_encoded_in_transport_error_is_redacted() {
        struct EchoingTransportRepo;
        impl CyanRepo for EchoingTransportRepo {
            fn prompt_template(
                &self,
                _: crate::domain::models::template::input::TemplateAnswerInput,
            ) -> Result<TemplateOutput, Box<dyn Error + Send>> {
                unreachable!()
            }
            fn validate_template(
                &self,
                _: TemplateValidateInput,
            ) -> Result<Option<String>, Box<dyn Error + 'static + Send + Sync>> {
                Err(Box::new(std::io::Error::other(
                    "name must not match token=pa+ss%2Fword",
                )))
            }
        }
        let mut siblings = HashMap::new();
        siblings.insert("token".to_string(), Answer::String("pa ss/word".into()));
        let res = validate_answer(
            &text_q("name"),
            &Answer::String("myapp".into()),
            Rc::new(EchoingTransportRepo),
            siblings,
            HashMap::new(),
        );
        match res {
            Err(msg) => {
                assert!(
                    msg.contains("name"),
                    "transport error must reference the question id: {msg}"
                );
                assert!(
                    !msg.contains("pa+ss%2Fword"),
                    "a transport error must never echo a form-encoded sibling secret (FR11): {msg}"
                );
                assert!(
                    !msg.contains("pa ss/word"),
                    "a transport error must never echo the raw sibling secret (FR11): {msg}"
                );
            }
            Ok(()) => panic!("a transport error must surface as Err, not Ok"),
        }
    }

    // FR11 form-charset-echo bypass (validate-sibling surface): `application/x-www-form-urlencoded`
    // (e.g. WHATWG `URLSearchParams` / Node) leaves `*` literal but encodes `~` → `%7E`, the
    // OPPOSITE of RFC 3986 (which leaves `~` literal and encodes `*` → `%2A`). A cross-field
    // validator (or a proxy that re-encodes the request body to form format) echoing the sibling
    // secret `a*b~c` as `a*b%7Ec` would slip past the RFC-3986-only charset; the form variant now
    // catches it. This proves the form-charset fix lands on the validate-sibling surface.
    #[test]
    fn sibling_secret_form_charset_in_coordinator_message_is_redacted() {
        let repo = ValidatorRepo {
            // Form-encoded echo of the sibling secret: `*` stays literal, `~` → `%7E`.
            validate: Box::new(|_v: &str| Ok(Some("rejected token=a*b%7Ec".to_string()))),
        };
        let mut siblings = HashMap::new();
        siblings.insert("token".to_string(), Answer::String("a*b~c".into()));
        let res = validate_answer(
            &text_q("name"),
            &Answer::String("myapp".into()),
            Rc::new(repo),
            siblings,
            HashMap::new(),
        );
        match res {
            Err(msg) => {
                assert!(
                    !msg.contains("a*b%7Ec"),
                    "the form-charset sibling secret must be redacted (FR11): {msg}"
                );
                assert!(
                    !msg.contains("a*b~c"),
                    "the raw sibling secret must not appear either: {msg}"
                );
                assert!(
                    msg.contains("name"),
                    "the error must still reference the question id: {msg}"
                );
            }
            Ok(()) => panic!("a rejected answer must surface as Err, not Ok"),
        }
    }

    // FR11 lowercase-hex-echo bypass (validate-sibling surface): RFC 3986 treats `%2f` as
    // equivalent to `%2F`, and many encoders/proxies emit lowercase hex. A validator/proxy that
    // echoes the sibling secret with lowercase percent-encoding (`pa%40ss%2fword`) must still be
    // redacted even though the canonical RFC 3986 rendering is uppercase.
    #[test]
    fn sibling_secret_lowercase_percent_encoded_in_coordinator_message_is_redacted() {
        let repo = ValidatorRepo {
            // Lowercase-hex percent-encoded echo of the sibling Password: `@`→`%40`, `/`→`%2f`.
            validate: Box::new(|_v: &str| Ok(Some("rejected token=pa%40ss%2fword".to_string()))),
        };
        let mut siblings = HashMap::new();
        siblings.insert("token".to_string(), Answer::String("pa@ss/word".into()));
        let res = validate_answer(
            &text_q("name"),
            &Answer::String("myapp".into()),
            Rc::new(repo),
            siblings,
            HashMap::new(),
        );
        match res {
            Err(msg) => {
                assert!(
                    !msg.contains("pa%40ss%2fword"),
                    "the lowercase-hex sibling secret must be redacted (FR11): {msg}"
                );
                assert!(
                    !msg.contains("pa@ss/word"),
                    "the raw sibling secret must not appear either: {msg}"
                );
                assert!(
                    msg.contains("name"),
                    "the error must still reference the question id: {msg}"
                );
            }
            Ok(()) => panic!("a rejected answer must surface as Err, not Ok"),
        }
    }

    // Shared-renderer boolean policy on the validate surface: a non-echoing coordinator
    // message is KEPT even when a sibling boolean (Confirm) is present — booleans are not
    // tracked, so they no longer blank otherwise-useful validation messages — while a
    // sibling String secret is STILL redacted. This proves the single shared renderer's
    // boolean policy holds on the validate surface without weakening secret redaction.
    #[test]
    fn boolean_sibling_does_not_blank_message_but_secret_sibling_still_redacts() {
        // Case A: only a boolean sibling — a useful, non-echoing message survives.
        let repo = ValidatorRepo {
            validate: Box::new(|_v: &str| Ok(Some("name is too short".to_string()))),
        };
        let mut siblings = HashMap::new();
        siblings.insert("use_db".to_string(), Answer::Bool(true));
        let res = validate_answer(
            &text_q("name"),
            &Answer::String("ab".into()),
            Rc::new(repo),
            siblings,
            HashMap::new(),
        );
        assert!(
            matches!(&res, Err(msg) if msg.contains("name is too short")),
            "a non-echoing message must be kept when only a boolean sibling exists: {res:?}"
        );

        // Case B: a boolean sibling alongside a String secret — the message echoing the
        // secret is still dropped.
        let repo = ValidatorRepo {
            validate: Box::new(|_v: &str| {
                Ok(Some("name must not contain s3cr3t-token".to_string()))
            }),
        };
        let mut siblings = HashMap::new();
        siblings.insert("use_db".to_string(), Answer::Bool(true));
        siblings.insert("token".to_string(), Answer::String("s3cr3t-token".into()));
        let res = validate_answer(
            &text_q("name"),
            &Answer::String("myapp".into()),
            Rc::new(repo),
            siblings,
            HashMap::new(),
        );
        assert!(
            matches!(&res, Err(msg) if !msg.contains("s3cr3t-token")),
            "a sibling secret must still be redacted even with a boolean sibling present: {res:?}"
        );
    }
}
