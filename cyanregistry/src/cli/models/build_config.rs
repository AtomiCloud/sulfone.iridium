use crate::cli::env_subst::{EnvSubstError, substitute_env_vars};
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
    /// Image name (e.g., "my-template")
    #[serde(default)]
    pub image: Option<String>,

    /// Path to the Dockerfile
    pub dockerfile: String,

    /// Build context directory
    pub context: String,
}

impl BuildConfig {
    /// Substitutes environment variables in all string fields.
    ///
    /// Walks through `registry`, `platforms`, and each `ImageConfig`'s
    /// `image`, `dockerfile`, and `context` fields.
    ///
    /// # Errors
    ///
    /// Returns `EnvSubstError` if any required environment variable is missing
    /// and no default value is provided.
    pub fn substitute_env(self) -> Result<Self, EnvSubstError> {
        Ok(BuildConfig {
            registry: self.registry.map(|r| substitute_env_vars(&r)).transpose()?,
            platforms: self
                .platforms
                .map(|p| {
                    p.into_iter()
                        .map(|s| substitute_env_vars(&s))
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?,
            images: self.images.map(|i| i.substitute_env()).transpose()?,
        })
    }
}

impl ImagesConfig {
    /// Substitutes environment variables in all nested ImageConfig fields.
    pub fn substitute_env(self) -> Result<Self, EnvSubstError> {
        Ok(ImagesConfig {
            template: self.template.map(|t| t.substitute_env()).transpose()?,
            blob: self.blob.map(|b| b.substitute_env()).transpose()?,
            processor: self.processor.map(|p| p.substitute_env()).transpose()?,
            plugin: self.plugin.map(|p| p.substitute_env()).transpose()?,
            resolver: self.resolver.map(|r| r.substitute_env()).transpose()?,
        })
    }
}

impl ImageConfig {
    /// Substitutes environment variables in all string fields.
    pub fn substitute_env(self) -> Result<Self, EnvSubstError> {
        Ok(ImageConfig {
            image: self.image.map(|i| substitute_env_vars(&i)).transpose()?,
            dockerfile: substitute_env_vars(&self.dockerfile)?,
            context: substitute_env_vars(&self.context)?,
        })
    }
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
    image: my-template
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
    image: my-template
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
    image: my-template
    dockerfile: docker/Dockerfile.template
    context: .
  blob:
    image: my-blob
    dockerfile: docker/Dockerfile.blob
    context: ./blob
  processor:
    image: my-processor
    dockerfile: docker/Dockerfile.processor
    context: ./processor
  plugin:
    image: my-plugin
    dockerfile: docker/Dockerfile.plugin
    context: ./plugin
  resolver:
    image: my-resolver
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
        assert_eq!(template.image, Some("my-template".to_string()));
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
