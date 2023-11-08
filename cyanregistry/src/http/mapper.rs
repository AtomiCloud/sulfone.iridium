use crate::domain::config::plugin_config::CyanPluginConfig;
use crate::domain::config::processor_config::CyanProcessorConfig;
use crate::domain::config::template_config::{CyanPluginRef, CyanProcessorRef, CyanTemplateConfig};
use crate::http::models::plugin_req::PluginReq;
use crate::http::models::processor_req::ProcessorReq;
use crate::http::models::template_req::{PluginRefReq, ProcessorRefReq, TemplateReq};

pub fn processor_req_mapper(r: &CyanProcessorConfig, desc: String, docker_ref: String, docker_sha: String) -> ProcessorReq {
    ProcessorReq {
        name: r.name.clone(),
        project: r.project.clone(),
        source: r.source.clone(),
        email: r.email.clone(),
        tags: r.tags.clone(),
        description: r.description.clone(),
        readme: r.readme.clone(),
        version_description: desc,
        docker_reference: docker_ref.to_string(),
        docker_sha: docker_sha.to_string(),
    }
}

pub fn plugin_req_mapper(r: &CyanPluginConfig, desc: String, docker_ref: String, docker_sha: String) -> PluginReq {
    PluginReq {
        name: r.name.clone(),
        project: r.project.clone(),
        source: r.source.clone(),
        email: r.email.clone(),
        tags: r.tags.clone(),
        description: r.description.clone(),
        readme: r.readme.clone(),
        version_description: desc,
        docker_reference: docker_ref.to_string(),
        docker_sha: docker_sha.to_string(),
    }
}

pub fn plugin_ref_req_mapper(r : &CyanPluginRef) -> PluginRefReq {
    PluginRefReq {
        username: r.username.clone(),
        name: r.name.clone(),
        version: r.version.unwrap_or(0),
    }
}

pub fn processor_ref_req_mapper(r : &CyanProcessorRef) -> ProcessorRefReq {
    ProcessorRefReq {
        username: r.username.clone(),
        name: r.name.clone(),
        version: r.version.unwrap_or(0),
    }
}

pub fn template_req_mapper(r: &CyanTemplateConfig, desc: String, blob_docker_ref: String, blob_docker_sha: String,
    template_docker_ref: String, template_docker_sha: String
) -> TemplateReq {
    TemplateReq {
        name: r.name.clone(),
        project: r.project.clone(),
        source: r.source.clone(),
        email: r.email.clone(),
        tags: r.tags.clone(),
        description: r.description.clone(),
        readme: r.readme.clone(),
        version_description: desc,
        blob_docker_reference: blob_docker_ref.to_string(),
        blob_docker_sha: blob_docker_sha.to_string(),
        template_docker_reference: template_docker_ref.to_string(),
        template_docker_sha: template_docker_sha.to_string(),
        plugins: r.plugins.iter().map(|p| plugin_ref_req_mapper(p)).collect(),
        processors: r.processors.iter().map(|p| processor_ref_req_mapper(p)).collect(),
    }
}