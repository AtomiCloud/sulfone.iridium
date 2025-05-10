use reqwest::blocking::Response;
use std::error::Error;
use std::rc::Rc;

use serde::Serialize;

use crate::cli::mapper::{
    plugin_config_mapper, processor_config_mapper, read_yaml, template_config_mapper,
};
use crate::cli::models::plugin_config::CyanPluginFileConfig;
use crate::cli::models::processor_config::CyanProcessorFileConfig;
use crate::cli::models::template_config::CyanTemplateFileConfig;
use crate::http::errors::{GenericError, ProblemDetails};
use crate::http::mapper::{plugin_req_mapper, processor_req_mapper, template_req_mapper};
use crate::http::models::plugin_req::PluginReq;
use crate::http::models::plugin_res::{PluginVersionPrincipalRes, PluginVersionRes};
use crate::http::models::processor_req::ProcessorReq;
use crate::http::models::processor_res::{ProcessorVersionPrincipalRes, ProcessorVersionRes};
use crate::http::models::template_req::TemplateReq;
use crate::http::models::template_res::{TemplateVersionPrincipalRes, TemplateVersionRes};

pub struct CyanRegistryClient {
    pub endpoint: String,
    pub version: String,
    pub client: Rc<reqwest::blocking::Client>,
}

impl CyanRegistryClient {
    fn json_post<T: Serialize + ?Sized>(
        &self,
        endpoint: String,
        r: &T,
        token: Option<String>,
    ) -> Result<Response, reqwest::Error> {
        let mut req = self.client.post(endpoint).json(r);
        if token.is_some() {
            req = req.header("X-API-TOKEN", token.unwrap().as_str())
        }
        req.send()
    }

    fn push_processor_internal(
        &self,
        username: String,
        token: String,
        r: &ProcessorReq,
    ) -> Result<ProcessorVersionPrincipalRes, Box<dyn Error + Send>> {
        let host = (self.endpoint).to_string().to_owned();
        let version = (self.version).to_string().to_owned();
        let endpoint = host
            + "/api/v".to_string().as_str()
            + version.as_str()
            + "/Processor/push/".to_string().as_str()
            + username.as_str();

        self.json_post(endpoint, r, Some(token))
            .map_err(|x| Box::new(x) as Box<dyn Error + Send>)
            .and_then(|x| {
                if x.status().is_success() {
                    x.json().map_err(|e| Box::new(e) as Box<dyn Error + Send>)
                } else {
                    let r: Result<ProblemDetails, Box<dyn Error + Send>> =
                        x.json().map_err(|e| Box::new(e) as Box<dyn Error + Send>);
                    match r {
                        Ok(ok) => {
                            Err(Box::new(GenericError::ProblemDetails(ok)) as Box<dyn Error + Send>)
                        }
                        Err(err) => Err(err),
                    }
                }
            })
    }

    fn push_plugin_internal(
        &self,
        username: String,
        token: String,
        r: &PluginReq,
    ) -> Result<PluginVersionPrincipalRes, Box<dyn Error + Send>> {
        let host = (self.endpoint).to_string().to_owned();
        let version = (self.version).to_string().to_owned();
        let endpoint = host
            + "/api/v".to_string().as_str()
            + version.as_str()
            + "/Plugin/push/".to_string().as_str()
            + username.as_str();

        self.json_post(endpoint, r, Some(token))
            .map_err(|x| Box::new(x) as Box<dyn Error + Send>)
            .and_then(|x| {
                if x.status().is_success() {
                    x.json().map_err(|e| Box::new(e) as Box<dyn Error + Send>)
                } else {
                    let r: Result<ProblemDetails, Box<dyn Error + Send>> =
                        x.json().map_err(|e| Box::new(e) as Box<dyn Error + Send>);
                    match r {
                        Ok(ok) => {
                            Err(Box::new(GenericError::ProblemDetails(ok)) as Box<dyn Error + Send>)
                        }
                        Err(err) => Err(err),
                    }
                }
            })
    }

    fn push_template_internal(
        &self,
        username: String,
        token: String,
        r: &TemplateReq,
    ) -> Result<TemplateVersionPrincipalRes, Box<dyn Error + Send>> {
        let host = (self.endpoint).to_string().to_owned();
        let version = (self.version).to_string().to_owned();
        let endpoint = host
            + "/api/v".to_string().as_str()
            + version.as_str()
            + "/Template/push/".to_string().as_str()
            + username.as_str();

        self.json_post(endpoint, r, Some(token))
            .map_err(|x| Box::new(x) as Box<dyn Error + Send>)
            .and_then(|x| {
                if x.status().is_success() {
                    x.json().map_err(|e| Box::new(e) as Box<dyn Error + Send>)
                } else {
                    let r: Result<ProblemDetails, Box<dyn Error + Send>> =
                        x.json().map_err(|e| Box::new(e) as Box<dyn Error + Send>);
                    match r {
                        Ok(ok) => {
                            Err(Box::new(GenericError::ProblemDetails(ok)) as Box<dyn Error + Send>)
                        }
                        Err(err) => Err(err),
                    }
                }
            })
    }

    pub fn push_processor(
        &self,
        config_path: String,
        token: String,
        desc: String,
        docker_ref: String,
        docker_tag: String,
    ) -> Result<ProcessorVersionPrincipalRes, Box<dyn Error + Send>> {
        let a: Result<CyanProcessorFileConfig, Box<dyn Error + Send>> = read_yaml(config_path);
        let config = a?;
        let domain = processor_config_mapper(&config)?;
        let req = processor_req_mapper(&domain, desc, docker_ref, docker_tag);
        self.push_processor_internal(domain.username, token, &req)
    }

