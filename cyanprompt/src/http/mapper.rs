use chrono::NaiveDate;
use inquire::{DateSelect, MultiSelect, PasswordDisplayMode};

use crate::domain::models::answer::Answer;
use crate::domain::models::cyan::{Cyan, CyanGlob, CyanPlugin, CyanProcessor, GlobType};
use crate::domain::models::prompt::Prompts;
use crate::domain::models::question::{
    CheckboxQuestion, ConfirmQuestion, DateQuestion, PasswordQuestion, Question, SelectQuestion,
    TextQuestion,
};
use crate::http::core::answer_req::{
    AnswerReq, BoolAnswerReq, StringAnswerReq, StringArrayAnswerReq,
};
use crate::http::core::answer_res::AnswerRes;
use crate::http::core::cyan_req::{CyanGlobReq, CyanPluginReq, CyanProcessorReq, CyanReq};
use crate::http::core::cyan_res::{CyanGlobRes, CyanPluginRes, CyanProcessorRes, CyanRes};
use crate::http::core::question_res::QuestionRes;

pub fn question_mapper(r: &QuestionRes) -> Question {
    match r {
        QuestionRes::Confirm(c) => Question::Confirm(ConfirmQuestion {
            message: c.message.clone(),
            desc: c.desc.clone(),
            default: c.default,
            error_message: c.error_message.clone(),
            id: c.id.clone(),
        }),
        QuestionRes::Date(date) => Question::Date(DateQuestion {
            message: date.message.clone(),
            desc: date.desc.clone(),
            default: date.default.clone(),
            min_date: date.min_date.clone(),
            max_date: date.max_date.clone(),
            id: date.id.clone(),
        }),
        QuestionRes::Checkbox(cb) => Question::Checkbox(CheckboxQuestion {
            message: cb.message.clone(),
            options: cb.options.clone(),
            desc: cb.desc.clone(),
            id: cb.id.clone(),
        }),
        QuestionRes::Password(pw) => Question::Password(PasswordQuestion {
            message: pw.message.clone(),
            desc: pw.desc.clone(),
            confirmation: pw.confirmation,
            id: pw.id.clone(),
        }),
        QuestionRes::Text(text) => Question::Text(TextQuestion {
            message: text.message.clone(),
            default: text.default.clone(),
            desc: text.desc.clone(),
            initial: text.initial.clone(),
            id: text.id.clone(),
        }),
        QuestionRes::Select(s) => Question::Select(SelectQuestion {
            message: s.message.clone(),
            desc: s.desc.clone(),
            options: s.options.clone(),
            id: s.id.clone(),
        }),
    }
}

pub fn ans_res_mapper(r: &AnswerRes) -> Answer {
    match r {
        AnswerRes::StringArray(sa) => Answer::StringArray(sa.answer.clone()),
        AnswerRes::String(s) => Answer::String(s.answer.clone()),
        AnswerRes::Bool(b) => Answer::Bool(b.answer),
    }
}

pub fn ans_req_mapper(a: &Answer) -> AnswerReq {
    match a {
        Answer::String(s) => AnswerReq::String(StringAnswerReq { answer: s.clone() }),
        Answer::StringArray(sa) => {
            AnswerReq::StringArray(StringArrayAnswerReq { answer: sa.clone() })
        }
        Answer::Bool(b) => AnswerReq::Bool(BoolAnswerReq { answer: b.clone() }),
    }
}

