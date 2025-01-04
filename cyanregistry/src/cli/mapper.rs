use crate::cli::models::plugin_config::CyanPluginFileConfig;
use serde::de::DeserializeOwned;
use std::error::Error;
use std::{fmt, fs};

use crate::cli::models::processor_config::CyanProcessorFileConfig;
use crate::cli::models::template_config::CyanTemplateFileConfig;
use crate::domain::config::plugin_config::CyanPluginConfig;
use crate::domain::config::processor_config::CyanProcessorConfig;
use crate::domain::config::template_config::{CyanPluginRef, CyanProcessorRef, CyanTemplateConfig};

pub fn processor_reference_mapper(s: String) -> Option<CyanProcessorRef> {
    let mut parts = s.splitn(2, '/');
    let username = parts.next()?.to_string();
    let rest = parts.next();

    // Split the rest by ':'
    let mut parts = rest?.splitn(2, ':');
    let name = parts.next()?.to_string();
    let version_str = parts.next();

    // Convert version string to u64 if present
    let version = match version_str {
        Some(v) => v.parse::<i64>().ok(),
        None => None,
    };

    Some(CyanProcessorRef {
        username,
        name,
        version,
    })
}

pub fn plugin_reference_mapper(s: String) -> Option<CyanPluginRef> {
    let mut parts = s.splitn(2, '/');
    let username = parts.next()?.to_string();
    let rest = parts.next()?;

    // Split the rest by ':'
    let mut parts = rest.splitn(2, ':');
    let name = parts.next()?.to_string();
    let version_str = parts.next();

    // Convert version string to u64 if present
    let version = match version_str {
        Some(v) => v.parse::<i64>().ok(),
        None => None,
    };

    Some(CyanPluginRef {
        username,
        name,
        version,
    })
}

#[derive(Debug)]
pub enum ParsingError {
    FailedParsingPluginReference(String),
    FailedParsingProcessorReference(String),
}

impl Error for ParsingError {}

impl fmt::Display for ParsingError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ParsingError::FailedParsingPluginReference(s) => {
                write!(f, "Incorrect Plugin Reference: {}", s)
            }
            ParsingError::FailedParsingProcessorReference(s) => {
                write!(f, "Incorrect Processor Reference: {}", s)
            }
        }
    }
}

pub fn read_file(config_path: String) -> Result<String, Box<dyn Error + Send>> {
    let resp: Result<String, Box<dyn Error + Send>> =
        fs::read_to_string(config_path).map_err(|e| Box::new(e) as Box<dyn Error + Send>);
    resp
}

pub fn read_yaml<T>(config_path: String) -> Result<T, Box<dyn Error + Send>>
where
    T: DeserializeOwned,
{
    let f = std::fs::File::open(config_path).map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
    let d: T = serde_yaml::from_reader(f).map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
    Ok(d)
}

pub fn template_config_mapper(
    r: &CyanTemplateFileConfig,
) -> Result<CyanTemplateConfig, Box<dyn Error + Send>> {
    let proc: Result<Vec<CyanProcessorRef>, Box<dyn Error + Send>> = r
        .processors
        .iter()
        .map(|p| processor_reference_mapper(p.clone()))
        .map(|opt| {
            opt.ok_or(ParsingError::FailedParsingProcessorReference(
                "unknown".to_string(),
            ))
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)
        })
        .collect();

    let plug: Result<Vec<CyanPluginRef>, Box<dyn Error + Send>> = r
        .plugins
        .iter()
        .map(|p| plugin_reference_mapper(p.clone()))
        .map(|opt| {
            opt.ok_or(ParsingError::FailedParsingProcessorReference(
                "unknown".to_string(),
            ))
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)
        })
        .collect();

    let readme_result: Result<String, Box<dyn Error + Send>> =
        fs::read_to_string(r.readme.clone()).map_err(|e| Box::new(e) as Box<dyn Error + Send>);

    let combined_result = proc.and_then(|proc_result| {
        plug.and_then(|plug_result| {
            readme_result.and_then(|readme_r| {
                Ok(CyanTemplateConfig {
                    readme: readme_r,
                    email: r.email.clone(),
                    name: r.name.clone(),
                    description: r.description.clone(),
                    project: r.project.clone(),
                    source: r.source.clone(),
                    tags: r.tags.clone(),
                    username: r.username.clone(),
                    processors: proc_result,
                    plugins: plug_result,
                }) // All three results are not errors, so return Ok(())
            })
        })
    });
    combined_result
}

pub fn processor_config_mapper(
    r: &CyanProcessorFileConfig,
) -> Result<CyanProcessorConfig, Box<dyn Error + Send>> {
    let readme_result: Result<CyanProcessorConfig, Box<dyn Error + Send>> =
        fs::read_to_string(r.readme.clone())
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)
            .map(|readme| CyanProcessorConfig {
                username: r.username.clone(),
                name: r.name.clone(),
                description: r.description.clone(),
                project: r.project.clone(),
                source: r.source.clone(),
                email: r.email.clone(),
                tags: r.tags.clone(),
                readme,
            });
    readme_result
}

pub fn plugin_config_mapper(
    r: &CyanPluginFileConfig,
) -> Result<CyanPluginConfig, Box<dyn Error + Send>> {
    let readme_result: Result<CyanPluginConfig, Box<dyn Error + Send>> =
        fs::read_to_string(r.readme.clone())
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)
            .map(|readme| CyanPluginConfig {
                username: r.username.clone(),
                name: r.name.clone(),
                description: r.description.clone(),
                project: r.project.clone(),
                source: r.source.clone(),
                email: r.email.clone(),
                tags: r.tags.clone(),
                readme,
            });
    readme_result
}
