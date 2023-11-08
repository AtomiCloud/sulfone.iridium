use crate::domain::models::extension::input::{ExtensionAnswerInput, ExtensionValidateInput};
use crate::domain::models::extension::output::{ExtensionFinalOutput, ExtensionOutput, ExtensionQnAOutput};
use crate::http::extension::req_model::{ExtensionAnswerReq, ExtensionValidateReq};
use crate::http::extension::res_model::ExtensionRes;
use crate::http::mapper::{ans_req_mapper, cyan_req_mapper, cyan_res_mapper, question_mapper};

pub fn extension_ans_input_mapper(r: &ExtensionAnswerInput) -> ExtensionAnswerReq {
    ExtensionAnswerReq {
        answers: r.answers.iter().map(|x| ans_req_mapper(x)).collect(),
        deterministic_states: r.deterministic_states.clone(),
        prev_answers: r.prev_answers.iter().map(|x| ans_req_mapper(x)).collect(),
        prev_cyan: cyan_req_mapper(r.prev.clone()),
    }
}

pub fn extension_validate_input_mapper(r: &ExtensionValidateInput) -> ExtensionValidateReq {
    ExtensionValidateReq {
        answers: r.answers.iter().map(|x| ans_req_mapper(x)).collect(),
        deterministic_states: r.deterministic_states.clone(),
        prev_answers: r.prev_answers.iter().map(|x| ans_req_mapper(x)).collect(),
        prev_cyan: cyan_req_mapper(r.prev.clone()),
        validate: r.validate.clone(),
    }
}


pub fn extension_ans_output_mapper(r: ExtensionRes) -> ExtensionOutput {
    match r {
        ExtensionRes::Qna(qna) => ExtensionOutput::QnA(
            ExtensionQnAOutput {
                deterministic_state: qna.deterministic_state.clone(),
                question: question_mapper(&qna.question),
            }
        ),
        ExtensionRes::Cyan(cyan) => ExtensionOutput::Final(
            ExtensionFinalOutput {
                cyan: cyan_res_mapper(cyan.cyan),
            }
        )
    }
}