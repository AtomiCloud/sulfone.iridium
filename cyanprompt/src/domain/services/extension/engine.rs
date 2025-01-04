use std::collections::HashMap;
use std::rc::Rc;

use crate::domain::models::answer::Answer;
use crate::domain::models::cyan::Cyan;
use crate::domain::models::extension::input::ExtensionAnswerInput;
use crate::domain::models::extension::output::ExtensionOutput;
use crate::domain::services::extension::states::ExtensionState;
use crate::domain::services::extension::validate::add_extension_validator;
use crate::domain::services::prompter::prompt;
use crate::domain::services::repo::CyanRepo;
use crate::http::mapper::prompt_mapper;

pub struct ExtensionEngine {
    pub client: Rc<dyn CyanRepo>,
}

impl ExtensionEngine {
    pub fn new(client: Rc<dyn CyanRepo>) -> ExtensionEngine {
        ExtensionEngine { client }
    }

    pub fn start(&self, prev: Cyan, prev_answers: Vec<Answer>) -> ExtensionState {
        println!("ExtensionEngine started");
        let mut state = ExtensionState::QnA();

        // Track answer
        let mut answers: Vec<Answer> = vec![];
        let mut states: Vec<HashMap<String, String>> = vec![HashMap::new()];

        while state.cont() {
            let input = ExtensionAnswerInput {
                answers: answers.clone(),
                deterministic_states: states.clone(),
                prev_answers: prev_answers.clone(),
                prev: prev.clone(),
            };
            let result = self
                .client
                .prompt_extension(input)
                .and_then(|resp| match resp {
                    ExtensionOutput::QnA(q) => {
                        let ans = prompt_mapper(&q.question)
                            .map(|p| {
                                add_extension_validator(
                                    p,
                                    Rc::clone(&self.client),
                                    answers.clone(),
                                    states.clone(),
                                    prev_answers.clone(),
                                    prev.clone(),
                                )
                            })
                            .and_then(|p| prompt(p))
                            // handle responses
                            .map(|x| match x {
                                // if skipped
                                None => {
                                    if answers.is_empty() {
                                        println!("Cannot go back!");
                                    } else {
                                        answers.pop();
                                        states.pop();
                                    }
                                }
                                Some(val) => {
                                    answers.push(val);
                                    states = q.deterministic_state;
                                    states.push(HashMap::new());
                                }
                            });
                        match ans {
                            Ok(_) => Ok(ExtensionState::QnA()),
                            Err(err) => Err(err),
                        }
                    }
                    ExtensionOutput::Final(c) => Ok(ExtensionState::Complete(c.cyan)),
                });

            state = match result {
                Ok(ok) => ok,
                Err(err) => ExtensionState::Err(err.to_string()),
            };
        }
        state
    }
}
