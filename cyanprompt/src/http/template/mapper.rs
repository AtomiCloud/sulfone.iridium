use crate::domain::models::template::input::{TemplateAnswerInput, TemplateValidateInput};
use crate::domain::models::template::output::{TemplateFinalOutput, TemplateOutput, TemplateQnAOutput};
use crate::http::mapper::{ans_req_mapper, cyan_res_mapper, question_mapper};
use crate::http::template::req_model::{TemplateAnswerReq, TemplateValidateReq};
use crate::http::template::res_model::TemplateRes;

pub fn template_ans_input_mapper(r: &TemplateAnswerInput) -> TemplateAnswerReq {
    TemplateAnswerReq {
        answers: r.answers.iter().map(|x| ans_req_mapper(x)).collect(),
        deterministic_states: r.deterministic_states.clone(),
    }
}

pub fn template_validate_input_mapper(r: &TemplateValidateInput) -> TemplateValidateReq {
    TemplateValidateReq {
        answers: r.answers.iter().map(|x| ans_req_mapper(x)).collect(),
        deterministic_states: r.deterministic_states.clone(),
        validate: r.validate.clone(),
    }
}


pub fn template_ans_output_mapper(r: TemplateRes) -> TemplateOutput {
    match r {
        TemplateRes::Qna(qna) => TemplateOutput::QnA(
            TemplateQnAOutput {
                deterministic_state: qna.deterministic_state.clone(),
                question: question_mapper(&qna.question),
            }
        ),
        TemplateRes::Cyan(cyan) => TemplateOutput::Final(
            TemplateFinalOutput {
                cyan: cyan_res_mapper(cyan.cyan),
            }
        )
    }
}