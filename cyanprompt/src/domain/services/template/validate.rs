use crate::domain::models::answer::Answer;
use crate::domain::models::prompt::Prompts;
use crate::domain::models::template::input::TemplateValidateInput;
use crate::domain::services::repo::CyanRepo;
use chrono::NaiveDate;
use inquire::validator::{ErrorMessage, Validation};
use inquire::CustomUserError;
use std::collections::HashMap;
use std::rc::Rc;

fn validate_template(
    result: &str,
    repo: Rc<dyn CyanRepo>,
    answers: Vec<Answer>,
    deterministic_states: Vec<HashMap<String, String>>,
) -> Result<Validation, CustomUserError> {
    let input = TemplateValidateInput {
        answers: answers.clone(),
        deterministic_states: deterministic_states.clone(),
        validate: result.to_string(),
    };
    repo.validate_template(input).map(|r| {
        r.map_or(Validation::Valid, |x| {
            Validation::Invalid(ErrorMessage::Custom(x))
        })
    })
}

pub fn add_template_validator(
    p: Prompts,
    repo: Rc<dyn CyanRepo>,
    answers: Vec<Answer>,
    deterministic_states: Vec<HashMap<String, String>>,
) -> Prompts {
    match p {
        Prompts::Text(text) => Prompts::Text(text.with_validator(move |v: &str| {
            validate_template(
                v,
                Rc::clone(&repo),
                answers.clone(),
                deterministic_states.clone(),
            )
        })),
        Prompts::Password(pw) => Prompts::Password(pw.with_validator(move |v: &str| {
            validate_template(
                v,
                Rc::clone(&repo),
                answers.clone(),
                deterministic_states.clone(),
            )
        })),
        Prompts::Date(d) => Prompts::Date(d.with_validator(move |v: NaiveDate| {
            validate_template(
                v.format("%Y-%m-%d").to_string().as_str(),
                Rc::clone(&repo),
                answers.clone(),
                deterministic_states.clone(),
            )
        })),
        default => default,
    }
}