    pub fn push_plugin(
        &self,
        config_path: String,
        token: String,
        desc: String,
        docker_ref: String,
        docker_tag: String,
    ) -> Result<PluginVersionPrincipalRes, Box<dyn Error + Send>> {
        let a: Result<CyanPluginFileConfig, Box<dyn Error + Send>> = read_yaml(config_path);
        let config = a?;
        let domain = plugin_config_mapper(&config)?;
        let req = plugin_req_mapper(&domain, desc, docker_ref, docker_tag);
        self.push_plugin_internal(domain.username, token, &req)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn push_template(
        &self,
        config_path: String,
        token: String,
        desc: String,
        blob_docker_ref: String,
        blob_docker_tag: String,
        template_docker_ref: String,
        template_docker_tag: String,
    ) -> Result<TemplateVersionPrincipalRes, Box<dyn Error + Send>> {
        let a: Result<CyanTemplateFileConfig, Box<dyn Error + Send>> = read_yaml(config_path);
        let config = a?;
        let domain = template_config_mapper(&config)?;
        let req = template_req_mapper(
            &domain,
            desc,
            blob_docker_ref,
            blob_docker_tag,
            template_docker_ref,
            template_docker_tag,
        );
        self.push_template_internal(domain.username, token, &req)
    }

    pub fn get_template(
        &self,
        username: String,
        name: String,
        v: Option<i64>,
    ) -> Result<TemplateVersionRes, Box<dyn Error + Send>> {
        let host = (self.endpoint).to_string().to_owned();
        let version = (self.version).to_string().to_owned();

        let endpoint = match v {
            None => format!(
                "{}/api/v{}/Template/slug/{}/{}/versions/latest?bumpDownload=true",
                host, version, username, name
            ),
            Some(ver) => format!(
                "{}/api/v{}/Template/slug/{}/{}/versions/{}?bumpDownload=true",
                host, version, username, name, ver
            ),
        };
        self.client
            .get(endpoint)
            .send()
            .map_err(|x| Box::new(x) as Box<dyn Error + Send>)
            .and_then(|x| {
                if x.status().is_success() {
                    x.json().map_err(|e| Box::new(e) as Box<dyn Error + Send>)
                } else {
                    let r: Result<ProblemDetails, Box<dyn Error + Send>> =
                        x.json().map_err(|e| Box::new(e) as Box<dyn Error + Send>);
                    match r {
                        Ok(ok) => {
                            Err(Box::new(GenericError::ProblemDetails(ok)) as Box<dyn Error + Send>)
                        }
                        Err(err) => Err(err),
                    }
                }
            })
    }

    pub fn get_plugin(
        &self,
        username: String,
        name: String,
        v: Option<i64>,
    ) -> Result<PluginVersionRes, Box<dyn Error + Send>> {
        let host = (self.endpoint).to_string().to_owned();
        let version = (self.version).to_string().to_owned();

        let endpoint = match v {
            None => format!(
                "{}/api/v{}/Plugin/slug/{}/{}/versions/latest?bumpDownload=true",
                host, version, username, name
            ),
            Some(ver) => format!(
                "{}/api/v{}/Plugin/slug/{}/{}/versions/{}?bumpDownload=true",
                host, version, username, name, ver
            ),
        };

        println!("{}", endpoint);
        self.client
            .get(endpoint)
            .send()
            .map_err(|x| Box::new(x) as Box<dyn Error + Send>)
            .and_then(|x| {
                if x.status().is_success() {
                    x.json().map_err(|e| Box::new(e) as Box<dyn Error + Send>)
                } else {
                    let r: Result<ProblemDetails, Box<dyn Error + Send>> =
                        x.json().map_err(|e| Box::new(e) as Box<dyn Error + Send>);
                    match r {
                        Ok(ok) => {
                            Err(Box::new(GenericError::ProblemDetails(ok)) as Box<dyn Error + Send>)
                        }
                        Err(err) => Err(err),
                    }
                }
            })
    }

    pub fn get_processor(
        &self,
        username: String,
        name: String,
        v: Option<i64>,
    ) -> Result<ProcessorVersionRes, Box<dyn Error + Send>> {
        let host = (self.endpoint).to_string().to_owned();
        let version = (self.version).to_string().to_owned();

        let endpoint = match v {
            None => format!(
                "{}/api/v{}/Processor/slug/{}/{}/versions/latest?bumpDownload=true",
                host, version, username, name
            ),
            Some(ver) => format!(
                "{}/api/v{}/Processor/slug/{}/{}/versions/{}?bumpDownload=true",
                host, version, username, name, ver
            ),
        };
        self.client
            .get(endpoint)
            .send()
            .map_err(|x| Box::new(x) as Box<dyn Error + Send>)
            .and_then(|x| {
                if x.status().is_success() {
                    x.json().map_err(|e| Box::new(e) as Box<dyn Error + Send>)
                } else {
                    let r: Result<ProblemDetails, Box<dyn Error + Send>> =
                        x.json().map_err(|e| Box::new(e) as Box<dyn Error + Send>);
                    match r {
                        Ok(ok) => {
                            Err(Box::new(GenericError::ProblemDetails(ok)) as Box<dyn Error + Send>)
                        }
                        Err(err) => Err(err),
                    }
                }
            })
    }
}
