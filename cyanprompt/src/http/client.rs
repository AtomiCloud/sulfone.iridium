use std::error::Error;
use std::rc::Rc;

use reqwest::blocking::Response;
use serde::Serialize;

use crate::http::template::req_model::{TemplateAnswerReq, TemplateValidateReq};
use crate::http::template::res_model::{TemplateRes, TemplateValidRes};

pub struct CyanClient {
    pub endpoint: String,
    pub client: Rc<reqwest::blocking::Client>,
}

impl CyanClient {
    fn json_post<T: Serialize + ?Sized>(
        &self,
        endpoint: String,
        r: &T,
    ) -> Result<Response, reqwest::Error> {
        self.client.post(endpoint).json(r).send()
    }

    pub fn prompt_template(
        &self,
        r: &TemplateAnswerReq,
    ) -> Result<TemplateRes, Box<dyn Error + Send>> {
        let host = (&self.endpoint).to_string().to_owned();
        let endpoint = host + "/api/template/init".to_string().as_str();
        self.json_post(endpoint, r)
            .map_err(|x| Box::new(x) as Box<dyn Error + Send>)
            .and_then(|resp| {
                resp.json()
                    .map_err(|e| Box::new(e) as Box<dyn Error + Send>)
            })
    }

    pub fn validate_template(
        &self,
        r: &TemplateValidateReq,
    ) -> Result<TemplateValidRes, Box<dyn Error + 'static + Send + Sync>> {
        let host = (&self.endpoint).to_string().to_owned();
        let endpoint = host + "/api/template/validate".to_string().as_str();

        self.json_post(endpoint, r)
            .map_err(|x| x.into())
            .and_then(|resp| resp.json().map_err(|e| e.into()))
    }
}
