use serde::{Deserialize, Serialize};

/// Build section configuration from cyan.yaml
/// Fields are optional to allow validation in mapper to return proper error messages
/// instead of serde's "missing field" errors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    /// Container registry URL (e.g., "ghcr.io/atomicloud")
    #[serde(default)]
    pub registry: Option<String>,

    /// Target platforms for multi-arch builds (e.g., ["linux/amd64", "linux/arm64"])
    #[serde(default)]
    pub platforms: Option<Vec<String>>,

    /// Image configurations for each artifact type
    #[serde(default)]
    pub images: Option<ImagesConfig>,
}

/// Image configurations for all artifact types
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ImagesConfig {
    /// Template image configuration
    #[serde(default)]
    pub template: Option<ImageConfig>,

    /// Blob image configuration
    #[serde(default)]
    pub blob: Option<ImageConfig>,

    /// Processor image configuration
    #[serde(default)]
    pub processor: Option<ImageConfig>,

    /// Plugin image configuration
    #[serde(default)]
    pub plugin: Option<ImageConfig>,

    /// Resolver image configuration
    #[serde(default)]
    pub resolver: Option<ImageConfig>,
}

/// Individual image build configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageConfig {
    /// Path to the Dockerfile
    pub dockerfile: String,

    /// Build context directory
    pub context: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_build_config_minimal() {
        let yaml = r#"
registry: ghcr.io/atomicloud
images:
  template:
    dockerfile: Dockerfile.template
    context: .
"#;
        let config: BuildConfig = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        assert_eq!(config.registry, Some("ghcr.io/atomicloud".to_string()));
        assert!(config.platforms.is_none());
        assert!(config.images.is_some());
        let images = config.images.unwrap();
        assert!(images.template.is_some());
        assert!(images.blob.is_none());
        assert!(images.processor.is_none());
        assert!(images.plugin.is_none());
        assert!(images.resolver.is_none());
    }

    #[test]
    fn test_parse_build_config_with_platforms() {
        let yaml = r#"
registry: ghcr.io/atomicloud
platforms:
  - linux/amd64
  - linux/arm64
images:
  template:
    dockerfile: Dockerfile.template
    context: .
"#;
        let config: BuildConfig = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        assert_eq!(config.registry, Some("ghcr.io/atomicloud".to_string()));
        assert_eq!(
            config.platforms,
            Some(vec!["linux/amd64".to_string(), "linux/arm64".to_string()])
        );
    }

    #[test]
    fn test_parse_build_config_all_images() {
        let yaml = r#"
registry: ghcr.io/atomicloud
platforms:
  - linux/amd64
images:
  template:
    dockerfile: docker/Dockerfile.template
    context: .
  blob:
    dockerfile: docker/Dockerfile.blob
    context: ./blob
  processor:
    dockerfile: docker/Dockerfile.processor
    context: ./processor
  plugin:
    dockerfile: docker/Dockerfile.plugin
    context: ./plugin
  resolver:
    dockerfile: docker/Dockerfile.resolver
    context: ./resolver
"#;
        let config: BuildConfig = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        let images = config.images.as_ref().expect("images should be present");
        assert!(images.template.is_some());
        assert!(images.blob.is_some());
        assert!(images.processor.is_some());
        assert!(images.plugin.is_some());
        assert!(images.resolver.is_some());

        let template = images.template.as_ref().unwrap();
        assert_eq!(template.dockerfile, "docker/Dockerfile.template");
        assert_eq!(template.context, ".");
    }

    #[test]
    fn test_images_config_default() {
        let config = ImagesConfig::default();
        assert!(config.template.is_none());
        assert!(config.blob.is_none());
        assert!(config.processor.is_none());
        assert!(config.plugin.is_none());
        assert!(config.resolver.is_none());
    }
}
