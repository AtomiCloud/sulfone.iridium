//! Headless Q&A wire contract.
//!
//! Defines the machine-parseable JSON envelope emitted by the non-interactive
//! (`--headless`) driver and the wire representation of a [`Question`]. The
//! envelope is the ONLY thing a headless invocation prints on stdout, and is
//! distinguished by both a `status` tag and an exit code:
//! - `need_input` (exit 2): carries the next unanswered question.
//! - `done` (exit 0): the walk completed.
//! - `error` (exit 1): a human-readable message (never echoes answer values).
//!
//! Secret redaction: the [`QuestionWire::Password`] variant deliberately carries no
//! `default`/value field, so a secret default can never be serialized.

use serde::Serialize;

use crate::domain::models::question::Question;

/// Wire representation of a [`Question`], serialized with a `type` tag matching
/// the HTTP `QuestionRes` kinds (confirm/date/checkbox/password/text/select) and
/// stable snake_case field names. Mapped from the domain [`Question`] rather than
/// derived on it, to keep the domain type decoupled from the wire shape.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum QuestionWire {
    Confirm {
        id: String,
        message: String,
        desc: Option<String>,
        default: Option<bool>,
    },
    Date {
        id: String,
        message: String,
        desc: Option<String>,
        default: Option<String>,
        min_date: Option<String>,
        max_date: Option<String>,
    },
    Checkbox {
        id: String,
        message: String,
        desc: Option<String>,
        options: Vec<String>,
    },
    /// Secret-typed question. Intentionally carries NO default/value field so a
    /// secret default is never emitted in plaintext.
    Password {
        id: String,
        message: String,
        desc: Option<String>,
        confirmation: Option<bool>,
    },
    Text {
        id: String,
        message: String,
        desc: Option<String>,
        default: Option<String>,
        initial: Option<String>,
    },
    Select {
        id: String,
        message: String,
        desc: Option<String>,
        options: Vec<String>,
    },
}

impl From<&Question> for QuestionWire {
    fn from(q: &Question) -> Self {
        match q {
            Question::Confirm(c) => QuestionWire::Confirm {
                id: c.id.clone(),
                message: c.message.clone(),
                desc: c.desc.clone(),
                default: c.default,
            },
            Question::Date(d) => QuestionWire::Date {
                id: d.id.clone(),
                message: d.message.clone(),
                desc: d.desc.clone(),
                default: d.default.clone(),
                min_date: d.min_date.clone(),
                max_date: d.max_date.clone(),
            },
            Question::Checkbox(cb) => QuestionWire::Checkbox {
                id: cb.id.clone(),
                message: cb.message.clone(),
                desc: cb.desc.clone(),
                options: cb.options.clone(),
            },
            // Map only non-secret metadata; never the default/value.
            Question::Password(pw) => QuestionWire::Password {
                id: pw.id.clone(),
                message: pw.message.clone(),
                desc: pw.desc.clone(),
                confirmation: pw.confirmation,
            },
            Question::Text(t) => QuestionWire::Text {
                id: t.id.clone(),
                message: t.message.clone(),
                desc: t.desc.clone(),
                default: t.default.clone(),
                initial: t.initial.clone(),
            },
            Question::Select(s) => QuestionWire::Select {
                id: s.id.clone(),
                message: s.message.clone(),
                desc: s.desc.clone(),
                options: s.options.clone(),
            },
        }
    }
}

/// The headless output envelope. Serialized as a single JSON object tagged by
/// `status`, the sole stdout output of a headless invocation.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum HeadlessEnvelope {
    /// The walk needs an answer for `question` before it can proceed (exit 2).
    NeedInput { question: QuestionWire },
    /// The walk completed with all answers supplied (exit 0). Minimal summary
    /// only — never echoes accumulated answers.
    Done,
    /// The walk failed; `message` is human-readable and references question ids
    /// only, never answer values (exit 1).
    Error { message: String },
}

impl HeadlessEnvelope {
    /// Construct an `error` envelope from any message.
    pub fn error(message: impl Into<String>) -> Self {
        HeadlessEnvelope::Error {
            message: message.into(),
        }
    }

