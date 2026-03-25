use crate::domain::config::plugin_config::CyanPluginConfig;
use crate::domain::config::processor_config::CyanProcessorConfig;
use crate::domain::config::resolver_config::CyanResolverConfig;
use crate::domain::config::template_config::{
    CyanPluginRef, CyanProcessorRef, CyanResolverRef, CyanTemplateConfig, CyanTemplateRef,
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
        preset_answers: r.preset_answers.clone(),
    }
}

/// Maps CyanResolverRef domain model to ResolverRefReq HTTP request model
pub fn resolver_ref_req_mapper(r: &CyanResolverRef) -> ResolverRefReq {
    ResolverRefReq {
        username: r.username.clone(),
        name: r.name.clone(),
        version: r.version.map(|v| v as i64).unwrap_or(0),
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
        commands: r
            .commands
            .iter()
            .filter(|c| !c.trim().is_empty())
            .cloned()
            .collect(),
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
        commands: r
            .commands
            .iter()
            .filter(|c| !c.trim().is_empty())
            .cloned()
            .collect(),
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
            config: serde_json::json!({"strategy": "deep-merge"}),
            files: vec!["package.json".to_string(), "**/tsconfig.json".to_string()],
        };

        let req = resolver_ref_req_mapper(&resolver_ref);

        assert_eq!(req.username, "atomi");
        assert_eq!(req.name, "json-merger");
        assert_eq!(req.version, 1);
        assert_eq!(req.config, serde_json::json!({"strategy": "deep-merge"}));
        assert_eq!(req.files, vec!["package.json", "**/tsconfig.json"]);
    }

    #[test]
    fn test_resolver_ref_req_mapper_without_version() {
        let resolver_ref = CyanResolverRef {
            username: "atomi".to_string(),
            name: "json-merger".to_string(),
            version: None,
            config: serde_json::json!({}),
            files: vec!["*.json".to_string()],
        };

        let req = resolver_ref_req_mapper(&resolver_ref);

        assert_eq!(req.username, "atomi");
        assert_eq!(req.name, "json-merger");
        assert_eq!(req.version, 0); // Default to 0 when no version
        assert_eq!(req.config, serde_json::json!({}));
        assert_eq!(req.files, vec!["*.json"]);
    }

    #[test]
    fn test_template_ref_req_mapper_with_preset_answers() {
        let mut preset_answers = std::collections::HashMap::new();
        preset_answers.insert("framework".to_string(), serde_json::json!("react"));
        preset_answers.insert("language".to_string(), serde_json::json!("typescript"));

        let template_ref = CyanTemplateRef {
            username: "cyane2e".to_string(),
            name: "web-app".to_string(),
            version: Some(3),
            preset_answers: preset_answers.clone(),
        };

        let req = template_ref_req_mapper(&template_ref);

        assert_eq!(req.username, "cyane2e");
        assert_eq!(req.name, "web-app");
        assert_eq!(req.version, 3);
        assert_eq!(req.preset_answers, preset_answers);
    }

    #[test]
    fn test_template_ref_req_mapper_without_preset_answers() {
        let template_ref = CyanTemplateRef {
            username: "cyane2e".to_string(),
            name: "base-template".to_string(),
            version: Some(1),
            preset_answers: std::collections::HashMap::new(),
        };

        let req = template_ref_req_mapper(&template_ref);

        assert_eq!(req.username, "cyane2e");
        assert_eq!(req.name, "base-template");
        assert_eq!(req.version, 1);
        assert!(req.preset_answers.is_empty());
    }

    #[test]
    fn test_template_ref_req_serde_roundtrip_with_preset_answers() {
        let mut preset_answers = std::collections::HashMap::new();
        preset_answers.insert("key".to_string(), serde_json::json!("value"));
        preset_answers.insert("count".to_string(), serde_json::json!(42));
        preset_answers.insert(
            "nested".to_string(),
            serde_json::json!({"foo": "bar", "enabled": true}),
        );

        let original = TemplateRefReq {
            username: "testuser".to_string(),
            name: "my-template".to_string(),
            version: 5,
            preset_answers,
        };

        let json = serde_json::to_string(&original).expect("serialization should succeed");
        let deserialized: TemplateRefReq =
            serde_json::from_str(&json).expect("deserialization should succeed");

        assert_eq!(deserialized.username, original.username);
        assert_eq!(deserialized.name, original.name);
        assert_eq!(deserialized.version, original.version);
        assert_eq!(deserialized.preset_answers, original.preset_answers);
    }

    #[test]
    fn test_template_req_with_properties_mapper_with_commands() {
        let config = CyanTemplateConfig {
            username: "testuser".to_string(),
            name: "my-template".to_string(),
            description: "A test template".to_string(),
            project: "test-project".to_string(),
            source: "github.com/test/template".to_string(),
            email: "test@test.com".to_string(),
            tags: vec!["test".to_string()],
            processors: vec![],
            plugins: vec![],
            templates: vec![],
            readme: "Test template".to_string(),
            resolvers: vec![],
            commands: vec!["echo hello".to_string(), "echo world".to_string()],
        };

        let req = template_req_with_properties_mapper(
            &config,
            "Initial version".to_string(),
            "blob-ref".to_string(),
            "blob-tag".to_string(),
            "template-ref".to_string(),
            "template-tag".to_string(),
        );

        assert_eq!(req.commands, vec!["echo hello", "echo world"]);
    }

    #[test]
    fn test_template_req_without_properties_mapper_with_commands() {
        let config = CyanTemplateConfig {
            username: "testuser".to_string(),
            name: "my-template".to_string(),
            description: "A test template".to_string(),
            project: "test-project".to_string(),
            source: "github.com/test/template".to_string(),
            email: "test@test.com".to_string(),
            tags: vec!["test".to_string()],
            processors: vec![],
            plugins: vec![],
            templates: vec![],
            readme: "Test template".to_string(),
            resolvers: vec![],
            commands: vec!["build".to_string(), "test".to_string()],
        };

        let req = template_req_without_properties_mapper(&config, "v1".to_string());

        assert_eq!(req.commands, vec!["build", "test"]);
    }

    #[test]
    fn test_template_req_mapper_with_empty_commands() {
        let config = CyanTemplateConfig {
            username: "testuser".to_string(),
            name: "my-template".to_string(),
            description: "A test template".to_string(),
            project: "test-project".to_string(),
            source: "github.com/test/template".to_string(),
            email: "test@test.com".to_string(),
            tags: vec!["test".to_string()],
            processors: vec![],
            plugins: vec![],
            templates: vec![],
            readme: "Test template".to_string(),
            resolvers: vec![],
            commands: vec![],
        };

        let req = template_req_with_properties_mapper(
            &config,
            "Initial version".to_string(),
            "blob-ref".to_string(),
            "blob-tag".to_string(),
            "template-ref".to_string(),
            "template-tag".to_string(),
        );

        assert!(req.commands.is_empty());
    }

    #[test]
    fn test_template_req_mapper_with_single_command() {
        let config = CyanTemplateConfig {
            username: "testuser".to_string(),
            name: "my-template".to_string(),
            description: "A test template".to_string(),
            project: "test-project".to_string(),
            source: "github.com/test/template".to_string(),
            email: "test@test.com".to_string(),
            tags: vec!["test".to_string()],
            processors: vec![],
            plugins: vec![],
            templates: vec![],
            readme: "Test template".to_string(),
            resolvers: vec![],
            commands: vec!["npm run build".to_string()],
        };

        let req = template_req_with_properties_mapper(
            &config,
            "Initial version".to_string(),
            "blob-ref".to_string(),
            "blob-tag".to_string(),
            "template-ref".to_string(),
            "template-tag".to_string(),
        );

        assert_eq!(req.commands, vec!["npm run build"]);
    }

    #[test]
    fn test_template_req_mapper_filters_empty_commands() {
        let config = CyanTemplateConfig {
            username: "testuser".to_string(),
            name: "my-template".to_string(),
            description: "A test template".to_string(),
            project: "test-project".to_string(),
            source: "github.com/test/template".to_string(),
            email: "test@test.com".to_string(),
            tags: vec!["test".to_string()],
            processors: vec![],
            plugins: vec![],
            templates: vec![],
            readme: "Test template".to_string(),
            resolvers: vec![],
            commands: vec![
                "echo hello".to_string(),
                "".to_string(),
                "   ".to_string(),
                "echo world".to_string(),
            ],
        };

        let req = template_req_with_properties_mapper(
            &config,
            "Initial version".to_string(),
            "blob-ref".to_string(),
            "blob-tag".to_string(),
            "template-ref".to_string(),
            "template-tag".to_string(),
        );

        // Empty and whitespace-only commands should be filtered out
        assert_eq!(req.commands, vec!["echo hello", "echo world"]);
    }

    #[test]
    fn test_template_req_serde_roundtrip_with_commands() {
        let original = TemplateReq {
            name: "my-template".to_string(),
            project: "test-project".to_string(),
            source: "github.com/test/template".to_string(),
            email: "test@test.com".to_string(),
            tags: vec!["test".to_string()],
            description: "A test template".to_string(),
            readme: "Test template".to_string(),
            version_description: "Initial version".to_string(),
            properties: None,
            plugins: vec![],
            processors: vec![],
            templates: vec![],
            resolvers: vec![],
            commands: vec!["build".to_string(), "test".to_string()],
        };

        let json = serde_json::to_string(&original).expect("serialization should succeed");
        let deserialized: TemplateReq =
            serde_json::from_str(&json).expect("deserialization should succeed");

        assert_eq!(deserialized.name, original.name);
        assert_eq!(deserialized.commands, original.commands);
    }

    #[test]
    fn test_template_req_serde_roundtrip_without_commands() {
        let original = TemplateReq {
            name: "my-template".to_string(),
            project: "test-project".to_string(),
            source: "github.com/test/template".to_string(),
            email: "test@test.com".to_string(),
            tags: vec!["test".to_string()],
            description: "A test template".to_string(),
            readme: "Test template".to_string(),
            version_description: "Initial version".to_string(),
            properties: None,
            plugins: vec![],
            processors: vec![],
            templates: vec![],
            resolvers: vec![],
            commands: vec![],
        };

        let json = serde_json::to_string(&original).expect("serialization should succeed");
        let deserialized: TemplateReq =
            serde_json::from_str(&json).expect("deserialization should succeed");

        assert_eq!(deserialized.name, original.name);
        assert!(deserialized.commands.is_empty());
    }
}
