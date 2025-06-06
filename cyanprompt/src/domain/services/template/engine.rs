use std::collections::HashMap;
use std::process::exit;
use std::rc::Rc;

use crate::domain::models::answer::Answer;
use crate::domain::models::question::QuestionTrait;
use crate::domain::models::template::input::TemplateAnswerInput;
use crate::domain::models::template::output::TemplateOutput;
use crate::domain::services::prompter::prompt;
use crate::domain::services::repo::CyanRepo;
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
}
