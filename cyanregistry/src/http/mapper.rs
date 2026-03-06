use crate::domain::config::plugin_config::CyanPluginConfig;
use crate::domain::config::processor_config::CyanProcessorConfig;
use crate::domain::config::resolver_config::CyanResolverConfig;
use crate::domain::config::resolver_ref_config::CyanResolverRef;
use crate::domain::config::template_config::{
    CyanPluginRef, CyanProcessorRef, CyanTemplateConfig, CyanTemplateRef,
};
use crate::http::models::plugin_req::PluginReq;
use crate::http::models::processor_req::ProcessorReq;
use crate::http::models::resolver_ref_req::ResolverRefReq;
use crate::http::models::resolver_req::ResolverReq;
use crate::http::models::template_req::{
    PluginRefReq, ProcessorRefReq, TemplatePropertyReq, TemplateRefReq, TemplateReq,
};

pub fn processor_req_mapper(
    r: &CyanProcessorConfig,
    desc: String,
    docker_ref: String,
    docker_tag: String,
) -> ProcessorReq {
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
        docker_tag: docker_tag.to_string(),
    }
}

pub fn plugin_req_mapper(
    r: &CyanPluginConfig,
    desc: String,
    docker_ref: String,
    docker_tag: String,
) -> PluginReq {
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
        docker_tag: docker_tag.to_string(),
    }
}

pub fn plugin_ref_req_mapper(r: &CyanPluginRef) -> PluginRefReq {
    PluginRefReq {
        username: r.username.clone(),
        name: r.name.clone(),
        version: r.version.unwrap_or(0),
    }
}

pub fn processor_ref_req_mapper(r: &CyanProcessorRef) -> ProcessorRefReq {
    ProcessorRefReq {
        username: r.username.clone(),
        name: r.name.clone(),
        version: r.version.unwrap_or(0),
    }
}

pub fn template_ref_req_mapper(r: &CyanTemplateRef) -> TemplateRefReq {
    TemplateRefReq {
        username: r.username.clone(),
        name: r.name.clone(),
        version: r.version.unwrap_or(0),
    }
}

/// Maps CyanResolverRef domain model to ResolverRefReq HTTP request model
pub fn resolver_ref_req_mapper(r: &CyanResolverRef) -> ResolverRefReq {
    ResolverRefReq {
        resolver_reference: format!("{}/{}", r.username, r.name),
        resolver_version: r.version.unwrap_or(0),
        config: r.config.clone(),
        files: r.files.clone(),
    }
}

// Mapper for template with properties
pub fn template_req_with_properties_mapper(
    r: &CyanTemplateConfig,
    desc: String,
    blob_docker_ref: String,
    blob_docker_tag: String,
    template_docker_ref: String,
    template_docker_tag: String,
) -> TemplateReq {
    let properties = Some(TemplatePropertyReq {
        blob_docker_reference: blob_docker_ref.to_string(),
        blob_docker_tag: blob_docker_tag.to_string(),
        template_docker_reference: template_docker_ref.to_string(),
        template_docker_tag: template_docker_tag.to_string(),
    });

    TemplateReq {
        name: r.name.clone(),
        project: r.project.clone(),
        source: r.source.clone(),
        email: r.email.clone(),
        tags: r.tags.clone(),
        description: r.description.clone(),
        readme: r.readme.clone(),
        version_description: desc,
        properties,
        plugins: r.plugins.iter().map(plugin_ref_req_mapper).collect(),
        processors: r.processors.iter().map(processor_ref_req_mapper).collect(),
        templates: r.templates.iter().map(template_ref_req_mapper).collect(),
        resolvers: r.resolvers.iter().map(resolver_ref_req_mapper).collect(),
    }
}

// Mapper for template without properties
pub fn template_req_without_properties_mapper(r: &CyanTemplateConfig, desc: String) -> TemplateReq {
    TemplateReq {
        name: r.name.clone(),
        project: r.project.clone(),
        source: r.source.clone(),
        email: r.email.clone(),
        tags: r.tags.clone(),
        description: r.description.clone(),
        readme: r.readme.clone(),
        version_description: desc,
        properties: None,
        plugins: r.plugins.iter().map(plugin_ref_req_mapper).collect(),
        processors: r.processors.iter().map(processor_ref_req_mapper).collect(),
        templates: r.templates.iter().map(template_ref_req_mapper).collect(),
        resolvers: r.resolvers.iter().map(resolver_ref_req_mapper).collect(),
    }
}

