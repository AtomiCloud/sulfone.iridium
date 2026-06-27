use std::collections::HashMap;
use std::process::exit;
use std::rc::Rc;

use crate::domain::models::answer::Answer;
use crate::domain::models::question::Question;
use crate::domain::models::question::QuestionTrait;
use crate::domain::models::template::input::TemplateAnswerInput;
use crate::domain::models::template::output::TemplateOutput;
use crate::domain::services::prompter::prompt;
use crate::domain::services::repo::CyanRepo;
use crate::domain::services::template::redact;
use crate::domain::services::template::states::TemplateState;
use crate::domain::services::template::validate::add_template_validator;
use crate::http::mapper::prompt_mapper;

pub struct TemplateEngine {
    pub client: Rc<dyn CyanRepo>,
}

impl TemplateEngine {
    pub fn new(client: Rc<dyn CyanRepo>) -> TemplateEngine {
        TemplateEngine { client }
    }

    pub fn start_with(
        &self,
        initial_answers: Option<HashMap<String, Answer>>,
        initial_states: Option<HashMap<String, String>>,
    ) -> TemplateState {
        println!("TemplateEngine started");
        let mut state = TemplateState::QnA();

        // Track answer
        let mut answers: HashMap<String, Answer> = initial_answers.unwrap_or_default();
        let mut state_data: HashMap<String, String> = initial_states.clone().unwrap_or_default();
        let mut last_question_id: Option<String> = None;

        while state.cont() {
            let input = TemplateAnswerInput {
                answers: answers.clone(),
                deterministic_state: state_data.clone(),
            };
            let result = self
                .client
                .prompt_template(input)
                .and_then(|resp| match resp {
                    TemplateOutput::QnA(q) => {
                        // Get ID from the Question struct using pattern matching
                        let question_id = &q.question.id();
                        last_question_id = Some(question_id.clone());

                        let ans = prompt_mapper(&q.question)
                            .map(|p| {
                                add_template_validator(
                                    p,
                                    Rc::clone(&self.client),
                                    answers.clone(),
                                    state_data.clone(),
                                )
                            })
                            .and_then(|p| prompt(p))
                            // handle responses
                            .map(|x| match x {
                                // if skipped
                                None => {
                                    if answers.is_empty() {
                                        println!("User aborted! Exiting...");
                                        exit(0)
                                    } else if let Some(last_id) = &last_question_id {
                                        answers.remove(last_id);
                                    }
                                }
                                Some(val) => {
                                    answers.insert(question_id.clone(), val);
                                    state_data = q.deterministic_state;
                                }
                            });
                        match ans {
                            Ok(_) => Ok(TemplateState::QnA()),
                            Err(err) => Err(err),
                        }
                    }
                    TemplateOutput::Final(c) => {
                        Ok(TemplateState::Complete(c.cyan, answers.clone()))
                    }
                });

            state = match result {
                Ok(ok) => ok,
                Err(err) => TemplateState::Err(err.to_string()),
            };
        }
        state
    }

    /// Drive the template Q&A non-interactively from a supplied answer set.
    ///
    /// Replays the same `prompt_template` loop as [`start_with`](Self::start_with),
    /// but instead of prompting the user it:
    /// - validates each supplied answer against the coordinator (the same
    ///   `validate_template` call the interactive path's `inquire` validator makes)
    ///   and returns [`TemplateState::Err`] when it fails; on success it re-derives
    ///   deterministic state and continues the walk;
    /// - stops and returns [`TemplateState::NeedInput`] carrying the question and the
    ///   deterministic state accumulated so far when the coordinator asks a question
    ///   whose `id` is **not** in `answers` (emit-and-stop, the headless contract);
    /// - returns [`TemplateState::Complete`] when the coordinator finalizes;
    /// - returns [`TemplateState::Err`] on any transport/coordinator error.
    ///
    /// The walk is stateless across invocations: deterministic state is re-derived
    /// internally by the replay, never supplied by the caller. This method performs NO
    /// interactive prompting and writes nothing to stdout, so its output is safe to wrap
    /// in a machine-parseable envelope.
    pub fn start_headless(
        &self,
        initial_answers: Option<HashMap<String, Answer>>,
    ) -> TemplateState {
        let answers: HashMap<String, Answer> = initial_answers.unwrap_or_default();

        // Eager validation (model-independent): validate EVERY supplied answer whose
        // question the coordinator reveals — BEFORE the outcome walk. This runs a
        // discovery walk that feeds answers incrementally (an empty "discovered" map),
        // which forces the coordinator to reveal each question one at a time even when
        // all answers were supplied up front. That closes the first-unanswered gap: a
        // coordinator that finalizes as soon as every id is present (never re-emitting an
        // answered question) would otherwise accept an invalid supplied answer as `done`.
        // A failing answer is surfaced as `Err` referencing the question id only, never
        // the offending value.
        //
        // The discovery walk also yields the precise set of supplied answers that were
        // actually revealed and accepted — the ONLY answers the caller may persist on
        // completion. Supplied answers for questions the coordinator never asked (an
        // untaken branch, an over-supplied value) are excluded, so a stale answer —
        // including a secret for a branch that was not taken — can never be carried into
        // persisted state and silently reused when a later run takes that branch. This
        // is the model-independent source of truth; see `validate_supplied_answers` for
        // why it (not the outcome walk's `walked` snapshot) is correct under both
        // coordinator models.
        let revealed = match self.validate_supplied_answers(&answers) {
            Ok(revealed) => revealed,
            Err(msg) => return TemplateState::Err(msg),
        };

        let mut state_data: HashMap<String, String> = HashMap::new();
        // Answers the outcome walk has already progressed past, in the order the
        // coordinator revealed them. This is the pre-insertion snapshot used for in-walk
        // validation: the interactive path captures answers BEFORE inserting the current
        // one, so the sibling map here contains only earlier answers. It is populated as
        // the loop advances, so it never contains the current question's answer or any
        // not-yet-revealed (future) answer.
        let mut walked: HashMap<String, Answer> = HashMap::new();
        // Tracks supplied answer values the walk has put in flight, split by secrecy so a
        // `prompt_template` transport error can be redacted: a coordinator/transport error
        // string can echo any answer value currently in the request body, so an error that
        // echoes a SECRET (Password) value must never reach the headless error envelope,
        // and an error echoing a non-secret value is dropped to be safe. Refined as
        // questions are revealed (the question kind is what marks a value as secret), but
        // seeded up front from ALL supplied answers: the FIRST `prompt_template` call below
        // sends the full supplied set, so an error it returns before any question has been
        // revealed must already be redactable against every supplied value — otherwise a
        // secret supplied answer could leak through that early error.
        let mut in_flight = InFlightAnswers::new();
        in_flight.seed_all(&answers);

        // Safety bound: a misbehaving coordinator that re-emits an already-answered
        // question without progressing would otherwise spin forever. This mirrors the
        // unbounded `start_with` loop but caps the headless (automation-oriented) walk.
        const MAX_HEADLESS_ITERATIONS: usize = 1000;
        for _ in 0..MAX_HEADLESS_ITERATIONS {
            let input = TemplateAnswerInput {
                answers: answers.clone(),
                deterministic_state: state_data.clone(),
            };

            match self.client.prompt_template(input) {
                Ok(TemplateOutput::Final(c)) => {
                    // Persist ONLY the answers the coordinator actually revealed and
                    // accepted during the discovery walk — not the full caller-supplied
                    // map. `revealed` excludes over-supplied / untaken-branch answers
                    // (and their secrets), so a stale value for a question that was never
                    // prompted cannot leak into persisted state and be silently reused on
                    // a later run where the branch changes.
                    return TemplateState::Complete(c.cyan, revealed);
                }
                Ok(TemplateOutput::QnA(q)) => {
                    let question_id = q.question.id();
                    if let Some(answer) = answers.get(&question_id) {
                        // Record the supplied answer for this revealed question in the
                        // in-flight tracker (split by secrecy) so a later transport error
                        // from `prompt_template` can be redacted. Done BEFORE validation
                        // so the value is tracked even if the coordinator rejects it.
                        in_flight.record(&q.question, answer);
                        // Validate the supplied answer through the SAME path the
                        // interactive `inquire` validator uses. The interactive path
                        // captures `answers` BEFORE inserting the current answer, so
                        // mirror that exactly: the sibling map must contain ONLY the
                        // answers the walk has already progressed PAST (i.e. the
                        // questions revealed before this one), with the current id
                        // excluded. Using the full supplied map here would let a
                        // coordinator validator that inspects sibling answers see a
                        // FUTURE answer the interactive path would not have had yet,
                        // which could flip a Valid result to Invalid and break the
                        // stateless-replay contract. `walked` accumulates answers in
                        // walk order as the loop advances; it does not yet contain the
                        // current question's answer, so it is the correct pre-insertion
                        // snapshot. Eager pre-validation (`validate_supplied_answers`)
                        // already covered this answer; the in-walk check is a
                        // defense-in-depth for the sequence model. The coordinator runs
                        // this validation; the headless driver simply refuses to ingest
                        // an invalid answer (mirroring how the interactive prompt would
                        // reject it). On success it re-derives deterministic state and
                        // continues; on failure it surfaces an error referencing the
                        // question id only, never the value.
                        match crate::domain::services::template::validate::validate_answer(
                            &q.question,
                            answer,
                            Rc::clone(&self.client),
                            walked.clone(),
                            state_data.clone(),
                        ) {
                            Ok(()) => {
                                // Answer already supplied and valid — record it in the
                                // walk-order snapshot, re-derive deterministic state
                                // from the coordinator's response, and continue.
                                walked.insert(question_id, answer.clone());
                                state_data = q.deterministic_state;
                            }
                            Err(msg) => {
                                return TemplateState::Err(msg);
                            }
                        }
                    } else {
                        // First unanswered question — emit and stop.
                        return TemplateState::NeedInput(q.question, q.deterministic_state);
                    }
                }
                Err(err) => {
                    // A `prompt_template` transport/coordinator error. The raw error
                    // string can echo any answer value currently in the request body (e.g.
                    // "template crashed while applying token=s3cr3t-token"), so redact it
                    // before it becomes an error envelope: a message echoing a SECRET
                    // (Password) value is dropped entirely, and one echoing a non-secret
                    // supplied value is dropped as well. Either way the message references
                    // no answer value.
                    return TemplateState::Err(in_flight.redact_error(&err.to_string()));
                }
            }
        }

        // The walk did not converge within the cap — treat it as an error rather than
        // looping indefinitely. References ids, never answer values.
        TemplateState::Err(format!(
            "headless Q&A exceeded {MAX_HEADLESS_ITERATIONS} iterations without completing"
        ))
    }

