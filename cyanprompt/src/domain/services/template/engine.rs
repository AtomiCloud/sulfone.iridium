use std::collections::HashMap;
use std::process::exit;
use std::rc::Rc;

use crate::domain::models::answer::Answer;
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
        TemplateEngine {
            client,
        }
    }

    pub fn start(&self) -> TemplateState {
        println!("TemplateEngine started");
        let mut state = TemplateState::QnA();

        // Track answer
        let mut answers: Vec<Answer> = vec![];
        let mut states: Vec<HashMap<String, String>> = vec![HashMap::new()];

        while state.cont() {
            let input = TemplateAnswerInput {
                answers: answers.clone(),
                deterministic_states: states.clone(),
            };
            let result = self.client.prompt_template(input)
                .and_then(|resp| match resp {
                    TemplateOutput::QnA(q) => {
                        let ans = prompt_mapper(&q.question)
                            .map(|p| add_template_validator(p, Rc::clone(&self.client), answers.clone(), states.clone()))
                            .and_then(|p| prompt(p))
                            // handle responses
                            .map(|x| match x {
                                // if skipped
                                None => {
                                    if answers.is_empty() {
                                        println!("User aborted! Exiting...");
                                        exit(0)
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
                            Ok(_) => Ok(TemplateState::QnA()),
                            Err(err) => Err(err),
                        }
                    }
                    TemplateOutput::Final(c) => Ok(TemplateState::Complete(c.cyan, answers.clone())),
                });

            state = match result {
                Ok(ok) => ok,
                Err(err) => TemplateState::Err(err.to_string()),
            };
        }
        state
    }
}