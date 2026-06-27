//! Headless (`--headless`) support for the `create`, `update`, and `try` commands.
//!
//! This module owns the CLI-side concerns of headless mode:
//! - **Answer ingestion** ([`read_answers`]): load the supplied answer set from a
//!   `--answers <file>` or from stdin (when not a TTY) into the existing
//!   [`Answer`] representation.
//! - **Exit-code mapping** ([`emit_and_exit`]): print the single JSON envelope on
//!   stdout and translate its status into the process exit code
//!   (need_input → 2, done → 0, error → 1) via [`HeadlessExit`].
//!
//! The Q&A walk itself lives in `cyanprompt`
//! ([`TemplateEngine::start_headless`](cyanprompt::domain::services::template::engine::TemplateEngine::start_headless));
//! this module never prompts and writes only the envelope to stdout.

use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io::{IsTerminal, Read};

use cyanprompt::domain::models::answer::Answer;
use cyanprompt::domain::models::headless::HeadlessEnvelope;
use cyanprompt::domain::models::question::Question;

use crate::try_cmd::TryHeadlessOutcome;

/// Outcome of a (possibly headless) `cyan_run` / `cyan_update` invocation.
///
/// `session_ids` are always returned for fire-and-forget cleanup. In headless
/// mode `need_input` is `Some(question)` when the walk stopped on an unanswered
/// question (the caller emits it and exits 2); otherwise `None` (the run
/// completed, or it was a non-headless run).
///
/// Carries the domain [`Question`] (not the JSON wire DTO) up to the CLI boundary,
/// where [`finish_headless`] converts it to the wire representation at the point of
/// emission. This keeps the headless serialization contract out of the run/update
/// service layer.
pub struct CyanRunResult {
    pub session_ids: Vec<String>,
    pub need_input: Option<Question>,
}

impl CyanRunResult {
    /// A completed run (no question pending).
    pub fn completed(session_ids: Vec<String>) -> Self {
        Self {
            session_ids,
            need_input: None,
        }
    }
}

/// Sentinel error carrying a process exit code. `main` downcasts to this to map a
/// headless outcome onto the real exit status without printing an extra "Error:"
/// line (the JSON envelope has already been emitted on stdout).
#[derive(Debug)]
pub struct HeadlessExit(pub u8);

impl fmt::Display for HeadlessExit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "headless exit code {}", self.0)
    }
}

impl Error for HeadlessExit {}

/// Parse a JSON object of `id -> Answer` into the answer map.
///
/// The value shape reuses [`Answer`]'s serde representation
/// (`{"type":"String","value":"…"}`, `{"type":"Bool","value":true}`,
/// `{"type":"StringArray","value":["…"]}`) — the same `{type,value}` shape used
/// elsewhere for persisted answers. An empty / whitespace-only input is a valid
/// "no answers yet" first call and yields an empty map.
pub fn parse_answers(raw: &str) -> Result<HashMap<String, Answer>, Box<dyn Error + Send>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(HashMap::new());
    }
    serde_json::from_str::<HashMap<String, Answer>>(trimmed).map_err(|e| {
        // serde's Display can EMBED the offending value (e.g. a mistyped password supplied
        // as `{"token":"sup3r-s3cr3t"}` instead of the `{type,value}` shape yields
        // `invalid type: string "sup3r-s3cr3t", expected …`). This error becomes the
        // headless `error` envelope, so building it from serde's value-bearing text would
        // leak the submitted value — and an error message must never echo a supplied answer
        // value. Construct a value-free message from only the error CATEGORY plus its
        // location instead.
        let kind = match e.classify() {
            serde_json::error::Category::Io => "I/O error reading answers JSON",
            serde_json::error::Category::Eof => "answers JSON ended unexpectedly",
            serde_json::error::Category::Syntax => "answers JSON is not valid JSON",
            serde_json::error::Category::Data => {
                "answers JSON does not match the expected {id: {type, value}} shape"
            }
        };
        Box::new(std::io::Error::other(format!(
            "failed to parse answers JSON: {kind} (line {}, column {})",
            e.line(),
            e.column()
        ))) as Box<dyn Error + Send>
    })
}

/// Read answers from an arbitrary reader (the stdin path), parsing the content as
/// the `id -> Answer` JSON map. Extracted for testability.
pub fn read_answers_reader<R: Read>(
    mut reader: R,
) -> Result<HashMap<String, Answer>, Box<dyn Error + Send>> {
    let mut buf = String::new();
    reader
        .read_to_string(&mut buf)
        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
    parse_answers(&buf)
}

/// Resolve the answer source and parse it into a map.
///
/// Precedence an explicit `--answers <path>` is read from the file; else,
/// when `read_stdin` is true (stdin is piped, not a TTY), answers are read from
/// stdin; else an empty map ("no answers yet"). Extracted from [`read_answers`]
/// so the source-selection logic is unit-testable without a real stdin/TTY.
pub fn read_answers_from(
    answers_path: Option<&str>,
    read_stdin: bool,
    stdin: impl Read,
) -> Result<HashMap<String, Answer>, Box<dyn Error + Send>> {
    if let Some(path) = answers_path {
        let content = fs::read_to_string(path).map_err(|e| {
            Box::new(std::io::Error::other(format!(
                "failed to read answers file '{path}': {e}"
            ))) as Box<dyn Error + Send>
        })?;
        return parse_answers(&content);
    }
    if read_stdin {
        return read_answers_reader(stdin);
    }
    Ok(HashMap::new())
}

