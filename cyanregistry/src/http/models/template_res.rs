use crate::http::models::plugin_res::PluginVersionPrincipalRes;
use crate::http::models::processor_res::ProcessorVersionPrincipalRes;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateVersionPrincipalRes {
    pub id: String,
    pub version: i64,
    pub created_at: String,
    pub description: String,
    pub properties: Option<TemplatePropertyRes>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplatePropertyRes {
    pub blob_docker_reference: String,
    pub blob_docker_tag: String,
    pub template_docker_reference: String,
    pub template_docker_tag: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateVersionRes {
    pub principal: TemplateVersionPrincipalRes,
    pub template: TemplatePrincipalRes,
    pub plugins: Vec<PluginVersionPrincipalRes>,
    pub processors: Vec<ProcessorVersionPrincipalRes>,
    pub templates: Vec<TemplateVersionTemplateRefRes>,
    #[serde(default)]
    pub resolvers: Vec<TemplateVersionResolverRes>,

    #[serde(default)]
    pub commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplatePrincipalRes {
    pub id: String,
    pub name: String,
    pub project: String,
    pub source: String,
    pub email: String,
    pub tags: Vec<String>,
    pub description: String,
    pub readme: String,
    pub user_id: String,
}

/// Template dependency reference with preset answers
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateVersionTemplateRefRes {
    pub id: String,
    pub version: i64,
    #[serde(default)]
    pub preset_answers: std::collections::HashMap<String, serde_json::Value>,
}

/// Resolver reference attached to a template version
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateVersionResolverRes {
    pub id: String,
    pub version: i64,
    pub created_at: String,
    pub description: Option<String>,
    pub docker_reference: String,
    pub docker_tag: String,
    pub config: serde_json::Value,
    pub files: Vec<String>, // Glob patterns
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_version_res_serde_roundtrip_with_commands() {
        let original = TemplateVersionRes {
            principal: TemplateVersionPrincipalRes {
                id: "test-id".to_string(),
                version: 1,
                created_at: "2024-01-01".to_string(),
                description: "Test template".to_string(),
                properties: None,
            },
            template: TemplatePrincipalRes {
                id: "template-id".to_string(),
                name: "my-template".to_string(),
                project: "test-project".to_string(),
                source: "github.com/test/template".to_string(),
                email: "test@test.com".to_string(),
                tags: vec!["test".to_string()],
                description: "A test template".to_string(),
                readme: "Test template".to_string(),
                user_id: "user-123".to_string(),
            },
            plugins: vec![],
            processors: vec![],
            templates: vec![],
            resolvers: vec![],
            commands: vec!["build".to_string(), "test".to_string()],
        };

        let json = serde_json::to_string(&original).expect("serialization should succeed");
        let deserialized: TemplateVersionRes =
            serde_json::from_str(&json).expect("deserialization should succeed");

        assert_eq!(deserialized.template.name, original.template.name);
        assert_eq!(deserialized.commands, original.commands);
    }

    #[test]
    fn test_template_version_res_serde_roundtrip_without_commands() {
        let original = TemplateVersionRes {
            principal: TemplateVersionPrincipalRes {
                id: "test-id".to_string(),
                version: 1,
                created_at: "2024-01-01".to_string(),
                description: "Test template".to_string(),
                properties: None,
            },
            template: TemplatePrincipalRes {
                id: "template-id".to_string(),
                name: "my-template".to_string(),
                project: "test-project".to_string(),
                source: "github.com/test/template".to_string(),
                email: "test@test.com".to_string(),
                tags: vec!["test".to_string()],
                description: "A test template".to_string(),
                readme: "Test template".to_string(),
                user_id: "user-123".to_string(),
            },
            plugins: vec![],
            processors: vec![],
            templates: vec![],
            resolvers: vec![],
            commands: vec![],
        };

        let json = serde_json::to_string(&original).expect("serialization should succeed");
        let deserialized: TemplateVersionRes =
            serde_json::from_str(&json).expect("deserialization should succeed");

        assert_eq!(deserialized.template.name, original.template.name);
        assert!(deserialized.commands.is_empty());
    }

    #[test]
    fn test_template_version_res_backward_compat_without_commands_field() {
        // Simulate old JSON response without commands field
        let old_json = r#"{
            "principal": {
                "id": "test-id",
                "version": 1,
                "createdAt": "2024-01-01",
                "description": "Test template",
                "properties": null
            },
            "template": {
                "id": "template-id",
                "name": "my-template",
                "project": "test-project",
                "source": "github.com/test/template",
                "email": "test@test.com",
                "tags": ["test"],
                "description": "A test template",
                "readme": "Test template",
                "userId": "user-123"
            },
            "plugins": [],
            "processors": [],
            "templates": [],
            "resolvers": []
        }"#;

        let deserialized: TemplateVersionRes =
            serde_json::from_str(old_json).expect("deserialization should succeed");

        // commands should default to empty vec
        assert!(deserialized.commands.is_empty());
    }
}