    /// Exit code for this envelope: need_input → 2, done → 0, error → 1.
    pub fn exit_code(&self) -> u8 {
        match self {
            HeadlessEnvelope::NeedInput { .. } => 2,
            HeadlessEnvelope::Done => 0,
            HeadlessEnvelope::Error { .. } => 1,
        }
    }

    /// Serialize to a single-line JSON object.
    ///
    /// This is a closed enum whose fields are all owned `String`s and a [`QuestionWire`]
    /// (itself only `String`/`Option`/`Vec<String>`/`bool`) — serde_json serialization of
    /// such a type cannot fail. A failure here would be a serde_json bug, not a reachable
    /// state, so the invariant is asserted with `expect` rather than masked behind a
    /// synthetic fallback envelope (which would hide a real serialization regression).
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("HeadlessEnvelope serialization is infallible")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::models::question::{
        ConfirmQuestion, PasswordQuestion, SelectQuestion, TextQuestion,
    };

    #[test]
    fn need_input_envelope_serializes_with_status_and_question_fields() {
        let q = Question::Text(TextQuestion {
            message: "Project name?".to_string(),
            default: Some("demo".to_string()),
            desc: Some("the name".to_string()),
            initial: None,
            id: "name".to_string(),
        });
        let env = HeadlessEnvelope::NeedInput {
            question: QuestionWire::from(&q),
        };
        let v: serde_json::Value = serde_json::from_str(&env.to_json()).unwrap();
        assert_eq!(v["status"], "need_input");
        assert_eq!(v["question"]["type"], "text");
        assert_eq!(v["question"]["id"], "name");
        assert_eq!(v["question"]["message"], "Project name?");
        assert_eq!(v["question"]["default"], "demo");
        assert_eq!(env.exit_code(), 2);
    }

    #[test]
    fn select_envelope_carries_options() {
        let q = Question::Select(SelectQuestion {
            message: "Pick one".to_string(),
            desc: None,
            options: vec!["a".to_string(), "b".to_string()],
            id: "choice".to_string(),
        });
        let wire = QuestionWire::from(&q);
        let v = serde_json::to_value(&wire).unwrap();
        assert_eq!(v["type"], "select");
        assert_eq!(v["options"][0], "a");
        assert_eq!(v["options"][1], "b");
    }

    #[test]
    fn confirm_envelope_carries_bool_default() {
        let q = Question::Confirm(ConfirmQuestion {
            message: "Sure?".to_string(),
            desc: None,
            default: Some(true),
            error_message: None,
            id: "sure".to_string(),
        });
        let v = serde_json::to_value(QuestionWire::from(&q)).unwrap();
        assert_eq!(v["type"], "confirm");
        assert_eq!(v["default"], true);
    }

    // A Password question NEVER serializes a default/value field.
    #[test]
    fn password_envelope_never_emits_a_default_or_value() {
        let q = Question::Password(PasswordQuestion {
            message: "API token?".to_string(),
            desc: Some("secret".to_string()),
            confirmation: Some(false),
            id: "token".to_string(),
        });
        let v = serde_json::to_value(QuestionWire::from(&q)).unwrap();
        assert_eq!(v["type"], "password");
        assert_eq!(v["id"], "token");
        // The serialized object must not carry any default/value/answer key.
        let obj = v.as_object().unwrap();
        assert!(!obj.contains_key("default"), "password must omit default");
        assert!(!obj.contains_key("value"), "password must omit value");
        assert!(!obj.contains_key("answer"), "password must omit answer");
    }

    #[test]
    fn done_envelope_is_just_status() {
        let v: serde_json::Value = serde_json::from_str(&HeadlessEnvelope::Done.to_json()).unwrap();
        assert_eq!(v["status"], "done");
        assert_eq!(HeadlessEnvelope::Done.exit_code(), 0);
        // Done must not leak any answers.
        let obj = v.as_object().unwrap();
        assert_eq!(obj.len(), 1, "done envelope carries only status");
    }

    #[test]
    fn error_envelope_carries_message_and_exit_one() {
        let env = HeadlessEnvelope::error("bad answer for id 'token'");
        let v: serde_json::Value = serde_json::from_str(&env.to_json()).unwrap();
        assert_eq!(v["status"], "error");
        assert_eq!(v["message"], "bad answer for id 'token'");
        assert_eq!(env.exit_code(), 1);
    }
}