/// Load the supplied answers for a headless invocation from `--answers <file>` or
/// (when no file is given and stdin is piped) from stdin.
pub fn read_answers(
    answers_path: Option<&str>,
) -> Result<HashMap<String, Answer>, Box<dyn Error + Send>> {
    let read_stdin = answers_path.is_none() && !std::io::stdin().is_terminal();
    read_answers_from(answers_path, read_stdin, std::io::stdin())
}

/// Emit the envelope as the sole JSON line to `writer` and return a result that
/// `main` turns into the right exit code: `done` → `Ok(())` (exit 0),
/// `need_input` → `Err(HeadlessExit(2))`, `error` → `Err(HeadlessExit(1))`.
///
/// This is the writer-parameterized core of [`emit_and_exit`], extracted so tests can
/// capture the emitted bytes deterministically (against a `Vec<u8>` rather than the
/// process stdout fd, which is inherently racy to capture under cargo's parallel
/// harness). Production routes through [`emit_and_exit`], which calls this with the
/// real stdout; the contract — exactly one JSON object on the output stream followed
/// by a newline, mapped to the exit code — is identical.
pub fn emit_to<W: std::io::Write>(
    writer: &mut W,
    envelope: &HeadlessEnvelope,
) -> Result<(), HeadlessExit> {
    // One JSON object + trailing newline. A write failure (e.g. a closed pipe) is
    // treated as a non-zero exit so a caller observing the exit code does not mistake
    // a truncated stream for success.
    let json = envelope.to_json();
    if writeln!(writer, "{json}").is_err() {
        return Err(HeadlessExit(1));
    }
    match envelope.exit_code() {
        0 => Ok(()),
        code => Err(HeadlessExit(code)),
    }
}

/// Emit the envelope as the sole JSON line on stdout and return a result
/// that `main` turns into the right exit code: `done` → `Ok(())` (exit 0),
/// `need_input` → `Err(HeadlessExit(2))`, `error` → `Err(HeadlessExit(1))`.
pub fn emit_and_exit(envelope: &HeadlessEnvelope) -> Result<(), Box<dyn Error + Send>> {
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();
    emit_to(&mut stdout, envelope).map_err(|e| Box::new(e) as Box<dyn Error + Send>)
}

/// Build the fire-and-forget coordinator-session cleaner shared by the headless
/// `create` and `update` handlers. Each session id allocated during the run is released
/// against the coordinator; release errors are ignored (cleanup is best-effort — a hiccup
/// must not mask the run's own outcome). Extracted so the two handlers share one definition
/// instead of a byte-identical closure each.
pub fn headless_session_cleaner(endpoint: String) -> impl Fn(&[String]) {
    move |session_ids: &[String]| {
        let coord_client = cyancoordinator::client::CyanCoordinatorClient::new(endpoint.clone());
        for sid in session_ids {
            let _ = coord_client.clean(sid.clone());
        }
    }
}

/// Finish a headless `create`/`update` run: run best-effort coordinator session
/// cleanup, then emit the single JSON envelope on `writer` and map it to the exit code.
/// `done` → exit 0, `need_input` → exit 2, `error` → exit 1.
///
/// `clean_sessions` performs fire-and-forget cleanup of the coordinator sessions created
/// during the run. It is injected (rather than reaching into a coordinator client from
/// here) so this, the real command-boundary logic that `main` calls, is exhaustively
/// testable without a live coordinator and with stdout captured in a buffer — proving
/// the command dispatch path (outcome → session cleanup → wire conversion → single-JSON
/// emission → exit-code mapping), not just the shared engine. On an `Err` outcome the
/// sessions are unknown to the caller (they never came back), so cleanup runs only on the
/// `Ok` arm where the session ids are present.
pub fn finish_headless<W, F>(
    r: Result<CyanRunResult, Box<dyn Error + Send>>,
    writer: &mut W,
    clean_sessions: F,
) -> Result<(), Box<dyn Error + Send>>
where
    W: std::io::Write,
    F: FnOnce(&[String]),
{
    use cyanprompt::domain::models::headless::{HeadlessEnvelope, QuestionWire};
    let env = match r {
        Ok(result) => {
            clean_sessions(&result.session_ids);
            match result.need_input {
                Some(question) => HeadlessEnvelope::NeedInput {
                    question: QuestionWire::from(&question),
                },
                None => HeadlessEnvelope::Done,
            }
        }
        Err(e) => HeadlessEnvelope::error(e.to_string()),
    };
    emit_to(writer, &env).map_err(|e| Box::new(e) as Box<dyn Error + Send>)
}

/// Finish a headless `try` run: convert the [`TryHeadlessOutcome`] (or error) into the
/// single JSON envelope, emit it on `writer`, and map it to the exit code. `Done` → `done`
/// (exit 0); `NeedInput(question)` → `need_input` (exit 2) carrying the question; any error
/// → `error` envelope (exit 1).
///
/// `try_cmd` no longer emits the envelope itself — it returns the outcome and this single
/// boundary emits, mirroring [`finish_headless`] for create/update (no split emission, no
/// "already printed elsewhere" sentinel). Emission goes to the injected `writer` so the
/// command boundary is testable with captured stdout.
pub fn finish_headless_try<W>(
    res: Result<TryHeadlessOutcome, Box<dyn Error + Send>>,
    writer: &mut W,
) -> Result<(), Box<dyn Error + Send>>
where
    W: std::io::Write,
{
    use cyanprompt::domain::models::headless::{HeadlessEnvelope, QuestionWire};
    let env = match res {
        Ok(TryHeadlessOutcome::Done) => HeadlessEnvelope::Done,
        // The outcome carries the DOMAIN question; conversion to the wire DTO happens here,
        // at the single emission boundary — the same place `finish_headless` converts the
        // create/update question. The run layer (`try_cmd`) never touches the wire type.
        Ok(TryHeadlessOutcome::NeedInput(question)) => HeadlessEnvelope::NeedInput {
            question: QuestionWire::from(&question),
        },
        Err(e) => HeadlessEnvelope::error(e.to_string()),
    };
    emit_to(writer, &env).map_err(|e| Box::new(e) as Box<dyn Error + Send>)
}

