use crate::domain::models::answer::Answer;
use crate::domain::models::prompt::Prompts;
use crate::domain::models::template::input::TemplateValidateInput;
use crate::domain::services::repo::CyanRepo;
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
