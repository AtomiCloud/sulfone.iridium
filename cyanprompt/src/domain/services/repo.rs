use std::error::Error;
use crate::domain::models::extension::input::{ExtensionAnswerInput, ExtensionValidateInput};
use crate::domain::models::extension::output::ExtensionOutput;

use crate::domain::models::template::input::{TemplateAnswerInput, TemplateValidateInput};
use crate::domain::models::template::output::TemplateOutput;
use crate::http::client::CyanClient;
use crate::http::extension::mapper::{extension_ans_input_mapper, extension_ans_output_mapper, extension_validate_input_mapper};
use crate::http::template::mapper::{template_ans_input_mapper, template_ans_output_mapper, template_validate_input_mapper};

pub trait CyanRepo {
    fn prompt_template(&self, input: TemplateAnswerInput)
                       -> Result<TemplateOutput, Box<dyn Error + Send>>;

    fn validate_template(&self, input: TemplateValidateInput)
                         -> Result<Option<String>, Box<dyn Error + 'static + Send + Sync>>;

    fn prompt_extension(&self, input: ExtensionAnswerInput)
                        -> Result<ExtensionOutput, Box<dyn Error + Send>>;

    fn validate_extension(&self, input: ExtensionValidateInput)
                          -> Result<Option<String>, Box<dyn Error + 'static + Send + Sync>>;
}

pub struct CyanHttpRepo {
    pub client: CyanClient,
}

impl CyanRepo for CyanHttpRepo {
    fn prompt_template(&self, input: TemplateAnswerInput) -> Result<TemplateOutput, Box<dyn Error + Send>> {
        let x = template_ans_input_mapper(&input);
        self.client.prompt_template(&x)
            .map(|r| template_ans_output_mapper(r))
    }

    fn validate_template(&self, input: TemplateValidateInput) -> Result<Option<String>, Box<dyn Error + 'static + Send + Sync>> {
        let req = template_validate_input_mapper(&input);
        self.client.validate_template(&req)
            .map(|r| r.valid)
    }

    fn prompt_extension(&self, input: ExtensionAnswerInput) -> Result<ExtensionOutput, Box<dyn Error + Send>> {
        let req = extension_ans_input_mapper(&input);
        self.client.prompt_extension(&req)
            .map(|r| extension_ans_output_mapper(r))
    }

    fn validate_extension(&self, input: ExtensionValidateInput) -> Result<Option<String>, Box<dyn Error + 'static + Send + Sync>> {
        let req = extension_validate_input_mapper(&input);
        self.client.validate_extension(&req)
            .map(|r| r.valid)
    }
}