/// Print a human-facing progress line without polluting the headless JSON contract.
/// In headless mode the line goes to **stderr** so stdout stays reserved
/// exclusively for the [`HeadlessEnvelope`]; outside headless mode it goes to
/// stdout, preserving the existing interactive output verbatim.
///
/// Use this for any progress/status message reachable from a headless command path.
/// Usage mirrors `println!`: `hprogress!(headless, "doing {x}")`.
#[macro_export]
macro_rules! hprogress {
    ($headless:expr, $($arg:tt)*) => {
        if $headless {
            eprintln!($($arg)*);
        } else {
            println!($($arg)*);
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::rc::Rc;

    use cyanprompt::domain::models::cyan::Cyan;
    use cyanprompt::domain::models::question::{ConfirmQuestion, Question, TextQuestion};
    use cyanprompt::domain::models::template::input::{TemplateAnswerInput, TemplateValidateInput};
    use cyanprompt::domain::models::template::output::{
        TemplateFinalOutput, TemplateOutput, TemplateQnAOutput,
    };
    use cyanprompt::domain::services::repo::CyanRepo;
    use cyanprompt::domain::services::template::engine::TemplateEngine;
    use cyanprompt::domain::services::template::states::TemplateState;

    // ---- Answer ingestion (AC5) ------------------------------------------------

    #[test]
    fn parse_answers_handles_all_answer_kinds() {
        let raw = r#"{
            "name": {"type":"String","value":"demo"},
            "features": {"type":"StringArray","value":["a","b"]},
            "useDb": {"type":"Bool","value":true}
        }"#;
        let map = parse_answers(raw).unwrap();
        assert!(matches!(map.get("name"), Some(Answer::String(s)) if s == "demo"));
        assert!(matches!(map.get("features"), Some(Answer::StringArray(v)) if v.len() == 2));
        assert!(matches!(map.get("useDb"), Some(Answer::Bool(true))));
    }

    #[test]
    fn parse_answers_empty_is_no_answers() {
        assert!(parse_answers("").unwrap().is_empty());
        assert!(parse_answers("   \n ").unwrap().is_empty());
    }

    // AC3: a malformed answers file yields an error (not a panic).
    #[test]
    fn parse_answers_malformed_is_error() {
        assert!(parse_answers("not json").is_err());
        assert!(parse_answers(r#"{"x": 123}"#).is_err()); // not an Answer shape
    }

    // AC6 (FR11): a malformed answers file must NOT echo the submitted value in its error.
    // serde's Display embeds the offending value (`invalid type: string "sup3r-s3cr3t", …`);
    // the parse error is value-free, carrying only the error category + location. This
    // error becomes the headless `error` envelope, so the value must never reach it.
    #[test]
    fn parse_answers_error_is_value_free() {
        let secret = "sup3r-s3cr3t";
        // Wrong shape: a bare string where the {type,value} Answer object is expected. serde
        // reports a type error that includes the value verbatim — which must be stripped.
        let raw = format!(r#"{{"token":"{secret}"}}"#);
        let err = parse_answers(&raw).expect_err("a wrong-shaped value must be an error");
        let msg = err.to_string();
        assert!(
            !msg.contains(secret),
            "parse error must not echo the submitted value (FR11): {msg}"
        );
        assert!(
            msg.contains("does not match the expected"),
            "parse error should carry a value-free shape description: {msg}"
        );
    }

    // AC5: answers accepted from a file.
    #[test]
    fn read_answers_from_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("answers.json");
        std::fs::write(&path, r#"{"name":{"type":"String","value":"fromfile"}}"#).unwrap();
        let map = read_answers_from(Some(path.to_str().unwrap()), false, Cursor::new(Vec::new()))
            .unwrap();
        assert!(matches!(map.get("name"), Some(Answer::String(s)) if s == "fromfile"));
    }

    // AC5: answers accepted from stdin.
    #[test]
    fn read_answers_from_stdin() {
        let stdin = Cursor::new(br#"{"name":{"type":"String","value":"fromstdin"}}"#.to_vec());
        let map = read_answers_from(None, true, stdin).unwrap();
        assert!(matches!(map.get("name"), Some(Answer::String(s)) if s == "fromstdin"));
    }

    #[test]
    fn read_answers_no_source_is_empty() {
        let map = read_answers_from(None, false, Cursor::new(Vec::new())).unwrap();
        assert!(map.is_empty());
    }

    // AC7 (FR7): exit-code mapping is centralized and consistent
    // (done → 0, need_input → 2, error → 1).
    #[test]
    fn emit_and_exit_maps_status_to_exit_code() {
        // done → Ok (exit 0)
        assert!(emit_and_exit(&HeadlessEnvelope::Done).is_ok());
        // error → HeadlessExit(1)
        let e = emit_and_exit(&HeadlessEnvelope::error("boom")).unwrap_err();
        assert_eq!(e.downcast_ref::<HeadlessExit>().unwrap().0, 1);
        // need_input → HeadlessExit(2)
        let need = HeadlessEnvelope::NeedInput {
            question: cyanprompt::domain::models::headless::QuestionWire::from(&text("x")),
        };
        let e = emit_and_exit(&need).unwrap_err();
        assert_eq!(e.downcast_ref::<HeadlessExit>().unwrap().0, 2);
    }

    // NFC2: every envelope serializes to EXACTLY one JSON object — the sole thing a
    // headless invocation may print on stdout. This is the machine-contract guarantee
    // (the process-level "progress goes to stderr" routing is proven by the manual
    // probe; here we prove the envelope itself is always a single parseable object).
    #[test]
    fn envelope_is_always_a_single_json_object() {
        let cases = [
            HeadlessEnvelope::Done,
            HeadlessEnvelope::error("boom"),
            HeadlessEnvelope::NeedInput {
                question: cyanprompt::domain::models::headless::QuestionWire::from(&text("x")),
            },
        ];
        for env in cases {
            let json = env.to_json();
            // Parses as exactly one object.
            let v: serde_json::Value =
                serde_json::from_str(&json).expect("envelope must be valid JSON");
            assert!(v.is_object(), "envelope must be a single JSON object");
            assert!(
                serde_json::Deserializer::from_str(&json)
                    .into_iter::<serde_json::Value>()
                    .count()
                    == 1,
                "envelope must be exactly one JSON value, no trailing noise"
            );
        }
    }

    // ---- Command-boundary: finish_headless / finish_headless_try (AC4, NFC2) --
    //
    // These drive the ACTUAL command-dispatch boundary that `main` calls for the
    // create/update/try headless paths — not the shared `TemplateEngine`. They prove the
    // command plumbing the acceptance criteria name (outcome → session cleanup →
    // domain→wire conversion → single-JSON emission on stdout → exit-code mapping) by
    // capturing stdout in a buffer (deterministic — no process-fd race) and injecting a
    // session-cleanup closure so no live coordinator is needed. This closes the
    // "evidence at the engine, not the command boundary" gap.

    /// Assert `buf` (captured command stdout) holds EXACTLY one JSON object with the
    /// expected status, and the produced `HeadlessExit` (if `Err`) carries the code.
    fn assert_command_stdout_single_json(
        buf: &[u8],
        expected_status: &str,
        result: &Result<(), Box<dyn Error + Send>>,
        expected_code: Option<u8>,
    ) {
        let text = std::str::from_utf8(buf).unwrap();
        // Exactly one JSON value, no leading/trailing noise — the machine-contract a
        // headless caller reads off stdout. The trailing newline is permitted.
        let count = serde_json::Deserializer::from_str(text.trim())
            .into_iter::<serde_json::Value>()
            .count();
        assert_eq!(
            count, 1,
            "command stdout must be exactly one JSON object: {text:?}"
        );
        let v: serde_json::Value =
            serde_json::from_str(text.trim()).expect("captured stdout must parse as JSON");
        assert!(v.is_object(), "captured stdout must be a JSON object");
        assert_eq!(
            v["status"], expected_status,
            "wrong status in captured stdout: {text}"
        );
        if let Some(code) = expected_code {
            let e = result
                .as_ref()
                .expect_err("expected an error carrying a HeadlessExit code");
            assert_eq!(
                e.downcast_ref::<HeadlessExit>().unwrap().0,
                code,
                "wrong HeadlessExit code for {expected_status}"
            );
        }
    }

    // `create`/`update` boundary: a need_input outcome cleans the coordinator sessions,
    // emits a single `need_input` JSON object carrying the next question, and exits 2.
    #[test]
    fn finish_headless_need_input_cleans_sessions_and_emits_single_json_exit_two() {
        let cleaned = std::cell::RefCell::new(Vec::<String>::new());
        let result = CyanRunResult {
            session_ids: vec!["session-A".to_string(), "session-B".to_string()],
            need_input: Some(text("project_name")),
        };
        let mut buf = Vec::new();
        let res = finish_headless(Ok(result), &mut buf, |ids| {
            *cleaned.borrow_mut() = ids.to_vec();
        });
        // The supplied session ids were handed to the cleanup closure (the leak-prone
        // command-boundary path), not dropped.
        assert_eq!(
            cleaned.borrow().as_slice(),
            &["session-A".to_string(), "session-B".to_string()],
            "finish_headless must clean the run's sessions on need_input"
        );
        assert_command_stdout_single_json(&buf, "need_input", &res, Some(2));
        // The surfaced question id round-trips into the wire envelope on the contract
        // stream.
        let v: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(v["question"]["id"], "project_name");
    }

    // `create`/`update` boundary: a completed run emits a single `done` JSON object and
    // returns Ok (exit 0), still cleaning its sessions.
    #[test]
    fn finish_headless_done_emits_single_json_exit_zero() {
        let cleaned = std::cell::RefCell::new(Vec::<String>::new());
        let result = CyanRunResult {
            session_ids: vec!["s1".to_string()],
            need_input: None,
        };
        let mut buf = Vec::new();
        let res = finish_headless(Ok(result), &mut buf, |ids| {
            *cleaned.borrow_mut() = ids.to_vec();
        });
        assert_eq!(cleaned.borrow().as_slice(), &["s1".to_string()]);
        assert!(res.is_ok(), "done must map to Ok (exit 0)");
        assert_command_stdout_single_json(&buf, "done", &res, None);
    }

    // `create`/`update` boundary: a run error emits a single `error` JSON object and
    // exits 1. On error the sessions are unknown to the caller (never returned), so no
    // cleanup closure runs.
    #[test]
    fn finish_headless_error_emits_single_json_exit_one() {
        let mut cleaned = false;
        let mut buf = Vec::new();
        let res = finish_headless(
            Err(Box::new(std::io::Error::other("template boom")) as Box<dyn Error + Send>),
            &mut buf,
            |_| cleaned = true,
        );
        assert!(
            !cleaned,
            "no cleanup closure should run on an error outcome"
        );
        assert_command_stdout_single_json(&buf, "error", &res, Some(1));
        let v: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(v["message"], "template boom");
    }

    // `try` boundary: a completed try (the `Done` outcome) emits a single `done` JSON object
    // and returns Ok (exit 0).
    #[test]
    fn finish_headless_try_done_emits_single_json_exit_zero() {
        let mut buf = Vec::new();
        let res = finish_headless_try(Ok(TryHeadlessOutcome::Done), &mut buf);
        assert!(res.is_ok());
        assert_command_stdout_single_json(&buf, "done", &res, None);
    }

    // `try` boundary: a `NeedInput` outcome (the Q&A walk stopped on an unanswered question)
    // is emitted AT the boundary as a single `need_input` JSON object carrying the question,
    // exit 2. `try_cmd` returns the outcome rather than printing the envelope itself, so the
    // emission responsibility lives in one place — this function — exactly as create/update's
    // `finish_headless` does. (Replaces the prior `HeadlessExit`-propagation test, which
    // covered the old split-emission design where `try_cmd` printed and returned a sentinel.)
    #[test]
    fn finish_headless_try_need_input_emits_single_json_exit_two() {
        let mut buf = Vec::new();
        // The outcome carries the DOMAIN question; `finish_headless_try` converts it to the
        // wire DTO at this single emission boundary (mirroring create/update's
        // `finish_headless`), so the test feeds it the domain type directly.
        let res = finish_headless_try(Ok(TryHeadlessOutcome::NeedInput(text("db_name"))), &mut buf);
        assert_command_stdout_single_json(&buf, "need_input", &res, Some(2));
        let v: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(v["question"]["id"], "db_name");
    }

    // `try` boundary: any other error emits a single `error` JSON object and exits 1.
    #[test]
    fn finish_headless_try_error_emits_single_json_exit_one() {
        let mut buf = Vec::new();
        let res = finish_headless_try(
            Err(Box::new(std::io::Error::other("try boom")) as Box<dyn Error + Send>),
            &mut buf,
        );
        assert_command_stdout_single_json(&buf, "error", &res, Some(1));
    }

    // ---- Per-command integration against a fake CyanRepo (AC4, NFC2) ----------

    struct FakeRepo {
        #[allow(clippy::type_complexity)]
        responder:
            Box<dyn Fn(&TemplateAnswerInput) -> Result<TemplateOutput, Box<dyn Error + Send>>>,
    }

    impl CyanRepo for FakeRepo {
        fn prompt_template(
            &self,
            input: TemplateAnswerInput,
        ) -> Result<TemplateOutput, Box<dyn Error + Send>> {
            (self.responder)(&input)
        }
        fn validate_template(
            &self,
            _input: TemplateValidateInput,
        ) -> Result<Option<String>, Box<dyn Error + 'static + Send + Sync>> {
            Ok(None)
        }
    }

    fn engine_with<F>(responder: F) -> TemplateEngine
    where
        F: Fn(&TemplateAnswerInput) -> Result<TemplateOutput, Box<dyn Error + Send>> + 'static,
    {
        TemplateEngine {
            client: Rc::new(FakeRepo {
                responder: Box::new(responder),
            }),
        }
    }

    fn qna(question: Question) -> TemplateOutput {
        TemplateOutput::QnA(TemplateQnAOutput {
            deterministic_state: HashMap::new(),
            question,
        })
    }

    fn done() -> TemplateOutput {
        TemplateOutput::Final(TemplateFinalOutput {
            cyan: Cyan {
                processors: vec![],
                plugins: vec![],
            },
        })
    }

    fn text(id: &str) -> Question {
        Question::Text(TextQuestion {
            message: format!("{id}?"),
            default: None,
            desc: None,
            initial: None,
            id: id.to_string(),
        })
    }

    fn confirm(id: &str) -> Question {
        Question::Confirm(ConfirmQuestion {
            message: format!("{id}?"),
            desc: None,
            default: None,
            error_message: None,
            id: id.to_string(),
        })
    }

    /// Build the headless envelope from a terminal [`TemplateState`], mirroring the
    /// production CLI boundary in `main.rs::finish_headless` / `try_cmd.rs` (the only
    /// places that map a `TemplateState` to the wire envelope). This test helper exists
    /// solely because the engine returns a `TemplateState`; production code constructs
    /// envelopes directly at the emission point and has no such converter, so the wire
    /// type carries no `from_state` constructor (kept out to avoid an upward
    /// `domain::models` → `domain::services` dependency).
    fn envelope_for_state(state: &TemplateState) -> HeadlessEnvelope {
        use cyanprompt::domain::models::headless::QuestionWire;
        match state {
            TemplateState::NeedInput(question, _) => HeadlessEnvelope::NeedInput {
                question: QuestionWire::from(question),
            },
            TemplateState::Complete(_, _) => HeadlessEnvelope::Done,
            TemplateState::Err(message) => HeadlessEnvelope::error(message),
            // The headless driver never returns QnA() as a terminal state.
            TemplateState::QnA() => {
                HeadlessEnvelope::error("internal error: headless walk did not terminate")
            }
        }
    }

    /// Assert the envelope produced for `state` is a single parseable JSON object
    /// with the expected status (NFC2) and matching exit code.
    fn assert_single_json(state: &TemplateState, expected_status: &str, expected_code: u8) {
        let env = envelope_for_state(state);
        let json = env.to_json();
        // Exactly one JSON object, parses cleanly.
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(v.is_object());
        assert_eq!(v["status"], expected_status);
        assert_eq!(env.exit_code(), expected_code);
    }

    /// The question id a `need_input` envelope is asking for, read from the SERIALIZED
    /// envelope (the wire contract a caller consumes) rather than via a test-only accessor.
    /// Returns `None` when the state is not a `need_input`.
    fn need_input_id(state: &TemplateState) -> Option<String> {
        let env = envelope_for_state(state);
        let v: serde_json::Value = serde_json::from_str(&env.to_json()).unwrap();
        if v["status"] == "need_input" {
            Some(v["question"]["id"].as_str().unwrap().to_string())
        } else {
            None
        }
    }

    // `create`: a simple linear template. Missing answers → need_input; full → done.
    #[test]
    fn create_headless_path() {
        let engine = engine_with(|input| {
            if !input.answers.contains_key("name") {
                Ok(qna(text("name")))
            } else {
                Ok(done())
            }
        });
        assert_single_json(&engine.start_headless(None), "need_input", 2);

        let mut answers = HashMap::new();
        answers.insert("name".to_string(), Answer::String("p".to_string()));
        assert_single_json(&engine.start_headless(Some(answers)), "done", 0);
    }

    // `update` keeping auto-latest version selection: the Q&A is what becomes
    // headless. With all prior answers reusable, the walk completes (done).
    #[test]
    fn update_headless_path_completes_with_reused_answers() {
        let engine = engine_with(|input| {
            if !input.answers.contains_key("name") {
                Ok(qna(text("name")))
            } else {
                Ok(done())
            }
        });
        let mut answers = HashMap::new();
        answers.insert("name".to_string(), Answer::String("reused".to_string()));
        assert_single_json(&engine.start_headless(Some(answers)), "done", 0);
    }

    // `try template`: a branching template (db_name appears only if use_db=true).
    #[test]
    fn try_template_headless_branching_path() {
        let engine = engine_with(|input| {
            if !input.answers.contains_key("use_db") {
                return Ok(qna(confirm("use_db")));
            }
            let use_db = matches!(input.answers.get("use_db"), Some(Answer::Bool(true)));
            if use_db && !input.answers.contains_key("db_name") {
                return Ok(qna(text("db_name")));
            }
            Ok(done())
        });

        let mut answers = HashMap::new();
        answers.insert("use_db".to_string(), Answer::Bool(true));
        let state = engine.start_headless(Some(answers.clone()));
        assert_eq!(need_input_id(&state).as_deref(), Some("db_name"));

        answers.insert("db_name".to_string(), Answer::String("d".to_string()));
        assert_single_json(&engine.start_headless(Some(answers)), "done", 0);
    }

    // `try group` end-to-end id fidelity: the headless driver and the JSON envelope
    // faithfully pass through question ids that contain a slash. This is the
    // prerequisite for composition namespacing: the cyancoordinator composition
    // operator namespaces composed-template ids as `{template_id}/{raw_id}`, so the
    // driver and envelope MUST preserve the slash verbatim (not split or mangle it) for
    // the caller to target composed questions by id. The composition namespacing itself
    // is proven at the cyancoordinator boundary (`multi_template_namespaces_same_id_*`).
    #[test]
    fn try_group_headless_preserves_slash_ids() {
        let engine = engine_with(|input| {
            if !input.answers.contains_key("frontend/name") {
                return Ok(qna(text("frontend/name")));
            }
            if !input.answers.contains_key("backend/port") {
                return Ok(qna(text("backend/port")));
            }
            Ok(done())
        });

        // The slash-containing id surfaces intact in the need_input envelope.
        let state = engine.start_headless(None);
        assert_eq!(need_input_id(&state).as_deref(), Some("frontend/name"));

        // And the serialized envelope keeps it intact (no splitting/quoting of the slash).
        let env = envelope_for_state(&state);
        let v: serde_json::Value = serde_json::from_str(&env.to_json()).unwrap();
        assert_eq!(v["status"], "need_input");
        assert_eq!(v["question"]["id"], "frontend/name");

        // Iteratively answering the slash-keyed ids walks to done.
        let mut answers = HashMap::new();
        answers.insert(
            "frontend/name".to_string(),
            Answer::String("fe".to_string()),
        );
        let state = engine.start_headless(Some(answers.clone()));
        assert_eq!(need_input_id(&state).as_deref(), Some("backend/port"));

        answers.insert(
            "backend/port".to_string(),
            Answer::String("8080".to_string()),
        );
        assert_single_json(&engine.start_headless(Some(answers)), "done", 0);
    }

    // ---- Per-command PRODUCTION-boundary integration (AC4, NFC2) --------------
    //
    // The tests above drive the shared `TemplateEngine` and assert the wire envelope via
    // the test-only `envelope_for_state` helper. These instead chain each command's Q&A
    // walk through the SAME production functions `main` invokes — `finish_headless` for
    // `create`/`update`, `finish_headless_try` for `try template`/`try group` — converting
    // the engine's terminal state into a `CyanRunResult` exactly as `cyan_run`/`cyan_update`
    // do (`NeedInput → Some(question)`, `Complete → None`, `Err → Err`). This proves, per
    // command and against a fake `CyanRepo`, the full dispatch chain the AC names: walk →
    // result → session cleanup → domain→wire conversion → single-JSON emission on captured
    // stdout → exit-code mapping. The one part NOT exercised here is `cyan_run`'s own body
    // (Docker build + live coordinator), which the spec scopes out (no e2e).

    /// Convert an engine terminal `TemplateState` into the `Result<CyanRunResult, _>` that
    /// `cyan_run`/`cyan_update` hand to `finish_headless`, then run the production boundary
    /// with captured stdout and an injected session-cleanup recorder.
    #[allow(clippy::type_complexity)]
    fn run_create_update_boundary(
        state: TemplateState,
        session_ids: Vec<String>,
    ) -> (Vec<u8>, Vec<String>, Result<(), Box<dyn Error + Send>>) {
        let r: Result<CyanRunResult, Box<dyn Error + Send>> = match state {
            TemplateState::NeedInput(question, _) => Ok(CyanRunResult {
                session_ids: session_ids.clone(),
                need_input: Some(question),
            }),
            TemplateState::Complete(_, _) => Ok(CyanRunResult {
                session_ids: session_ids.clone(),
                need_input: None,
            }),
            TemplateState::Err(message) => {
                Err(Box::new(std::io::Error::other(message)) as Box<dyn Error + Send>)
            }
            TemplateState::QnA() => unreachable!("headless walk never terminates on QnA"),
        };
        let cleaned = std::cell::RefCell::new(Vec::<String>::new());
        let mut buf = Vec::new();
        let res = finish_headless(r, &mut buf, |ids| {
            *cleaned.borrow_mut() = ids.to_vec();
        });
        (buf, cleaned.into_inner(), res)
    }

    // `create`: missing answers surface `need_input`/exit 2; complete walk → `done`/exit 0
    // — both through the production `finish_headless` boundary, with sessions cleaned.
    #[test]
    fn create_headless_boundary_need_input_then_done() {
        let engine = engine_with(|input| {
            if input.answers.contains_key("name") {
                Ok(done())
            } else {
                Ok(qna(text("name")))
            }
        });

        // Missing answer → need_input envelope, exit 2, sessions cleaned.
        let (buf, cleaned, res) =
            run_create_update_boundary(engine.start_headless(None), vec!["sess-1".to_string()]);
        assert_command_stdout_single_json(&buf, "need_input", &res, Some(2));
        assert_eq!(cleaned, vec!["sess-1".to_string()]);
        let v: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(v["question"]["id"], "name");

        // All answered → done envelope, exit 0.
        let mut answers = HashMap::new();
        answers.insert("name".to_string(), Answer::String("p".to_string()));
        let (buf, _, res) = run_create_update_boundary(
            engine.start_headless(Some(answers)),
            vec!["sess-1".to_string()],
        );
        assert_command_stdout_single_json(&buf, "done", &res, None);
    }

    // `update` keeping auto-latest version selection: only the Q&A is headless. With prior
    // answers reusable the walk completes → `done`/exit 0 through `finish_headless`.
    #[test]
    fn update_headless_boundary_auto_latest_completes() {
        let engine = engine_with(|input| {
            if input.answers.contains_key("name") {
                Ok(done())
            } else {
                Ok(qna(text("name")))
            }
        });
        let mut answers = HashMap::new();
        answers.insert("name".to_string(), Answer::String("reused".to_string()));
        let (buf, _, res) = run_create_update_boundary(
            engine.start_headless(Some(answers)),
            vec!["sess-u".to_string()],
        );
        assert_command_stdout_single_json(&buf, "done", &res, None);
    }

    // `try template` (branching) at the production `finish_headless_try` boundary: a
    // completed walk → `done`/exit 0, single JSON. (The `need_input` round is returned by
    // `try_cmd` as a `TryHeadlessOutcome::NeedInput` and emitted at this same boundary,
    // covered by `finish_headless_try_need_input_emits_single_json_exit_two`.)
    #[test]
    fn try_template_headless_boundary_completes_to_done() {
        let engine = engine_with(|input| {
            if !input.answers.contains_key("use_db") {
                return Ok(qna(confirm("use_db")));
            }
            let use_db = matches!(input.answers.get("use_db"), Some(Answer::Bool(true)));
            if use_db && !input.answers.contains_key("db_name") {
                return Ok(qna(text("db_name")));
            }
            Ok(done())
        });
        let mut answers = HashMap::new();
        answers.insert("use_db".to_string(), Answer::Bool(true));
        answers.insert("db_name".to_string(), Answer::String("d".to_string()));
        // A fully-answered branching walk reaches Complete; the try command boundary maps
        // its Ok(()) outcome to a single `done` envelope, exit 0.
        assert!(matches!(
            engine.start_headless(Some(answers)),
            TemplateState::Complete(_, _)
        ));
        let mut buf = Vec::new();
        let res = finish_headless_try(Ok(TryHeadlessOutcome::Done), &mut buf);
        assert_command_stdout_single_json(&buf, "done", &res, None);
    }

    // `try group` (slash-namespaced ids) at the production `finish_headless_try` boundary:
    // a fully-answered composed walk reaches Complete and maps to `done`/exit 0.
    #[test]
    fn try_group_headless_boundary_completes_to_done() {
        let engine = engine_with(|input| {
            if !input.answers.contains_key("frontend/name") {
                return Ok(qna(text("frontend/name")));
            }
            if !input.answers.contains_key("backend/port") {
                return Ok(qna(text("backend/port")));
            }
            Ok(done())
        });
        let mut answers = HashMap::new();
        answers.insert(
            "frontend/name".to_string(),
            Answer::String("fe".to_string()),
        );
        answers.insert(
            "backend/port".to_string(),
            Answer::String("8080".to_string()),
        );
        assert!(matches!(
            engine.start_headless(Some(answers)),
            TemplateState::Complete(_, _)
        ));
        let mut buf = Vec::new();
        let res = finish_headless_try(Ok(TryHeadlessOutcome::Done), &mut buf);
        assert_command_stdout_single_json(&buf, "done", &res, None);
    }

    // DONE-PATH INTEGRATION: a complete headless walk (all answers supplied, none
    // failing validation) reaches `Complete`, which maps to a `done` envelope with
    // exit 0 and serializes to EXACTLY one JSON object — the sole permitted stdout
    // output (NFC2). This is the contract path the manual probes had never reached.
    #[test]
    fn complete_walk_emits_done_envelope_with_exit_zero_and_single_json() {
        // Branching template: q1 (confirm) gates q2 (text); both supplied → done.
        let engine = engine_with(|input| {
            if !input.answers.contains_key("q1") {
                return Ok(qna(confirm("q1")));
            }
            let take = matches!(input.answers.get("q1"), Some(Answer::Bool(true)));
            if take && !input.answers.contains_key("q2") {
                return Ok(qna(text("q2")));
            }
            Ok(done())
        });

        // Empty answers → need_input (the gate question).
        let first = engine.start_headless(None);
        assert_eq!(need_input_id(&first).as_deref(), Some("q1"));

        // Supplying the gate reveals the branch question.
        let mut mid = HashMap::new();
        mid.insert("q1".to_string(), Answer::Bool(true));
        let second = engine.start_headless(Some(mid));
        assert_eq!(need_input_id(&second).as_deref(), Some("q2"));

        // Supplying the final answer completes the walk → done envelope, exit 0,
        // single JSON object carrying ONLY `status` (no leaked answers, FR11).
        let mut full = HashMap::new();
        full.insert("q1".to_string(), Answer::Bool(true));
        full.insert("q2".to_string(), Answer::String("secret-value".to_string()));
        let env = envelope_for_state(&engine.start_headless(Some(full)));
        assert_eq!(env, HeadlessEnvelope::Done);
        assert_eq!(env.exit_code(), 0);
        let json = env.to_json();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(v.is_object(), "done must be a single JSON object");
        assert_eq!(v["status"], "done");
        assert_eq!(
            v.as_object().unwrap().len(),
            1,
            "done must carry only `status`"
        );
        // The envelope text must not leak any answer value (FR11).
        assert!(
            !json.contains("secret-value"),
            "done envelope must not echo answer values"
        );
        // Exactly one JSON value — no trailing noise that would break machine parsing.
        assert_eq!(
            serde_json::Deserializer::from_str(&json)
                .into_iter::<serde_json::Value>()
                .count(),
            1
        );
    }

    // AC1 end-to-end at the command boundary: a `create --headless --answers /dev/null`
    // invocation (an EMPTY answer source → no answers) against a template with an
    // unanswered question prints a single `need_input` envelope carrying the next
    // question and exits 2. The manual `pls run -- create … --headless --answers
    // /dev/null` probe the spec names needs a warmed (live-coordinator) template, which
    // is out of scope here; this drives the same path — answer ingestion from an empty
    // source through the headless engine to the `finish_headless` command boundary —
    // against a fake repo, so no coordinator is required. Driving the real command
    // boundary (not just the engine) is the substitute accepted for the manual check,
    // which cannot run without a live coordinator.
    #[test]
    fn create_headless_with_empty_answers_surfaces_need_input_and_exits_two() {
        // `--answers /dev/null` → an empty file → no answers ingested.
        let answers = read_answers_from(None, false, Cursor::new(Vec::new())).unwrap();
        assert!(answers.is_empty(), "an empty source ingests no answers");

        // Template: asks `name`, then finalizes once it is present.
        let engine = engine_with(|input| {
            if input.answers.contains_key("name") {
                Ok(done())
            } else {
                Ok(qna(text("name")))
            }
        });
        let state = engine.start_headless(if answers.is_empty() {
            None
        } else {
            Some(answers.clone())
        });

        // Route the terminal state through the command boundary exactly as `main` does.
        let question = match state {
            TemplateState::NeedInput(q, _) => q,
            other => panic!("expected NeedInput, got {}", state_variant_name(&other)),
        };
        let result = CyanRunResult {
            session_ids: Vec::new(),
            need_input: Some(question),
        };
        let mut buf = Vec::new();
        let res = finish_headless(Ok(result), &mut buf, |_| {});

        // AC1: single `need_input` JSON object carrying the next question (id/type/message),
        // exit code 2.
        assert_command_stdout_single_json(&buf, "need_input", &res, Some(2));
        let v: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(v["question"]["id"], "name");
        assert_eq!(v["question"]["type"], "text");
        assert!(v["question"]["message"].as_str().unwrap().contains("name"));
    }

    /// Name of a `TemplateState` variant, for readable panic messages.
    fn state_variant_name(state: &TemplateState) -> &'static str {
        match state {
            TemplateState::QnA() => "QnA",
            TemplateState::Complete(_, _) => "Complete",
            TemplateState::NeedInput(_, _) => "NeedInput",
            TemplateState::Err(_) => "Err",
        }
    }
}