    /// Eagerly validate every supplied answer whose question the coordinator reveals
    /// (model-independent), and return the subset of supplied answers that were actually
    /// revealed and accepted during the walk.
    ///
    /// Runs a discovery walk that feeds answers INCREMENTALLY: it starts from an empty
    /// discovered map and, each time the coordinator reveals a question, records and
    /// validates the supplied answer for that id before continuing. Feeding answers one
    /// at a time (rather than all up front) forces BOTH coordinator models to reveal
    /// every question:
    /// - **Sequence model**: re-emits an answered question until it observes det-state
    ///   echoed back — the question is revealed each round.
    /// - **First-unanswered model**: returns the first question whose id is absent;
    ///   incrementally recording it advances the walk so the next call reveals the
    ///   next question, all the way to `Final`.
    ///
    /// This closes the first-unanswered gap: a first-unanswered coordinator that would
    /// otherwise finalize as soon as every supplied id is present (accepting an invalid
    /// value as `done`) is forced to reveal each question so its answer is validated.
    /// Only the supplied ids that the coordinator actually asks about are validated —
    /// ids the coordinator never reveals (e.g. an answer for a branch never taken) are
    /// not checked, matching interactive behavior (the question was never prompted).
    ///
    /// The returned map is the precise set of answers the caller's completion state may
    /// persist: it contains ONLY the ids the coordinator revealed during the walk and
    /// accepted. Supplied answers the coordinator never asked about (a branch not taken,
    /// an over-supplied answer) are excluded, so a stale answer for a question that was
    /// never prompted — including a secret for an untaken branch — can never be carried
    /// into persisted state and silently reused on a later run where the branch changes.
    /// This is the model-independent source of truth for "answers on the revealed path":
    /// unlike the outcome walk's `walked` snapshot, the discovery walk reveals every
    /// question one at a time (it never short-circuits on the first `Final`), so it
    /// accumulates the full revealed set under BOTH coordinator models.
    ///
    /// Validates against the sibling map of answers seen so far (the current id
    /// excluded), mirroring the interactive `inquire` validator's pre-insertion
    /// snapshot. On failure returns `Err(message)` where the message references the
    /// question id only, never the offending value.
    fn validate_supplied_answers(
        &self,
        answers: &HashMap<String, Answer>,
    ) -> Result<HashMap<String, Answer>, String> {
        if answers.is_empty() {
            return Ok(HashMap::new());
        }

        let mut discovered: HashMap<String, Answer> = HashMap::new();
        let mut state_data: HashMap<String, String> = HashMap::new();
        // Same in-flight secrecy tracking as the outcome walk: a `prompt_template`
        // transport error during discovery can echo any supplied value, so secret and
        // echoed-value messages must be redacted before reaching the error envelope.
        let mut in_flight = InFlightAnswers::new();

        const MAX_VALIDATION_ITERATIONS: usize = 1000;
        for _ in 0..MAX_VALIDATION_ITERATIONS {
            let input = TemplateAnswerInput {
                answers: discovered.clone(),
                deterministic_state: state_data.clone(),
            };

            match self.client.prompt_template(input) {
                Ok(TemplateOutput::Final(_)) => {
                    // The coordinator has no further questions. Any supplied answers
                    // it did not reveal (e.g. answers for untaken branches) are not
                    // validated — the interactive path would never have prompted for
                    // them either — and are NOT in `discovered`, so they are dropped
                    // from the returned (persistable) set.
                    return Ok(discovered);
                }
                Ok(TemplateOutput::QnA(q)) => {
                    let question_id = q.question.id();
                    match answers.get(&question_id) {
                        Some(answer) => {
                            // Track the value in flight (split by secrecy) BEFORE
                            // validation so it is covered even if rejected.
                            in_flight.record(&q.question, answer);
                            // Validate the supplied answer against the siblings seen so
                            // far (the current id excluded — mirrors the interactive
                            // validator's pre-insertion snapshot). On failure, surface an
                            // id-only error.
                            //
                            // Validate against the deterministic state accumulated BEFORE
                            // this question (`state_data`), NOT the question's own
                            // `q.deterministic_state` (which the coordinator already
                            // advanced PAST this question). The interactive path installs
                            // its validator with the pre-question snapshot, so a coordinator
                            // validator that inspects deterministic state must see the same
                            // input in both modes.
                            let mut siblings = discovered.clone();
                            siblings.remove(&question_id);
                            crate::domain::services::template::validate::validate_answer(
                                &q.question,
                                answer,
                                Rc::clone(&self.client),
                                siblings,
                                state_data.clone(),
                            )?;
                            // Valid — record it and thread deterministic state so the
                            // next call advances the walk to the following question.
                            discovered.insert(question_id, answer.clone());
                            state_data = q.deterministic_state;
                        }
                        None => {
                            // The coordinator is asking a question the caller has NOT
                            // supplied an answer for. Stop the discovery walk: any
                            // remaining supplied answers belong to questions further in
                            // the walk that cannot be reached yet (they depend on this
                            // unanswered question). They were never prompted for, so
                            // they are not validated — matching interactive behavior —
                            // and `discovered` holds only the revealed-so-far set.
                            return Ok(discovered);
                        }
                    }
                }
                Err(err) => {
                    // Redact a transport/coordinator error the same way the outcome walk
                    // does: drop a message that echoes a secret or a supplied value. The
                    // message references no answer value.
                    return Err(in_flight.redact_error(&err.to_string()));
                }
            }
        }

        // Same misbehaving-coordinator bound as the outcome walk (references no values).
        Err(format!(
            "headless validation exceeded {MAX_VALIDATION_ITERATIONS} iterations without completing"
        ))
    }
}

/// Tracks the supplied answer values a headless walk has put in flight, split by
/// secrecy, so a `prompt_template` transport/coordinator error can be redacted before it
/// becomes the headless error envelope. A coordinator/transport error string can echo any
/// answer value present in the request body, so:
/// - a message echoing a SECRET (Password) value is dropped entirely, and
/// - a message echoing a non-secret supplied value is also dropped (it may be echoing it).
///
/// This mirrors the redaction already applied to `validate_answer` rejections, extended to
/// the transport-error surface that the walk itself hits. A value is marked secret by the
/// KIND of the question that revealed it (Password); all other revealed string values are
/// treated as non-secret.
struct InFlightAnswers {
    secret_values: Vec<String>,
    known_values: Vec<String>,
}

impl InFlightAnswers {
    fn new() -> Self {
        Self {
            secret_values: Vec::new(),
            known_values: Vec::new(),
        }
    }

    /// Record a revealed question's supplied answer value, classified by secrecy, so a
    /// later transport error echoing any of it can be redacted.
    ///
    /// String-valued answers are tracked, classified by question kind; see
    /// [`answer_renderings`](Self::answer_renderings) for why boolean answers are not.
    /// - `Answer::String` (Text/Select/Password) is the value verbatim. A `Password` is a
    ///   SECRET; the rest are non-secret.
    /// - `Answer::StringArray` (Checkbox) elements are tracked as non-secret values.
    ///   These are template-author option labels the caller selected, but the letter of
    ///   the secrecy contract (errors must never echo a supplied answer value) covers
    ///   them too, and redacting an echoed label is always safe — it can leak nothing.
    fn record(&mut self, question: &Question, answer: &Answer) {
        let secret = matches!(question, Question::Password(_));
        for value in Self::answer_renderings(answer) {
            if secret {
                self.secret_values.push(value);
            } else {
                self.known_values.push(value);
            }
        }
    }

    /// Seed the tracker with EVERY supplied answer value BEFORE the outcome walk's first
    /// `prompt_template` call.
    ///
    /// That first call sends the FULL supplied answer set (`answers.clone()`), so a
    /// transport/coordinator error it returns can echo ANY supplied value — including a
    /// secret — before the walk has revealed the corresponding question and `record`ed it.
    /// Without this baseline the early error would be redacted against an empty
    /// known-values set and leak the value into the headless error envelope.
    ///
    /// Question kinds are not known here (only the answer map is), so every value goes in
    /// the non-secret bucket. That is safe for the no-secret-output guarantee:
    /// `redact_error` drops a message that echoes a secret OR a non-secret value to the
    /// SAME value-free generic message, so a password value seeded here is still never
    /// emitted — only the (cosmetic) secret/non-secret classification is coarser for the
    /// pre-reveal window, which the per-question `record` calls refine as the walk
    /// progresses.
    fn seed_all(&mut self, answers: &HashMap<String, Answer>) {
        for answer in answers.values() {
            self.known_values.extend(Self::answer_renderings(answer));
        }
    }