pub fn prompt_mapper<'a>(
    q: &'a Question,
) -> Result<Prompts<'a>, Box<dyn std::error::Error + Send>> {
    match q {
        Question::Confirm(c) => Ok(inquire::Confirm::new(&c.message))
            .map(|p| c.default.map_or(p.clone(), |def| p.with_default(def)))
            .map(|p| {
                c.desc
                    .as_ref()
                    .map_or(p.clone(), |help| p.with_help_message(help))
            })
            .map(|p| {
                c.error_message
                    .as_ref()
                    .map_or(p.clone(), |err_msg| p.with_error_message(err_msg))
            })
            .map(|p| Prompts::Confirm(p)),
        Question::Date(date) => Ok(DateSelect::new(&date.message))
            .map(|p| {
                date.desc
                    .as_ref()
                    .map_or(p.clone(), |desc| p.with_help_message(desc))
            })
            .and_then(|p| {
                date.default.as_ref().map_or(Ok(p.clone()), |s| {
                    NaiveDate::parse_from_str(s, "%Y-%m-%d")
                        .map(|e| p.with_default(e))
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)
                })
            })
            .and_then(|p| {
                date.min_date.as_ref().map_or(Ok(p.clone()), |s| {
                    NaiveDate::parse_from_str(s, "%Y-%m-%d")
                        .map(|e| p.with_min_date(e))
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)
                })
            })
            .and_then(|p| {
                date.max_date.as_ref().map_or(Ok(p.clone()), |s| {
                    NaiveDate::parse_from_str(s, "%Y-%m-%d")
                        .map(|e| p.with_max_date(e))
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)
                })
            })
            .map(|p| Prompts::Date(p)),
        Question::Checkbox(cb) => Ok(MultiSelect::new(&cb.message, cb.options.clone()))
            .map(|p| {
                cb.desc
                    .as_ref()
                    .map_or(p.clone(), |desc| p.with_help_message(desc))
            })
            .map(|p| Prompts::Checkbox(p)),
        Question::Password(pw) => {
            Ok(inquire::Password::new(&pw.message).with_display_mode(PasswordDisplayMode::Masked))
                .map(|p| {
                    pw.desc
                        .as_ref()
                        .map_or(p.clone(), |desc| p.with_help_message(desc))
                })
                .map(|p| {
                    pw.confirmation.map_or(p.clone(), |confirm| {
                        if confirm {
                            p.clone()
                        } else {
                            p.without_confirmation()
                        }
                    })
                })
                .map(|p| Prompts::Password(p))
        }
        Question::Text(text) => Ok(inquire::Text::new(&text.message))
            .map(|p| {
                text.desc
                    .as_ref()
                    .map_or(p.clone(), |desc| p.with_help_message(desc))
            })
            .map(|p| {
                text.default
                    .as_ref()
                    .map_or(p.clone(), |def| p.with_default(def))
            })
            .map(|p| {
                text.initial
                    .as_ref()
                    .map_or(p.clone(), |init| p.with_initial_value(init))
            })
            .map(|p| Prompts::Text(p)),
        Question::Select(s) => Ok(inquire::Select::new(&s.message, s.options.clone()))
            .map(|p| {
                s.desc
                    .as_ref()
                    .map_or(p.clone(), |desc| p.with_help_message(desc))
            })
            .map(|p| Prompts::Select(p)),
    }
}

pub fn processor_res_mapper(r: CyanProcessorRes) -> CyanProcessor {
    CyanProcessor {
        name: r.name,
        config: r.config,
        files: r.files.iter().map(|x| glob_res_mapper(x.clone())).collect(),
    }
}

pub fn plugin_res_mapper(r: CyanPluginRes) -> CyanPlugin {
    CyanPlugin {
        name: r.name,
        config: r.config,
    }
}

pub fn glob_res_mapper(r: CyanGlobRes) -> CyanGlob {
    CyanGlob {
        root: r.root,
        glob: r.glob,
        glob_type: glob_type_res_mapper(r.glob_type.as_str()),
        exclude: r.exclude,
    }
}

pub fn glob_type_res_mapper(r: &str) -> GlobType {
    match r {
        "template" => GlobType::Template(),
        "copy" => GlobType::Copy(),
        _ => panic!("unknown glob type"),
    }
}

pub fn cyan_res_mapper(r: CyanRes) -> Cyan {
    Cyan {
        processors: r
            .processors
            .iter()
            .map(|x| processor_res_mapper(x.clone()))
            .collect(),
        plugins: r
            .plugins
            .iter()
            .map(|x| plugin_res_mapper(x.clone()))
            .collect(),
    }
}

pub fn processor_req_mapper(r: CyanProcessor) -> CyanProcessorReq {
    CyanProcessorReq {
        name: r.name,
        config: r.config,
        files: r.files.iter().map(|x| glob_req_mapper(x.clone())).collect(),
    }
}

pub fn plugin_req_mapper(r: CyanPlugin) -> CyanPluginReq {
    CyanPluginReq {
        name: r.name,
        config: r.config,
    }
}

pub fn glob_req_mapper(r: CyanGlob) -> CyanGlobReq {
    CyanGlobReq {
        root: r.root,
        glob: r.glob,
        glob_type: glob_type_req_mapper(r.glob_type),
        exclude: r.exclude,
    }
}

pub fn glob_type_req_mapper(r: GlobType) -> String {
    match r {
        GlobType::Template() => "template".to_string(),
        GlobType::Copy() => "copy".to_string(),
    }
}

pub fn cyan_req_mapper(r: Cyan) -> CyanReq {
    CyanReq {
        processors: r
            .processors
            .iter()
            .map(|x| processor_req_mapper(x.clone()))
            .collect(),
        plugins: r
            .plugins
            .iter()
            .map(|x| plugin_req_mapper(x.clone()))
            .collect(),
    }
}
