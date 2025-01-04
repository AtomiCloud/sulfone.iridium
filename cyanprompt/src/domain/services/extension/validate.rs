use crate::domain::models::answer::Answer;
use crate::domain::models::cyan::Cyan;
use crate::domain::models::extension::input::ExtensionValidateInput;
use crate::domain::models::prompt::Prompts;
use crate::domain::services::repo::CyanRepo;
use chrono::NaiveDate;
use inquire::validator::{ErrorMessage, Validation};
use inquire::CustomUserError;
use std::collections::HashMap;
use std::rc::Rc;

fn validate_extension(
    result: &str,
    repo: Rc<dyn CyanRepo>,
    answers: Vec<Answer>,
    deterministic_states: Vec<HashMap<String, String>>,
    prev_answers: Vec<Answer>,
    prev: Cyan,
) -> Result<Validation, CustomUserError> {
    let input = ExtensionValidateInput {
        answers: answers.clone(),
        deterministic_states: deterministic_states.clone(),
        prev_answers,
        prev,
        validate: result.to_string(),
    };
    repo.validate_extension(input).map(|r| {
        r.map_or(Validation::Valid, |x| {
            Validation::Invalid(ErrorMessage::Custom(x))
        })
    })
}

pub fn add_extension_validator(
    p: Prompts,
    repo: Rc<dyn CyanRepo>,
    answers: Vec<Answer>,
    deterministic_states: Vec<HashMap<String, String>>,
    prev_answers: Vec<Answer>,
    prev: Cyan,
) -> Prompts {
    match p {
        Prompts::Text(text) => Prompts::Text(text.with_validator(move |v: &str| {
            validate_extension(
                v,
                Rc::clone(&repo),
                answers.clone(),
                deterministic_states.clone(),
                prev_answers.clone(),
                prev.clone(),
            )
        })),
        Prompts::Password(pw) => Prompts::Password(pw.with_validator(move |v: &str| {
            validate_extension(
                v,
                Rc::clone(&repo),
                answers.clone(),
                deterministic_states.clone(),
                prev_answers.clone(),
                prev.clone(),
            )
        })),
        Prompts::Date(d) => Prompts::Date(d.with_validator(move |v: NaiveDate| {
            validate_extension(
                v.format("%Y-%m-%d").to_string().as_str(),
                Rc::clone(&repo),
                answers.clone(),
                deterministic_states.clone(),
                prev_answers.clone(),
                prev.clone(),
            )
        })),
        default => default,
    }
}