    /// Render an answer's value(s) into the string forms a coordinator/transport error
    /// might echo. Shared by [`record`](Self::record) (per revealed question) and
    /// [`seed_all`](Self::seed_all) (the pre-walk baseline) so both track identical forms.
    ///
    /// Delegates to the crate-shared [`redact::answer_renderings`] so this surface and the
    /// validate-path sibling capture use ONE renderer with one boolean policy and cannot
    /// drift (strings tracked, booleans not, empties skipped — see that function's docs).
    fn answer_renderings(answer: &Answer) -> Vec<String> {
        redact::answer_renderings(answer)
    }

    /// Redact a `prompt_template` error message. If the message echoes any secret value it
    /// is replaced with a value-free generic message; if it echoes any non-secret supplied
    /// value it is likewise replaced. Otherwise the original message is kept (it carries
    /// useful, non-leaking detail). The returned message never contains a supplied answer
    /// value.
    ///
    /// Matching uses [`redact::value_echoed`], which catches the raw value AND its common
    /// encoded renderings (JSON-string escaping plus URL/form encoding — `%20` and `+` for a
    /// space), so a value echoed in an escaped or URL/form-encoded form inside an error body
    /// cannot slip past as a naive `contains` check would let it.
    fn redact_error(&self, raw: &str) -> String {
        const GENERIC: &str = "template coordinator error (see coordinator logs)";
        if self
            .secret_values
            .iter()
            .any(|s| redact::value_echoed(raw, s))
        {
            return GENERIC.to_string();
        }
        if self
            .known_values
            .iter()
            .any(|s| redact::value_echoed(raw, s))
        {
            return GENERIC.to_string();
        }
        raw.to_string()
    }
}

#[cfg(test)]
mod headless_tests {
    use super::*;
    use crate::domain::models::cyan::Cyan;
    use crate::domain::models::question::{
        ConfirmQuestion, Question, SelectQuestion, TextQuestion,
    };
    use crate::domain::models::template::output::{TemplateFinalOutput, TemplateQnAOutput};
    use std::error::Error;

