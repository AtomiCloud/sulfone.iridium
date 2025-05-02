use std::error::Error;

use crate::domain::models::template::input::{TemplateAnswerInput, TemplateValidateInput};
use crate::domain::models::template::output::TemplateOutput;
use crate::http::client::CyanClient;
use crate::http::template::mapper::{
    template_ans_input_mapper, template_ans_output_mapper, template_validate_input_mapper,
};

pub trait CyanRepo {
    fn prompt_template(
        &self,
        input: TemplateAnswerInput,
    ) -> Result<TemplateOutput, Box<dyn Error + Send>>;

    fn validate_template(
        &self,
        input: TemplateValidateInput,
    ) -> Result<Option<String>, Box<dyn Error + 'static + Send + Sync>>;
}

pub struct CyanHttpRepo {
    pub client: CyanClient,
}

impl CyanRepo for CyanHttpRepo {
    fn prompt_template(
        &self,
        input: TemplateAnswerInput,
    ) -> Result<TemplateOutput, Box<dyn Error + Send>> {
        let x = template_ans_input_mapper(&input);
        self.client
            .prompt_template(&x)
            .map(|r| template_ans_output_mapper(r))
    }

    fn validate_template(
        &self,
        input: TemplateValidateInput,
    ) -> Result<Option<String>, Box<dyn Error + 'static + Send + Sync>> {
        let req = template_validate_input_mapper(&input);
        self.client.validate_template(&req).map(|r| r.valid)
    }
}