// Legacy mapper for backward compatibility
pub fn template_req_mapper(
    r: &CyanTemplateConfig,
    desc: String,
    blob_docker_ref: String,
    blob_docker_tag: String,
    template_docker_ref: String,
    template_docker_tag: String,
) -> TemplateReq {
    template_req_with_properties_mapper(
        r,
        desc,
        blob_docker_ref,
        blob_docker_tag,
        template_docker_ref,
        template_docker_tag,
    )
}

pub fn resolver_req_mapper(
    r: &CyanResolverConfig,
    desc: String,
    docker_ref: String,
    docker_tag: String,
) -> ResolverReq {
    ResolverReq {
        name: r.name.clone(),
        project: r.project.clone(),
        source: r.source.clone(),
        email: r.email.clone(),
        tags: r.tags.clone(),
        description: r.description.clone(),
        readme: r.readme.clone(),
        version_description: desc,
        docker_reference: docker_ref.to_string(),
        docker_tag: docker_tag.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolver_req_mapper() {
        let config = CyanResolverConfig {
            username: "cyane2e".to_string(),
            name: "json-merger".to_string(),
            description: "Deep merge JSON files".to_string(),
            project: "atomi".to_string(),
            source: "github.com/atomi/resolvers".to_string(),
            email: "dev@atomi.com".to_string(),
            tags: vec!["json".to_string(), "merge".to_string()],
            readme: "JSON Merger\n\nMerges JSON files...".to_string(),
        };

        let req = resolver_req_mapper(
            &config,
            "Initial version".to_string(),
            "atomi/json-merger".to_string(),
            "1.0.0".to_string(),
        );

        assert_eq!(req.name, "json-merger");
        assert_eq!(req.project, "atomi");
        assert_eq!(req.source, "github.com/atomi/resolvers");
        assert_eq!(req.email, "dev@atomi.com");
        assert_eq!(req.tags, vec!["json", "merge"]);
        assert_eq!(req.description, "Deep merge JSON files");
        assert_eq!(req.readme, "JSON Merger\n\nMerges JSON files...");
        assert_eq!(req.version_description, "Initial version");
        assert_eq!(req.docker_reference, "atomi/json-merger");
        assert_eq!(req.docker_tag, "1.0.0");
    }

    #[test]
    fn test_resolver_req_mapper_preserves_all_fields() {
        let config = CyanResolverConfig {
            username: "testuser".to_string(),
            name: "line-merger".to_string(),
            description: "Line-based merge".to_string(),
            project: "test-project".to_string(),
            source: "github.com/test/resolvers".to_string(),
            email: "test@test.com".to_string(),
            tags: vec!["line".to_string()],
            readme: "Line Merger".to_string(),
        };

        let req = resolver_req_mapper(
            &config,
            "v2 release".to_string(),
            "test/line-merger".to_string(),
            "2.0.0".to_string(),
        );

        // All domain fields should be mapped
        assert_eq!(req.name, config.name);
        assert_eq!(req.project, config.project);
        assert_eq!(req.source, config.source);
        assert_eq!(req.email, config.email);
        assert_eq!(req.tags, config.tags);
        assert_eq!(req.description, config.description);
        assert_eq!(req.readme, config.readme);
    }

    #[test]
    fn test_resolver_ref_req_mapper() {
        let resolver_ref = CyanResolverRef {
            username: "atomi".to_string(),
            name: "json-merger".to_string(),
            version: Some(1),
            config: Some(serde_json::json!({"strategy": "deep-merge"})),
            files: vec!["package.json".to_string(), "**/tsconfig.json".to_string()],
        };

        let req = resolver_ref_req_mapper(&resolver_ref);

        assert_eq!(req.resolver_reference, "atomi/json-merger");
        assert_eq!(req.resolver_version, 1);
        assert_eq!(
            req.config,
            Some(serde_json::json!({"strategy": "deep-merge"}))
        );
        assert_eq!(req.files, vec!["package.json", "**/tsconfig.json"]);
    }

    #[test]
    fn test_resolver_ref_req_mapper_without_version() {
        let resolver_ref = CyanResolverRef {
            username: "atomi".to_string(),
            name: "json-merger".to_string(),
            version: None,
            config: None,
            files: vec!["*.json".to_string()],
        };

        let req = resolver_ref_req_mapper(&resolver_ref);

        assert_eq!(req.resolver_reference, "atomi/json-merger");
        assert_eq!(req.resolver_version, 0); // Default to 0 when no version
        assert_eq!(req.config, None);
        assert_eq!(req.files, vec!["*.json"]);
    }
}