    /// A scripted fake `CyanRepo` driven by a closure over the incoming answers.
    /// No real coordinator / HTTP involved — exercises the `prompt_template`
    /// trait boundary directly (spec §6).
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
            _input: crate::domain::models::template::input::TemplateValidateInput,
        ) -> Result<Option<String>, Box<dyn Error + 'static + Send + Sync>> {
            Ok(None)
        }
    }

    /// A fake repo whose `validate_template` rejects a specific answer value, to
    /// exercise the FR9 validation-failure path. It drives the Q&A via the same
    /// `responder` closure as [`FakeRepo`].
    struct ValidatingFakeRepo {
        #[allow(clippy::type_complexity)]
        responder:
            Box<dyn Fn(&TemplateAnswerInput) -> Result<TemplateOutput, Box<dyn Error + Send>>>,
        /// Reject any answer whose `validate` value equals this string.
        reject_value: String,
    }

    impl CyanRepo for ValidatingFakeRepo {
        fn prompt_template(
            &self,
            input: TemplateAnswerInput,
        ) -> Result<TemplateOutput, Box<dyn Error + Send>> {
            (self.responder)(&input)
        }

        fn validate_template(
            &self,
            input: crate::domain::models::template::input::TemplateValidateInput,
        ) -> Result<Option<String>, Box<dyn Error + 'static + Send + Sync>> {
            if input.validate == self.reject_value {
                Ok(Some("that value is not allowed".to_string()))
            } else {
                Ok(None)
            }
        }
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

    /// A `Password` (secret-typed) question.
    fn password(id: &str) -> Question {
        use crate::domain::models::question::PasswordQuestion;
        Question::Password(PasswordQuestion {
            message: format!("{id}?"),
            desc: None,
            confirmation: None,
            id: id.to_string(),
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

    /// A `Select` question offering the given options.
    fn select(id: &str, options: &[&str]) -> Question {
        Question::Select(SelectQuestion {
            message: format!("{id}?"),
            desc: None,
            options: options.iter().map(|s| s.to_string()).collect(),
            id: id.to_string(),
        })
    }

    /// A `Checkbox` question offering the given options.
    fn checkbox(id: &str, options: &[&str]) -> Question {
        use crate::domain::models::question::CheckboxQuestion;
        Question::Checkbox(CheckboxQuestion {
            message: format!("{id}?"),
            options: options.iter().map(|s| s.to_string()).collect(),
            desc: None,
            id: id.to_string(),
        })
    }

    fn qna(question: Question, det_key: &str) -> TemplateOutput {
        let mut deterministic_state = HashMap::new();
        deterministic_state.insert(det_key.to_string(), "seen".to_string());
        TemplateOutput::QnA(TemplateQnAOutput {
            deterministic_state,
            question,
        })
    }

    fn final_output() -> TemplateOutput {
        TemplateOutput::Final(TemplateFinalOutput {
            cyan: Cyan {
                processors: vec![],
                plugins: vec![],
            },
        })
    }

    /// Build a BRANCHING template: asks `use_db`; only if `use_db == true` does it
    /// reveal the `db_name` branch question; otherwise it finalizes.
    fn branching_engine() -> TemplateEngine {
        let responder = Box::new(|input: &TemplateAnswerInput| {
            if !input.answers.contains_key("use_db") {
                return Ok(qna(confirm("use_db"), "use_db"));
            }
            let use_db = matches!(input.answers.get("use_db"), Some(Answer::Bool(true)));
            if use_db && !input.answers.contains_key("db_name") {
                return Ok(qna(text("db_name"), "db_name"));
            }
            Ok(final_output())
        });
        TemplateEngine {
            client: Rc::new(FakeRepo { responder }),
        }
    }

    fn need_input_id(state: &TemplateState) -> Option<String> {
        match state {
            TemplateState::NeedInput(q, _) => Some(q.id()),
            _ => None,
        }
    }

    // AC1: with answers that don't cover the template, the first unanswered
    // question is surfaced as NeedInput.
    #[test]
    fn returns_need_input_for_first_missing_answer() {
        let engine = branching_engine();
        let state = engine.start_headless(None);
        assert_eq!(need_input_id(&state).as_deref(), Some("use_db"));
    }

    // AC2: feeding answers iteratively walks the BRANCH to completion.
    #[test]
    fn iterative_answers_walk_branch_to_done() {
        let engine = branching_engine();

        // First answer reveals the branch question.
        let mut answers = HashMap::new();
        answers.insert("use_db".to_string(), Answer::Bool(true));
        let state = engine.start_headless(Some(answers.clone()));
        assert_eq!(
            need_input_id(&state).as_deref(),
            Some("db_name"),
            "branch question must surface only after use_db=true"
        );

        // Supplying the branch answer reaches completion.
        answers.insert("db_name".to_string(), Answer::String("mydb".to_string()));
        let state = engine.start_headless(Some(answers));
        assert!(
            matches!(state, TemplateState::Complete(_, _)),
            "all answers present must complete"
        );
    }

    // AC2 (negative branch): the branch question never appears when use_db=false.
    #[test]
    fn branch_not_taken_completes_without_branch_question() {
        let engine = branching_engine();
        let mut answers = HashMap::new();
        answers.insert("use_db".to_string(), Answer::Bool(false));
        let state = engine.start_headless(Some(answers));
        assert!(matches!(state, TemplateState::Complete(_, _)));
    }

    // Transport / coordinator errors surface as Err (FR9).
    #[test]
    fn transport_error_becomes_err_state() {
        let engine = TemplateEngine {
            client: Rc::new(FakeRepo {
                responder: Box::new(|_| {
                    Err(Box::new(std::io::Error::other("coordinator down"))
                        as Box<dyn Error + Send>)
                }),
            }),
        };
        let state = engine.start_headless(None);
        match state {
            TemplateState::Err(msg) => assert!(msg.contains("coordinator down")),
            _ => panic!("expected Err state"),
        }
    }

    // A `prompt_template` transport error that echoes a SECRET (Password) value currently
    // in flight must be redacted: the error envelope references no value. The eager
    // discovery walk reveals the password question (recording the secret), then the next
    // call returns a transport error that echoes the secret verbatim — the surfaced error
    // must NOT contain it (FR11).
    #[test]
    fn transport_error_never_echoes_secret_in_flight() {
        use crate::domain::models::question::PasswordQuestion;
        let secret = "s3cr3t-token";
        let responder = Box::new(move |input: &TemplateAnswerInput| {
            if !input.answers.contains_key("token") {
                return Ok(qna(
                    Question::Password(PasswordQuestion {
                        message: "API token?".to_string(),
                        desc: None,
                        confirmation: None,
                        id: "token".to_string(),
                    }),
                    "token",
                ));
            }
            // The secret is now in flight — a transport error echoing it.
            Err(Box::new(std::io::Error::other(format!(
                "template crashed while applying token={secret}"
            ))) as Box<dyn Error + Send>)
        });
        let engine = TemplateEngine {
            client: Rc::new(FakeRepo { responder }),
        };
        let mut answers = HashMap::new();
        answers.insert("token".to_string(), Answer::String(secret.to_string()));
        match engine.start_headless(Some(answers)) {
            TemplateState::Err(msg) => {
                assert!(
                    !msg.contains(secret),
                    "transport error must never echo the secret value (FR11): {msg}"
                );
            }
            other => panic!("expected Err, got {}", state_variant_name(&other)),
        }
    }

    // (FR11) escaped-echo bypass: an in-flight secret containing a quote appears
    // JSON-escaped (`pa\"ss`) when a transport error embeds the request/response body as
    // JSON. The raw secret (`pa"ss`) is NOT a substring of the escaped text, so a naive
    // `contains` check would leak it; the encoding-aware `redact_error` catches the escaped
    // rendering and drops the message.
    #[test]
    fn transport_error_never_echoes_json_escaped_secret_in_flight() {
        use crate::domain::models::question::PasswordQuestion;
        let secret = "pa\"ss";
        let responder = Box::new(move |input: &TemplateAnswerInput| {
            if !input.answers.contains_key("token") {
                return Ok(qna(
                    Question::Password(PasswordQuestion {
                        message: "API token?".to_string(),
                        desc: None,
                        confirmation: None,
                        id: "token".to_string(),
                    }),
                    "token",
                ));
            }
            // The transport error embeds the secret the way serde_json would inside a JSON
            // string body — the quote is backslash-escaped.
            Err(Box::new(std::io::Error::other(
                r#"coordinator error: {"token":"pa\"ss"}"#.to_string(),
            )) as Box<dyn Error + Send>)
        });
        let engine = TemplateEngine {
            client: Rc::new(FakeRepo { responder }),
        };
        let mut answers = HashMap::new();
        answers.insert("token".to_string(), Answer::String(secret.to_string()));
        match engine.start_headless(Some(answers)) {
            TemplateState::Err(msg) => {
                assert!(
                    !msg.contains(r#"pa\"ss"#),
                    "the JSON-escaped secret must be redacted (FR11): {msg}"
                );
                assert!(
                    !msg.contains(secret),
                    "the raw secret must not appear either: {msg}"
                );
            }
            other => panic!("expected Err, got {}", state_variant_name(&other)),
        }
    }

    // (FR11) form-encoded-echo bypass (in-flight transport surface): an in-flight secret
    // containing a space appears form-encoded (`pa ss/word` → `pa+ss%2Fword`) when a transport
    // error embeds a request body encoded as `application/x-www-form-urlencoded` (space → `+`,
    // distinct from the RFC 3986 `%20`). The raw secret is NOT a substring of the encoded text,
    // so a naive `contains` check would leak it; the encoding-aware `redact_error` matches the
    // `+`-for-space variant and drops the message.
    #[test]
    fn transport_error_never_echoes_form_encoded_secret_in_flight() {
        use crate::domain::models::question::PasswordQuestion;
        let secret = "pa ss/word";
        let responder = Box::new(move |input: &TemplateAnswerInput| {
            if !input.answers.contains_key("token") {
                return Ok(qna(
                    Question::Password(PasswordQuestion {
                        message: "API token?".to_string(),
                        desc: None,
                        confirmation: None,
                        id: "token".to_string(),
                    }),
                    "token",
                ));
            }
            // Form-encoded echo of the secret: space → `+`, `/` → `%2F`.
            Err(Box::new(std::io::Error::other(
                "coordinator error: rejected token=pa+ss%2Fword".to_string(),
            )) as Box<dyn Error + Send>)
        });
        let engine = TemplateEngine {
            client: Rc::new(FakeRepo { responder }),
        };
        let mut answers = HashMap::new();
        answers.insert("token".to_string(), Answer::String(secret.to_string()));
        match engine.start_headless(Some(answers)) {
            TemplateState::Err(msg) => {
                assert!(
                    !msg.contains("pa+ss%2Fword"),
                    "the form-encoded secret must be redacted (FR11): {msg}"
                );
                assert!(
                    !msg.contains(secret),
                    "the raw secret must not appear either: {msg}"
                );
            }
            other => panic!("expected Err, got {}", state_variant_name(&other)),
        }
    }

    // FR11 lowercase-hex-echo bypass on the in-flight transport surface: a proxy/encoder that
    // echoes the secret with lowercase percent-encoding (`pa%40ss%2fword`) must be redacted too,
    // even though the canonical RFC 3986 rendering is uppercase (`%2F`).
    #[test]
    fn transport_error_never_echoes_lowercase_percent_encoded_secret_in_flight() {
        use crate::domain::models::question::PasswordQuestion;
        let secret = "pa@ss/word";
        let responder = Box::new(move |input: &TemplateAnswerInput| {
            if !input.answers.contains_key("token") {
                return Ok(qna(
                    Question::Password(PasswordQuestion {
                        message: "API token?".to_string(),
                        desc: None,
                        confirmation: None,
                        id: "token".to_string(),
                    }),
                    "token",
                ));
            }
            // Lowercase-hex percent-encoded echo of the secret: `@`→`%40`, `/`→`%2f`.
            Err(Box::new(std::io::Error::other(
                "coordinator error: rejected token=pa%40ss%2fword".to_string(),
            )) as Box<dyn Error + Send>)
        });
        let engine = TemplateEngine {
            client: Rc::new(FakeRepo { responder }),
        };
        let mut answers = HashMap::new();
        answers.insert("token".to_string(), Answer::String(secret.to_string()));
        match engine.start_headless(Some(answers)) {
            TemplateState::Err(msg) => {
                assert!(
                    !msg.contains("pa%40ss%2fword"),
                    "the lowercase-hex percent-encoded secret must be redacted (FR11): {msg}"
                );
                assert!(
                    !msg.contains(secret),
                    "the raw secret must not appear either: {msg}"
                );
            }
            other => panic!("expected Err, got {}", state_variant_name(&other)),
        }
    }

    // FR11 form-charset-echo bypass on the in-flight transport surface: form-urlencoding (e.g.
    // WHATWG `URLSearchParams` / Node) leaves `*` literal but encodes `~` → `%7E`, the opposite
    // of RFC 3986. A transport error echoing the secret `a*b~c` as `a*b%7Ec` would slip past the
    // RFC-3986-only charset; the form variant now catches it, proving the form-charset fix lands
    // on the in-flight surface as well as the validate-sibling one.
    #[test]
    fn transport_error_never_echoes_form_charset_secret_in_flight() {
        use crate::domain::models::question::PasswordQuestion;
        let secret = "a*b~c";
        let responder = Box::new(move |input: &TemplateAnswerInput| {
            if !input.answers.contains_key("token") {
                return Ok(qna(
                    Question::Password(PasswordQuestion {
                        message: "API token?".to_string(),
                        desc: None,
                        confirmation: None,
                        id: "token".to_string(),
                    }),
                    "token",
                ));
            }
            // Form-encoded echo of the secret: `*` stays literal, `~` → `%7E`.
            Err(Box::new(std::io::Error::other(
                "coordinator error: rejected token=a*b%7Ec".to_string(),
            )) as Box<dyn Error + Send>)
        });
        let engine = TemplateEngine {
            client: Rc::new(FakeRepo { responder }),
        };
        let mut answers = HashMap::new();
        answers.insert("token".to_string(), Answer::String(secret.to_string()));
        match engine.start_headless(Some(answers)) {
            TemplateState::Err(msg) => {
                assert!(
                    !msg.contains("a*b%7Ec"),
                    "the form-charset secret must be redacted (FR11): {msg}"
                );
                assert!(
                    !msg.contains(secret),
                    "the raw secret must not appear either: {msg}"
                );
            }
            other => panic!("expected Err, got {}", state_variant_name(&other)),
        }
    }

    // (FR11): the outcome walk's FIRST `prompt_template` call sends the full supplied
    // answer set, so a transport error it returns BEFORE any question is revealed (and
    // before the per-question `record` runs) must STILL be redacted against the supplied
    // secret. Eager validation passes (its first call carries an empty answer map and
    // reveals the question incrementally), then the outcome walk's first call — token
    // present but det-state empty — errors echoing the secret. With the up-front `seed_all`
    // the surfaced error must NOT contain the secret value; without it the error would leak.
    #[test]
    fn outcome_walk_first_call_error_redacts_supplied_secret() {
        use crate::domain::models::question::PasswordQuestion;
        let secret = "s3cr3t-token";
        let responder = Box::new(move |input: &TemplateAnswerInput| {
            // No answer yet → reveal the password question. This is hit on the eager
            // discovery walk's first call (empty `discovered` map), letting validation
            // record the secret and reach Final.
            if !input.answers.contains_key("token") {
                return Ok(qna(
                    Question::Password(PasswordQuestion {
                        message: "API token?".to_string(),
                        desc: None,
                        confirmation: None,
                        id: "token".to_string(),
                    }),
                    "token",
                ));
            }
            // Token present. The eager discovery walk reaches here with the recorded
            // det-state ("token":"seen") → finalize so validation passes. The OUTCOME
            // walk's first call reaches here with token present but EMPTY det-state
            // (it always starts from a fresh state) → error echoing the secret, BEFORE
            // the outcome walk has revealed the question / recorded the value.
            if input.deterministic_state.get("token").map(String::as_str) == Some("seen") {
                return Ok(final_output());
            }
            Err(Box::new(std::io::Error::other(format!(
                "template crashed while applying token={secret}"
            ))) as Box<dyn Error + Send>)
        });
        let engine = TemplateEngine {
            client: Rc::new(FakeRepo { responder }),
        };
        let mut answers = HashMap::new();
        answers.insert("token".to_string(), Answer::String(secret.to_string()));
        match engine.start_headless(Some(answers)) {
            TemplateState::Err(msg) => {
                assert!(
                    !msg.contains(secret),
                    "an early outcome-walk transport error must never echo the supplied \
                     secret (FR11): {msg}"
                );
            }
            other => panic!("expected Err, got {}", state_variant_name(&other)),
        }
    }

    // A `prompt_template` transport error that echoes a NON-secret supplied value is also
    // redacted (it may be echoing the value). The value must not appear in the envelope.
    #[test]
    fn transport_error_never_echoes_non_secret_in_flight() {
        let value = "mydb-name";
        let responder = Box::new(move |input: &TemplateAnswerInput| {
            if !input.answers.contains_key("name") {
                return Ok(qna(text("name"), "name"));
            }
            Err(Box::new(std::io::Error::other(format!(
                "failed applying name={value}"
            ))) as Box<dyn Error + Send>)
        });
        let engine = TemplateEngine {
            client: Rc::new(FakeRepo { responder }),
        };
        let mut answers = HashMap::new();
        answers.insert("name".to_string(), Answer::String(value.to_string()));
        match engine.start_headless(Some(answers)) {
            TemplateState::Err(msg) => {
                assert!(
                    !msg.contains(value),
                    "transport error must never echo an in-flight value (FR11): {msg}"
                );
            }
            other => panic!("expected Err, got {}", state_variant_name(&other)),
        }
    }

    // A transport error that does NOT echo any in-flight value keeps its message (useful,
    // non-leaking detail) — redaction only drops messages that actually echo a value.
    #[test]
    fn transport_error_keeps_non_echoing_message() {
        let responder = Box::new(|input: &TemplateAnswerInput| {
            if !input.answers.contains_key("name") {
                return Ok(qna(text("name"), "name"));
            }
            Err(Box::new(std::io::Error::other("coordinator unreachable")) as Box<dyn Error + Send>)
        });
        let engine = TemplateEngine {
            client: Rc::new(FakeRepo { responder }),
        };
        let mut answers = HashMap::new();
        answers.insert("name".to_string(), Answer::String("harmless".to_string()));
        match engine.start_headless(Some(answers)) {
            TemplateState::Err(msg) => {
                assert!(
                    msg.contains("coordinator unreachable"),
                    "a non-echoing transport error keeps its message: {msg}"
                );
            }
            other => panic!("expected Err, got {}", state_variant_name(&other)),
        }
    }

    // (FR11): a transport error echoing a Checkbox (StringArray) answer element is
    // redacted — non-String answer values are tracked too, not just `Answer::String`.
    // The echoed element ("billing") must not reach the error envelope.
    #[test]
    fn transport_error_never_echoes_checkbox_element() {
        let responder = Box::new(|input: &TemplateAnswerInput| {
            if !input.answers.contains_key("features") {
                return Ok(qna(checkbox("features", &["auth", "billing"]), "features"));
            }
            Err(
                Box::new(std::io::Error::other("failed applying features=billing"))
                    as Box<dyn Error + Send>,
            )
        });
        let engine = TemplateEngine {
            client: Rc::new(FakeRepo { responder }),
        };
        let mut answers = HashMap::new();
        answers.insert(
            "features".to_string(),
            Answer::StringArray(vec!["billing".to_string()]),
        );
        match engine.start_headless(Some(answers)) {
            TemplateState::Err(msg) => {
                assert!(
                    !msg.contains("billing"),
                    "transport error must never echo a Checkbox element (FR11): {msg}"
                );
            }
            other => panic!("expected Err, got {}", state_variant_name(&other)),
        }
    }

    // A transport error echoing ONLY a Confirm (Bool) answer keeps its message: a leaked
    // "true"/"false" reveals nothing private, so boolean renderings are intentionally NOT
    // tracked (tracking them would blank otherwise-useful transport errors, since those
    // substrings are extremely common). Contrast the Password/Checkbox cases above, where
    // the echoed value IS private and must be redacted. The contract still holds: a
    // secret string answer is still redacted even when a boolean is also present.
    #[test]
    fn transport_error_echoing_only_bool_keeps_message() {
        let responder = Box::new(|input: &TemplateAnswerInput| {
            if !input.answers.contains_key("use_db") {
                return Ok(qna(confirm("use_db"), "use_db"));
            }
            Err(
                Box::new(std::io::Error::other("applying use_db=True failed"))
                    as Box<dyn Error + Send>,
            )
        });
        let engine = TemplateEngine {
            client: Rc::new(FakeRepo { responder }),
        };
        let mut answers = HashMap::new();
        answers.insert("use_db".to_string(), Answer::Bool(true));
        match engine.start_headless(Some(answers)) {
            TemplateState::Err(msg) => {
                assert!(
                    msg.contains("applying use_db=True failed"),
                    "a transport error echoing only a non-secret boolean keeps its message: {msg}"
                );
            }
            other => panic!("expected Err, got {}", state_variant_name(&other)),
        }
    }

    // FR11 still holds when a boolean AND a secret coexist: the secret answer is redacted
    // even though the boolean is not tracked. This guards against a regression where
    // stopping boolean tracking could accidentally weaken secret redaction.
    #[test]
    fn transport_error_redacts_secret_even_with_bool_sibling() {
        let responder = Box::new(|input: &TemplateAnswerInput| {
            if !input.answers.contains_key("use_db") || !input.answers.contains_key("token") {
                if !input.answers.contains_key("use_db") {
                    return Ok(qna(confirm("use_db"), "use_db"));
                }
                return Ok(qna(password("token"), "token"));
            }
            Err(Box::new(std::io::Error::other(
                "applying use_db=True with token=s3cr3t failed",
            )) as Box<dyn Error + Send>)
        });
        let engine = TemplateEngine {
            client: Rc::new(FakeRepo { responder }),
        };
        let mut answers = HashMap::new();
        answers.insert("use_db".to_string(), Answer::Bool(true));
        answers.insert("token".to_string(), Answer::String("s3cr3t".to_string()));
        match engine.start_headless(Some(answers)) {
            TemplateState::Err(msg) => {
                assert!(
                    !msg.contains("s3cr3t"),
                    "a transport error must never echo a secret sibling (FR11): {msg}"
                );
            }
            other => panic!("expected Err, got {}", state_variant_name(&other)),
        }
    }

    // The driver re-derives deterministic state across the replay (FR6): a fake
    // that ONLY finalizes once it has observed the recorded det-state proves the
    // walk threads state internally without the caller supplying it.
    #[test]
    fn deterministic_state_is_rederived_during_replay() {
        // Sequence-model coordinator: it re-emits the (already-answered) question
        // `q1` carrying det-state until it observes that det-state echoed back —
        // only then does it finalize. This exercises the driver's "answer present
        // → thread det-state and continue" path. A driver that dropped det-state
        // would never satisfy the finalize condition.
        let responder = Box::new(|input: &TemplateAnswerInput| {
            if input.deterministic_state.get("q1").map(String::as_str) != Some("seen") {
                return Ok(qna(confirm("q1"), "q1"));
            }
            Ok(final_output())
        });
        let engine = TemplateEngine {
            client: Rc::new(FakeRepo { responder }),
        };
        let mut answers = HashMap::new();
        answers.insert("q1".to_string(), Answer::Bool(true));
        let state = engine.start_headless(Some(answers));
        assert!(matches!(state, TemplateState::Complete(_, _)));
    }

    // AC3 (FR9): an answer that FAILS the coordinator's validation is surfaced as
    // Err (the headless driver refuses to ingest an invalid answer, mirroring how
    // the interactive `inquire` validator rejects it). The error must reference the
    // question id and never echo the offending value.
    #[test]
    fn invalid_answer_is_rejected_as_err_state() {
        // Sequence-model coordinator: it re-emits the `text` question `name` until it
        // observes its det-state echoed back, then finalizes. This forces the driver
        // to VALIDATE the supplied answer on the first QnA (the value the interactive
        // `inquire` validator would send). A valid answer threads det-state and
        // completes; an invalid one is rejected before it is ingested.
        let responder = Box::new(|input: &TemplateAnswerInput| {
            if input.deterministic_state.get("name").map(String::as_str) != Some("seen") {
                return Ok(qna(text("name"), "name"));
            }
            Ok(final_output())
        });
        let engine = TemplateEngine {
            client: Rc::new(ValidatingFakeRepo {
                responder,
                reject_value: "bad".to_string(),
            }),
        };

        // A valid answer completes the walk.
        let mut good = HashMap::new();
        good.insert("name".to_string(), Answer::String("good".to_string()));
        assert!(matches!(
            engine.start_headless(Some(good)),
            TemplateState::Complete(_, _)
        ));

        // An invalid answer is rejected as Err, referencing the id, never the value.
        let mut bad = HashMap::new();
        bad.insert("name".to_string(), Answer::String("bad".to_string()));
        match engine.start_headless(Some(bad)) {
            TemplateState::Err(msg) => {
                assert!(msg.contains("name"), "error must reference the question id");
                // FR11: the offending ANSWER value ("bad") must never appear in the
                // message — only the coordinator's rejection reason (which is its own
                // wording, not the user's secret/value).
                assert!(
                    !msg.contains("bad"),
                    "error must never echo the offending answer value: {msg}"
                );
            }
            _ => panic!("expected Err state for a validation-failing answer"),
        }
    }

    // Iteration-cap safety bound: a misbehaving coordinator that re-emits an
    // already-answered question forever cannot spin the headless walk indefinitely —
    // it terminates as Err once the cap is hit (rather than hanging the automation).
    #[test]
    fn runaway_reemit_is_bounded() {
        let responder = Box::new(|_input: &TemplateAnswerInput| Ok(qna(text("loop"), "loop")));
        let engine = TemplateEngine {
            client: Rc::new(FakeRepo { responder }),
        };
        let mut answers = HashMap::new();
        answers.insert("loop".to_string(), Answer::String("v".to_string()));
        match engine.start_headless(Some(answers)) {
            TemplateState::Err(msg) => assert!(msg.contains("iterations")),
            _ => panic!("expected Err state from a runaway re-emit"),
        }
    }

    // (FR9): in the FIRST-UNANSWERED coordinator model the coordinator returns only
    // questions whose id is absent from `answers` and finalizes once every id is present.
    // It NEVER re-emits an already-answered question, so the in-walk validation branch
    // never runs. Without eager validation an invalid supplied answer would be silently
    // accepted as `done`. The eager pre-validation pass feeds answers incrementally so
    // each question is revealed and validated regardless of model — an invalid answer
    // surfaces as `Err` (exit 1), never `done` (exit 0).
    #[test]
    fn first_unanswered_model_validates_supplied_answers() {
        // First-unanswered coordinator: asks q1 then q2 (each only while absent),
        // finalizes once both are present. It never re-emits an answered question.
        let responder = Box::new(|input: &TemplateAnswerInput| {
            if !input.answers.contains_key("q1") {
                return Ok(qna(text("q1"), "q1"));
            }
            if !input.answers.contains_key("q2") {
                return Ok(qna(text("q2"), "q2"));
            }
            Ok(final_output())
        });

        // A VALID pair of answers completes the walk.
        let engine_ok = TemplateEngine {
            client: Rc::new(ValidatingFakeRepo {
                responder,
                reject_value: "forbidden".to_string(),
            }),
        };
        let mut good = HashMap::new();
        good.insert("q1".to_string(), Answer::String("ok1".to_string()));
        good.insert("q2".to_string(), Answer::String("ok2".to_string()));
        assert!(
            matches!(
                engine_ok.start_headless(Some(good)),
                TemplateState::Complete(_, _)
            ),
            "valid answers must complete even though the coordinator never re-emits them"
        );

        // An INVALID answer (q2 == "forbidden") is rejected as Err — proving eager
        // validation runs in the first-unanswered model.
        let responder2 = Box::new(|input: &TemplateAnswerInput| {
            if !input.answers.contains_key("q1") {
                return Ok(qna(text("q1"), "q1"));
            }
            if !input.answers.contains_key("q2") {
                return Ok(qna(text("q2"), "q2"));
            }
            Ok(final_output())
        });
        let engine_bad = TemplateEngine {
            client: Rc::new(ValidatingFakeRepo {
                responder: responder2,
                reject_value: "forbidden".to_string(),
            }),
        };
        let mut bad = HashMap::new();
        bad.insert("q1".to_string(), Answer::String("ok1".to_string()));
        bad.insert("q2".to_string(), Answer::String("forbidden".to_string()));
        match engine_bad.start_headless(Some(bad)) {
            TemplateState::Err(msg) => {
                assert!(
                    msg.contains("q2"),
                    "error must reference the rejected question id"
                );
                assert!(
                    !msg.contains("forbidden"),
                    "error must never echo the offending value (FR11): {msg}"
                );
            }
            other => panic!(
                "expected Err for an invalid answer in the first-unanswered model, got {}",
                state_variant_name(&other)
            ),
        }
    }

    // Regression: a first-unanswered coordinator still surfaces the first UNANSWERED
    // question as NeedInput when answers are incomplete — eager validation does not
    // change the emit-and-stop contract.
    #[test]
    fn first_unanswered_model_surfaces_need_input_when_incomplete() {
        let responder = Box::new(|input: &TemplateAnswerInput| {
            if !input.answers.contains_key("q1") {
                return Ok(qna(text("q1"), "q1"));
            }
            if !input.answers.contains_key("q2") {
                return Ok(qna(text("q2"), "q2"));
            }
            Ok(final_output())
        });
        let engine = TemplateEngine {
            client: Rc::new(FakeRepo { responder }),
        };
        // Supply only q1 → the coordinator asks q2 → NeedInput.
        let mut answers = HashMap::new();
        answers.insert("q1".to_string(), Answer::String("v".to_string()));
        let state = engine.start_headless(Some(answers));
        assert_eq!(need_input_id(&state).as_deref(), Some("q2"));
    }

    // A realistic coordinator optimization is to skip a question whose answer is
    // already present and finalize. Because the eager discovery walk feeds answers
    // INCREMENTALLY (empty map first), such a coordinator STILL reveals each question
    // when its answer is absent — so the supplied answer IS validated. An invalid answer
    // surfaces as Err (exit 1), never `done`. This is the case that matters in practice;
    // it is fully closed by the incremental discovery walk.
    #[test]
    fn finalize_on_present_coordinator_validates_supplied_answer() {
        // Coordinator: q1 present → Final; else ask q1.
        let make = || {
            Box::new(|input: &TemplateAnswerInput| {
                if input.answers.contains_key("q1") {
                    Ok(final_output())
                } else {
                    Ok(qna(text("q1"), "q1"))
                }
            })
                as Box<
                    dyn Fn(&TemplateAnswerInput) -> Result<TemplateOutput, Box<dyn Error + Send>>,
                >
        };

        // An INVALID supplied answer is rejected even though the coordinator would
        // finalize the moment q1 is present in the outcome walk.
        let engine_bad = TemplateEngine {
            client: Rc::new(ValidatingFakeRepo {
                responder: make(),
                reject_value: "bad".to_string(),
            }),
        };
        let mut bad = HashMap::new();
        bad.insert("q1".to_string(), Answer::String("bad".to_string()));
        match engine_bad.start_headless(Some(bad)) {
            TemplateState::Err(msg) => {
                assert!(msg.contains("q1"), "error must reference the question id");
                assert!(
                    !msg.contains("bad"),
                    "FR11: error must never echo the offending value: {msg}"
                );
            }
            other => panic!(
                "a skip-if-present coordinator must still validate the supplied answer, got {}",
                state_variant_name(&other)
            ),
        }

        // A VALID supplied answer completes.
        let engine_ok = TemplateEngine {
            client: Rc::new(ValidatingFakeRepo {
                responder: make(),
                reject_value: "bad".to_string(),
            }),
        };
        let mut good = HashMap::new();
        good.insert("q1".to_string(), Answer::String("good".to_string()));
        assert!(matches!(
            engine_ok.start_headless(Some(good)),
            TemplateState::Complete(_, _)
        ));
    }

    // A coordinator that finalizes on the FIRST call even with NO answers models a
    // template that asks ZERO questions. Interactively, `prompt_template` returns Final
    // immediately → the user is never prompted → the run completes. Headless mirrors
    // this: supplied answers reference no question the template asks, so they are simply
    // unused and the run COMPLETES. Making this `Err` would require rejecting every
    // unconsumed supplied answer, which would ALSO break the legitimate "pre-supply all
    // answers up front, including an untaken branch" use case below.
    #[test]
    fn zero_question_coordinator_completes_even_with_extra_answers() {
        let engine = TemplateEngine {
            client: Rc::new(FakeRepo {
                responder: Box::new(|_| Ok(final_output())),
            }),
        };
        let mut answers = HashMap::new();
        answers.insert("ghost".to_string(), Answer::String("x".to_string()));
        assert!(
            matches!(
                engine.start_headless(Some(answers)),
                TemplateState::Complete(_, _)
            ),
            "answers for a template that asks nothing are unused, not an error"
        );
    }

    // The counterexample that justifies NOT erroring on unconsumed answers: a caller
    // may legitimately pre-supply EVERY answer up front — including a branch answer that
    // turns out not to be reached. Here use_db=false is supplied, so the db_name branch
    // is never taken, yet db_name is also supplied. The walk must COMPLETE (db_name is
    // simply unused), exactly as the interactive path would (db_name is never prompted).
    #[test]
    fn presupplied_untaken_branch_answer_completes() {
        let engine = branching_engine();
        let mut answers = HashMap::new();
        answers.insert("use_db".to_string(), Answer::Bool(false));
        answers.insert("db_name".to_string(), Answer::String("unused".to_string()));
        assert!(
            matches!(
                engine.start_headless(Some(answers)),
                TemplateState::Complete(_, _)
            ),
            "a pre-supplied answer for an untaken branch must not block completion"
        );
    }

    // (FR11): a coordinator whose rejection message ECHOES the submitted value must NOT
    // leak that value into the error envelope. Here the validator says
    // "rejected: hunter2" (echoing the supplied password) — the headless error must
    // reference only the question id, never the value.
    #[test]
    fn password_validation_never_echoes_secret_in_error() {
        use crate::domain::models::question::PasswordQuestion;

        // Validator echoes the submitted value verbatim in its rejection message.
        struct EchoingValidator;
        impl CyanRepo for EchoingValidator {
            fn prompt_template(
                &self,
                input: TemplateAnswerInput,
            ) -> Result<TemplateOutput, Box<dyn Error + Send>> {
                // Sequence-model responder: re-emits the password question until its
                // det-state is echoed back, then finalizes — forcing the supplied
                // answer to be validated.
                if input.deterministic_state.get("pw").map(String::as_str) != Some("seen") {
                    return Ok(qna(
                        Question::Password(PasswordQuestion {
                            message: "Password?".to_string(),
                            desc: None,
                            confirmation: None,
                            id: "pw".to_string(),
                        }),
                        "pw",
                    ));
                }
                Ok(final_output())
            }
            fn validate_template(
                &self,
                input: crate::domain::models::template::input::TemplateValidateInput,
            ) -> Result<Option<String>, Box<dyn Error + 'static + Send + Sync>> {
                Ok(Some(format!("rejected: {}", input.validate)))
            }
        }

        let engine = TemplateEngine {
            client: Rc::new(EchoingValidator),
        };
        let mut answers = HashMap::new();
        answers.insert("pw".to_string(), Answer::String("hunter2".to_string()));
        match engine.start_headless(Some(answers)) {
            TemplateState::Err(msg) => {
                assert!(msg.contains("pw"), "error must reference the question id");
                assert!(
                    !msg.contains("hunter2"),
                    "password error must never echo the secret value (FR11): {msg}"
                );
            }
            other => panic!("expected Err, got {}", state_variant_name(&other)),
        }
    }

    // (FR11): for a non-secret (Text) question, a coordinator message that does NOT echo
    // the value is kept (useful wording), but one that DOES contain the value is dropped
    // to an id-only message.
    #[test]
    fn non_secret_validation_keeps_clean_message_drops_value_echo() {
        // Clean message (does not contain the value) is preserved.
        let clean = crate::domain::services::template::validate::validate_answer(
            &text("name"),
            &Answer::String("myval".to_string()),
            Rc::new(RejectingRepo(Some("must be lowercase".to_string()))),
            HashMap::new(),
            HashMap::new(),
        )
        .unwrap_err();
        assert!(clean.contains("name"));
        assert!(clean.contains("must be lowercase"));

        // Value-echoing message is dropped to id-only.
        let echoed = crate::domain::services::template::validate::validate_answer(
            &text("name"),
            &Answer::String("supersecret".to_string()),
            Rc::new(RejectingRepo(Some(
                "supersecret is not allowed".to_string(),
            ))),
            HashMap::new(),
            HashMap::new(),
        )
        .unwrap_err();
        assert!(echoed.contains("name"));
        assert!(
            !echoed.contains("supersecret"),
            "value-echoing message must be redacted: {echoed}"
        );
    }

    // Date validation: a malformed date that does not parse as %Y-%m-%d is now REJECTED
    // as Err — it is a shape the interactive date picker (which always yields a valid
    // NaiveDate) could never produce, so headless must not silently accept it. The error
    // references the question id only, never the offending value (FR11).
    #[test]
    fn malformed_date_is_rejected() {
        use crate::domain::models::question::DateQuestion;
        let q = Question::Date(DateQuestion {
            message: "When?".to_string(),
            desc: None,
            default: None,
            min_date: None,
            max_date: None,
            id: "when".to_string(),
        });
        // A non-date string is rejected BEFORE the coordinator is consulted (the repo
        // would reject everything if called, but the type/parse guard fires first).
        let res = crate::domain::services::template::validate::validate_answer(
            &q,
            &Answer::String("not-a-date".to_string()),
            Rc::new(RejectingRepo(Some("should-not-be-called".to_string()))),
            HashMap::new(),
            HashMap::new(),
        );
        match res {
            Err(msg) => {
                assert!(msg.contains("when"), "error must reference the question id");
                assert!(
                    !msg.contains("not-a-date"),
                    "error must never echo the offending value (FR11): {msg}"
                );
            }
            Ok(()) => panic!("a malformed date must be rejected, not accepted"),
        }

        // A well-formed date round-trips through the validator; an accepting repo (None)
        // → Ok, proving the well-formed value IS sent and accepted.
        let good = crate::domain::services::template::validate::validate_answer(
            &q,
            &Answer::String("2026-06-25".to_string()),
            Rc::new(RejectingRepo(None)),
            HashMap::new(),
            HashMap::new(),
        );
        assert!(good.is_ok(), "well-formed date validates cleanly");
    }

    // Type mismatch: a supplied answer whose `Answer` discriminant does not match the
    // question kind (e.g. a Text question answered with a Bool) is a shape the interactive
    // prompt could never produce. Headless must reject it as Err, not silently accept it.
    // The message references the id only (FR11).
    #[test]
    fn type_mismatched_answer_is_rejected() {
        // Text question answered with a Bool → reject (coordinator never consulted).
        let res = crate::domain::services::template::validate::validate_answer(
            &text("name"),
            &Answer::Bool(true),
            Rc::new(RejectingRepo(Some("should-not-be-called".to_string()))),
            HashMap::new(),
            HashMap::new(),
        );
        assert!(
            res.is_err(),
            "a Text question answered with Bool must be rejected (type mismatch)"
        );
        assert!(res.unwrap_err().contains("name"));

        // Confirm question answered with a Bool → type-aligned, accepted (no validator).
        let ok = crate::domain::services::template::validate::validate_answer(
            &confirm("flag"),
            &Answer::Bool(true),
            Rc::new(RejectingRepo(Some("unused".to_string()))),
            HashMap::new(),
            HashMap::new(),
        );
        assert!(ok.is_ok(), "a Confirm answered with Bool is type-aligned");
    }

    // (FR11): the redaction of a value-echoing coordinator message must NOT be gated on
    // value length. A short (≤2 char) value echoed verbatim is still a leak.
    #[test]
    fn short_value_is_redacted_from_error_message() {
        // Coordinator echoes the 2-char value "ab" in its rejection message. The headless
        // error must drop it to an id-only message rather than leaking "ab".
        let echoed = crate::domain::services::template::validate::validate_answer(
            &text("name"),
            &Answer::String("ab".to_string()),
            Rc::new(RejectingRepo(Some("ab is not allowed".to_string()))),
            HashMap::new(),
            HashMap::new(),
        )
        .unwrap_err();
        assert!(echoed.contains("name"), "error references the question id");
        assert!(
            !echoed.contains("ab is not allowed"),
            "a short echoed value must still be redacted: {echoed}"
        );
    }

    // A Select value that is NOT among the question's options is a shape the
    // interactive picker can never produce (the picker only ever yields one of the
    // listed options). Headless must reject it as Err — BEFORE the coordinator is
    // consulted — referencing the question id only (FR11). A member value is accepted.
    #[test]
    fn select_value_outside_options_is_rejected() {
        let q = Question::Select(SelectQuestion {
            message: "env?".to_string(),
            desc: None,
            options: vec!["dev".to_string(), "prod".to_string()],
            id: "env".to_string(),
        });

        // A non-member value is rejected (coordinator never consulted — the repo would
        // accept anything, proving the local guard fires).
        let res = crate::domain::services::template::validate::validate_answer(
            &q,
            &Answer::String("staging".to_string()),
            Rc::new(RejectingRepo(None)),
            HashMap::new(),
            HashMap::new(),
        );
        match res {
            Err(msg) => {
                assert!(msg.contains("env"), "error must reference the question id");
                assert!(
                    !msg.contains("staging"),
                    "error must never echo the offending value (FR11): {msg}"
                );
            }
            Ok(()) => panic!("a Select value outside options must be rejected"),
        }

        // A member value is accepted (the accepting repo returns None).
        let ok = crate::domain::services::template::validate::validate_answer(
            &q,
            &Answer::String("dev".to_string()),
            Rc::new(RejectingRepo(None)),
            HashMap::new(),
            HashMap::new(),
        );
        assert!(ok.is_ok(), "a Select value that is a member validates");
    }

    // A Checkbox value containing an element NOT among the question's options is
    // a shape the interactive MultiSelect can never produce. Headless must reject it as
    // Err. A subset of the options is accepted.
    #[test]
    fn checkbox_value_outside_options_is_rejected() {
        use crate::domain::models::question::CheckboxQuestion;
        let q = Question::Checkbox(CheckboxQuestion {
            message: "features?".to_string(),
            options: vec!["auth".to_string(), "billing".to_string()],
            desc: None,
            id: "features".to_string(),
        });

        // A non-member element ("root_shell") is rejected.
        let res = crate::domain::services::template::validate::validate_answer(
            &q,
            &Answer::StringArray(vec!["auth".to_string(), "root_shell".to_string()]),
            Rc::new(RejectingRepo(None)),
            HashMap::new(),
            HashMap::new(),
        );
        match res {
            Err(msg) => {
                assert!(
                    msg.contains("features"),
                    "error must reference the question id"
                );
                assert!(
                    !msg.contains("root_shell"),
                    "error must never echo the offending value (FR11): {msg}"
                );
            }
            Ok(()) => panic!("a Checkbox value outside options must be rejected"),
        }

        // A subset of the options is accepted.
        let ok = crate::domain::services::template::validate::validate_answer(
            &q,
            &Answer::StringArray(vec!["auth".to_string(), "billing".to_string()]),
            Rc::new(RejectingRepo(None)),
            HashMap::new(),
            HashMap::new(),
        );
        assert!(ok.is_ok(), "a Checkbox subset of options validates");
    }

    // A Date value outside the question's min_date/max_date range is a shape the
    // interactive date picker (which is clamped to min/max) can never produce. Headless
    // must reject values before min_date and after max_date as Err; a value in range is
    // accepted.
    #[test]
    fn date_value_outside_range_is_rejected() {
        use crate::domain::models::question::DateQuestion;
        let q = Question::Date(DateQuestion {
            message: "When?".to_string(),
            desc: None,
            default: None,
            min_date: Some("2026-01-01".to_string()),
            max_date: Some("2026-12-31".to_string()),
            id: "when".to_string(),
        });

        // Before min_date → rejected (coordinator never consulted).
        let too_early = crate::domain::services::template::validate::validate_answer(
            &q,
            &Answer::String("1900-01-01".to_string()),
            Rc::new(RejectingRepo(None)),
            HashMap::new(),
            HashMap::new(),
        );
        match too_early {
            Err(msg) => {
                assert!(msg.contains("when"), "error must reference the question id");
                assert!(
                    !msg.contains("1900-01-01"),
                    "error must never echo the offending value (FR11): {msg}"
                );
            }
            Ok(()) => panic!("a Date before min_date must be rejected"),
        }

        // After max_date → rejected.
        let too_late = crate::domain::services::template::validate::validate_answer(
            &q,
            &Answer::String("2099-01-01".to_string()),
            Rc::new(RejectingRepo(None)),
            HashMap::new(),
            HashMap::new(),
        );
        match too_late {
            Err(msg) => assert!(msg.contains("when")),
            Ok(()) => panic!("a Date after max_date must be rejected"),
        }

        // In range → accepted.
        let ok = crate::domain::services::template::validate::validate_answer(
            &q,
            &Answer::String("2026-06-25".to_string()),
            Rc::new(RejectingRepo(None)),
            HashMap::new(),
            HashMap::new(),
        );
        assert!(ok.is_ok(), "a Date in range validates");
    }

    // Integration through the REAL start_headless walk boundary: a Select
    // question offering [dev, prod] driven through the full eager-validation + outcome
    // walk — an out-of-options value ("staging") is rejected as Err (never `done`),
    // while a member value ("prod") completes. This drives the same `validate_answer`
    // → `validateable_value` path the four done paths exercise, but at the engine
    // boundary (not just the unit), closing the "tests that drive real boundaries"
    // gap prior loops flagged. Message is id-only (FR11).
    #[test]
    fn select_outside_options_is_rejected_through_headless_walk() {
        // Sequence-model coordinator: re-emits the `env` Select question until its
        // det-state is echoed back, then finalizes — forcing the supplied answer to be
        // validated on the first QnA.
        let responder = Box::new(|input: &TemplateAnswerInput| {
            if input.deterministic_state.get("env").map(String::as_str) != Some("seen") {
                return Ok(qna(select("env", &["dev", "prod"]), "env"));
            }
            Ok(final_output())
        });

        // Out-of-options value → Err, never `done`.
        let engine_bad = TemplateEngine {
            client: Rc::new(FakeRepo { responder }),
        };
        let mut bad = HashMap::new();
        bad.insert("env".to_string(), Answer::String("staging".to_string()));
        match engine_bad.start_headless(Some(bad)) {
            TemplateState::Err(msg) => {
                assert!(msg.contains("env"), "error must reference the question id");
                assert!(
                    !msg.contains("staging"),
                    "error must never echo the offending value (FR11): {msg}"
                );
            }
            other => panic!(
                "a Select value outside options must be rejected through the walk, got {}",
                state_variant_name(&other)
            ),
        }

        // Member value → completes (the FakeRepo's validate_template returns None = accept).
        // Build a fresh engine with an identical responder.
        let responder2 = Box::new(|input: &TemplateAnswerInput| {
            if input.deterministic_state.get("env").map(String::as_str) != Some("seen") {
                return Ok(qna(select("env", &["dev", "prod"]), "env"));
            }
            Ok(final_output())
        });
        let engine_ok = TemplateEngine {
            client: Rc::new(FakeRepo {
                responder: responder2,
            }),
        };
        let mut good = HashMap::new();
        good.insert("env".to_string(), Answer::String("prod".to_string()));
        assert!(
            matches!(
                engine_ok.start_headless(Some(good)),
                TemplateState::Complete(_, _)
            ),
            "a Select member value must complete the headless walk"
        );
    }

    // Stale-answer pruning: a stateless headless caller may supply answers for BOTH branches
    // of a gate — including a SECRET for the branch it did not take. The completed (persistable)
    // answer set must contain ONLY the answers for the branch the coordinator actually
    // revealed, never the stale over-supplied answer. Concretely: a gate `enable_db` (bool);
    // only if `enable_db=true` does the coordinator ask `db_password`. The caller supplies
    // `enable_db=false` AND a stale `db_password`. The walk completes (the false branch
    // finalizes without asking `db_password`), and the persisted answers must hold ONLY
    // `enable_db` — the stale `db_password` must be dropped, so a later run where the branch
    // flips to `enable_db=true` can never silently reuse it.
    #[test]
    fn completed_state_drops_over_supplied_secret_on_untaken_branch() {
        let stale_secret = "stale-secret-from-a-previous-run";
        let responder = Box::new(|input: &TemplateAnswerInput| {
            // Gate first.
            if !input.answers.contains_key("enable_db") {
                return Ok(qna(confirm("enable_db"), "enable_db"));
            }
            // Only the true branch reveals the password question.
            let enable_db = matches!(input.answers.get("enable_db"), Some(Answer::Bool(true)));
            if enable_db && !input.answers.contains_key("db_password") {
                return Ok(qna(password("db_password"), "db_password"));
            }
            // The false branch finalizes without ever asking about db_password.
            Ok(final_output())
        });
        let engine = TemplateEngine {
            client: Rc::new(FakeRepo { responder }),
        };

        // The caller over-supplies: enable_db=false (the taken branch) AND a stale
        // db_password (for the untaken branch).
        let mut answers = HashMap::new();
        answers.insert("enable_db".to_string(), Answer::Bool(false));
        answers.insert(
            "db_password".to_string(),
            Answer::String(stale_secret.to_string()),
        );
        let persisted = match engine.start_headless(Some(answers)) {
            TemplateState::Complete(_, persisted) => persisted,
            other => panic!("expected Complete, got {}", state_variant_name(&other)),
        };

        // The revealed-path answer is kept.
        assert!(
            matches!(persisted.get("enable_db"), Some(Answer::Bool(false))),
            "the revealed gate answer must be persisted"
        );
        // The over-supplied secret for the untaken branch is DROPPED — it was never
        // prompted, so it must not be carried into persisted state (and could not then be
        // silently reused when the branch later flips to true).
        assert!(
            !persisted.contains_key("db_password"),
            "a supplied answer for an untaken branch must NOT be persisted (stale secret would \
             be reused): {persisted:?}"
        );
    }

    // Over-supply pruning (general, both coordinator models): any over-supplied answer for a
    // question the coordinator never reveals is dropped from the persisted set, while the
    // revealed answers are kept. Covers the first-unanswered model (finalizes once every
    // revealed id is present, never re-emitting answered questions) AND the sequence model
    // (re-emits until det-state is echoed). In both, a `ghost` answer supplied for a question
    // the coordinator never asks must not survive into the completed state.
    #[test]
    fn completed_state_keeps_only_revealed_answers_drops_oversupply() {
        // First-unanswered coordinator: asks `name` only while absent, finalizes once present.
        let responder_fu = Box::new(|input: &TemplateAnswerInput| {
            if !input.answers.contains_key("name") {
                return Ok(qna(text("name"), "name"));
            }
            Ok(final_output())
        });
        let engine_fu = TemplateEngine {
            client: Rc::new(FakeRepo {
                responder: responder_fu,
            }),
        };
        let mut answers_fu = HashMap::new();
        answers_fu.insert("name".to_string(), Answer::String("real".to_string()));
        // Over-supply a ghost answer the coordinator never asks about.
        answers_fu.insert("ghost".to_string(), Answer::String("unused".to_string()));
        match engine_fu.start_headless(Some(answers_fu)) {
            TemplateState::Complete(_, persisted) => {
                assert_eq!(
                    persisted.len(),
                    1,
                    "only the revealed answer persists: {persisted:?}"
                );
                assert!(matches!(
                    persisted.get("name"),
                    Some(Answer::String(s)) if s == "real"
                ));
                assert!(
                    !persisted.contains_key("ghost"),
                    "an over-supplied answer for an unasked question must be dropped"
                );
            }
            other => panic!(
                "first-unanswered model must complete, got {}",
                state_variant_name(&other)
            ),
        }

        // Sequence coordinator: re-emits `name` until its det-state is echoed back, then
        // finalizes. Same over-supply dropped expectation.
        let responder_seq = Box::new(|input: &TemplateAnswerInput| {
            if input.deterministic_state.get("name").map(String::as_str) != Some("seen") {
                return Ok(qna(text("name"), "name"));
            }
            Ok(final_output())
        });
        let engine_seq = TemplateEngine {
            client: Rc::new(FakeRepo {
                responder: responder_seq,
            }),
        };
        let mut answers_seq = HashMap::new();
        answers_seq.insert("name".to_string(), Answer::String("real".to_string()));
        answers_seq.insert("ghost".to_string(), Answer::String("unused".to_string()));
        match engine_seq.start_headless(Some(answers_seq)) {
            TemplateState::Complete(_, persisted) => {
                assert!(
                    !persisted.contains_key("ghost"),
                    "oversupply must be dropped even in the sequence model: {persisted:?}"
                );
                assert!(persisted.contains_key("name"));
            }
            other => panic!(
                "sequence model must complete, got {}",
                state_variant_name(&other)
            ),
        }
    }

    // No over-prune: all revealed answers ARE persisted when nothing is over-supplied — the
    // pruning must not drop legitimately-answered questions on the taken path.
    // A two-question linear walk with both answers supplied must persist BOTH.
    #[test]
    fn completed_state_persists_all_answers_on_taken_path() {
        let responder = Box::new(|input: &TemplateAnswerInput| {
            if !input.answers.contains_key("q1") {
                return Ok(qna(text("q1"), "q1"));
            }
            if !input.answers.contains_key("q2") {
                return Ok(qna(text("q2"), "q2"));
            }
            Ok(final_output())
        });
        let engine = TemplateEngine {
            client: Rc::new(FakeRepo { responder }),
        };
        let mut answers = HashMap::new();
        answers.insert("q1".to_string(), Answer::String("a1".to_string()));
        answers.insert("q2".to_string(), Answer::String("a2".to_string()));
        let persisted = match engine.start_headless(Some(answers)) {
            TemplateState::Complete(_, persisted) => persisted,
            other => panic!("expected Complete, got {}", state_variant_name(&other)),
        };
        assert_eq!(
            persisted.len(),
            2,
            "both taken-path answers persist: {persisted:?}"
        );
        assert!(matches!(persisted.get("q1"), Some(Answer::String(s)) if s == "a1"));
        assert!(matches!(persisted.get("q2"), Some(Answer::String(s)) if s == "a2"));
    }

    /// A minimal `CyanRepo` whose `validate_template` always rejects with a fixed
    /// message, used to exercise message redaction. `prompt_template` is unused in these
    /// direct `validate_answer` tests.
    struct RejectingRepo(Option<String>);
    impl CyanRepo for RejectingRepo {
        fn prompt_template(
            &self,
            _input: TemplateAnswerInput,
        ) -> Result<TemplateOutput, Box<dyn Error + Send>> {
            Err(Box::new(std::io::Error::other("unused")) as Box<dyn Error + Send>)
        }
        fn validate_template(
            &self,
            _input: crate::domain::models::template::input::TemplateValidateInput,
        ) -> Result<Option<String>, Box<dyn Error + 'static + Send + Sync>> {
            Ok(self.0.clone())
        }
    }

    /// Name of a `TemplateState` variant, for readable panic messages without requiring
    /// `Debug` on `TemplateState` (which carries non-Debug domain types).
    fn state_variant_name(state: &TemplateState) -> &'static str {
        match state {
            TemplateState::QnA() => "QnA",
            TemplateState::Complete(_, _) => "Complete",
            TemplateState::NeedInput(_, _) => "NeedInput",
            TemplateState::Err(_) => "Err",
        }
    }
}
