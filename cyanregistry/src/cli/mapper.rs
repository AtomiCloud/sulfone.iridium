use crate::cli::models::build_config::BuildConfig;
use crate::cli::models::plugin_config::CyanPluginFileConfig;
use crate::cli::models::resolver_config::CyanResolverFileConfig;
use serde::de::DeserializeOwned;
use std::error::Error;
use std::fs::File;
use std::{fmt, fs};

use crate::cli::models::processor_config::CyanProcessorFileConfig;
use crate::cli::models::template_config::{CyanResolverRefFileConfig, CyanTemplateFileConfig};
use crate::domain::config::plugin_config::CyanPluginConfig;
use crate::domain::config::processor_config::CyanProcessorConfig;
use crate::domain::config::resolver_config::CyanResolverConfig;
use crate::domain::config::template_config::{
    CyanPluginRef, CyanProcessorRef, CyanResolverRef, CyanTemplateConfig, CyanTemplateRef,
};

pub fn processor_reference_mapper(s: String) -> Option<CyanProcessorRef> {
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

    // Convert version string to i64, require version
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

pub fn template_reference_mapper(s: String) -> Option<CyanTemplateRef> {
    let mut parts = s.splitn(2, '/');
    let username = parts.next()?.to_string();
    let rest = parts.next()?;

    // Split the rest by ':'
    let mut parts = rest.splitn(2, ':');
    let name = parts.next()?.to_string();
    let version_str = parts.next();

    // Convert version string to i64, require version
    let version = match version_str {
        Some(v) => v.parse::<i64>().ok(),
        None => None,
    };

    Some(CyanTemplateRef {
        username,
        name,
        version,
    })
}

/// Parsed resolver reference parts (username, name, optional version)
pub type ResolverReferenceParts = (String, String, Option<u64>);

/// Maps a resolver reference string (e.g., "username/name:version") to parts
/// Returns error if version is present but malformed (not a valid non-negative integer)
pub fn resolver_reference_parse(s: &str) -> Option<Result<ResolverReferenceParts, String>> {
    let mut parts = s.splitn(2, '/');
    let username = parts.next()?.to_string();
    let rest = parts.next()?;

    // Split the rest by ':'
    let mut parts = rest.splitn(2, ':');
    let name = parts.next()?.to_string();
    let version_str = parts.next();

    // Convert version string to u64
    // If version string is present but malformed, return error
    let version = match version_str {
        Some(v) => match v.parse::<u64>() {
            Ok(ver) => Some(ver),
            Err(_) => {
                return Some(Err(format!(
                    "Invalid version '{v}': must be a non-negative integer"
                )));
            }
        },
        None => None,
    };

    Some(Ok((username, name, version)))
}

/// Maps CyanResolverRefFileConfig to CyanResolverRef
pub fn resolver_ref_mapper(r: &CyanResolverRefFileConfig) -> Option<CyanResolverRef> {
    let result = resolver_reference_parse(&r.resolver)?;
    match result {
        Ok((username, name, version)) => Some(CyanResolverRef {
            username,
            name,
            version,
            config: r.config.clone(),
            files: r.files.clone(),
        }),
        Err(_) => None,
    }
}

#[derive(Debug)]
pub enum ParsingError {
    FailedParsingPluginReference(String),
    FailedParsingProcessorReference(String),
    FailedParsingTemplateReference(String),
    FailedParsingResolverReference(String),
    MissingBuildSection(Option<String>),
    MissingBuildRegistry,
    MissingBuildImages,
    MissingImageField(String),
}

impl Error for ParsingError {}

impl fmt::Display for ParsingError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ParsingError::FailedParsingPluginReference(s) => {
                write!(f, "Incorrect Plugin Reference: {s}")
            }
            ParsingError::FailedParsingProcessorReference(s) => {
                write!(f, "Incorrect Processor Reference: {s}")
            }
            ParsingError::FailedParsingTemplateReference(s) => {
                write!(f, "Incorrect Template Reference: {s}")
            }
            ParsingError::FailedParsingResolverReference(s) => {
                write!(f, "Incorrect Resolver Reference: {s}")
            }
            ParsingError::MissingBuildSection(Some(path)) => {
                write!(f, "No build configuration found in {path}")
            }
            ParsingError::MissingBuildSection(None) => {
                write!(f, "No build configuration found")
            }
            ParsingError::MissingBuildRegistry => {
                write!(f, "build.registry is required")
            }
            ParsingError::MissingBuildImages => {
                write!(f, "At least one image must be defined in build.images")
            }
            ParsingError::MissingImageField(image_name) => {
                write!(f, "build.images.{image_name}.image is required")
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
    let f = File::open(config_path).map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
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
            opt.ok_or(ParsingError::FailedParsingPluginReference(
                "unknown".to_string(),
            ))
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)
        })
        .collect();

    let temp: Result<Vec<CyanTemplateRef>, Box<dyn Error + Send>> = r
        .templates
        .iter()
        .map(|t| template_reference_mapper(t.clone()))
        .map(|opt| {
            opt.ok_or(ParsingError::FailedParsingTemplateReference(
                "unknown".to_string(),
            ))
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)
        })
        .collect();

    let resolvers: Result<Vec<CyanResolverRef>, Box<dyn Error + Send>> = r
        .resolvers
        .iter()
        .map(resolver_ref_mapper)
        .map(|opt| {
            opt.ok_or(ParsingError::FailedParsingResolverReference(
                "unknown".to_string(),
            ))
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)
        })
        .collect();

    let readme_result: Result<String, Box<dyn Error + Send>> =
        fs::read_to_string(r.readme.clone()).map_err(|e| Box::new(e) as Box<dyn Error + Send>);

    proc.and_then(|proc_result| {
        plug.and_then(|plug_result| {
            temp.and_then(|temp_result| {
                resolvers.and_then(|resolvers_result| {
                    readme_result.map(|readme_r| CyanTemplateConfig {
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
                        templates: temp_result,
                        resolvers: resolvers_result,
                    })
                })
            })
        })
    })
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

pub fn resolver_config_mapper(
    r: &CyanResolverFileConfig,
) -> Result<CyanResolverConfig, Box<dyn Error + Send>> {
    let readme_result: Result<CyanResolverConfig, Box<dyn Error + Send>> =
        fs::read_to_string(r.readme.clone())
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)
            .map(|readme| CyanResolverConfig {
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

/// File configuration wrapper for build section
/// This is used to parse the build section from cyan.yaml
/// The build field is optional to allow us to detect missing build sections
/// and return the proper "No build configuration found" error message.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct BuildFileConfig {
    #[serde(default)]
    pub build: Option<BuildConfig>,
}

/// Validates that the image field exists and is not empty
fn validate_image_field(
    image_config: &crate::cli::models::build_config::ImageConfig,
    image_name: &str,
) -> Result<(), Box<dyn Error + Send>> {
    match &image_config.image {
        None => Err(
            Box::new(ParsingError::MissingImageField(image_name.to_string()))
                as Box<dyn Error + Send>,
        ),
        Some(image_value) if image_value.trim().is_empty() => Err(Box::new(
            ParsingError::MissingImageField(image_name.to_string()),
        )
            as Box<dyn Error + Send>),
        Some(_) => Ok(()),
    }
}

/// Validates and returns the BuildConfig from a parsed file
/// Returns error if:
/// - No build section exists
/// - Registry is missing
/// - No images are defined
/// - Image field is missing or empty
pub fn build_config_mapper(config: &BuildConfig) -> Result<BuildConfig, Box<dyn Error + Send>> {
    // Validate registry exists and is not empty
    match &config.registry {
        None => return Err(Box::new(ParsingError::MissingBuildRegistry)),
        Some(registry) if registry.trim().is_empty() => {
            return Err(Box::new(ParsingError::MissingBuildRegistry));
        }
        _ => {}
    }

    // Validate images section exists
    let images = config
        .images
        .as_ref()
        .ok_or_else(|| Box::new(ParsingError::MissingBuildImages) as Box<dyn Error + Send>)?;

    // Validate each image has the required 'image' field
    if let Some(ref img) = images.template {
        validate_image_field(img, "template")?;
    }
    if let Some(ref img) = images.blob {
        validate_image_field(img, "blob")?;
    }
    if let Some(ref img) = images.processor {
        validate_image_field(img, "processor")?;
    }
    if let Some(ref img) = images.plugin {
        validate_image_field(img, "plugin")?;
    }
    if let Some(ref img) = images.resolver {
        validate_image_field(img, "resolver")?;
    }

    // Validate at least one image is defined
    let has_images = images.template.is_some()
        || images.blob.is_some()
        || images.processor.is_some()
        || images.plugin.is_some()
        || images.resolver.is_some();

    if !has_images {
        return Err(Box::new(ParsingError::MissingBuildImages));
    }

    Ok(config.clone())
}

/// Reads and parses a build configuration from a YAML file
/// Returns specific error messages for missing build section, missing registry, or no images
pub fn read_build_config(config_path: String) -> Result<BuildConfig, Box<dyn Error + Send>> {
    let file_config: BuildFileConfig = read_yaml(config_path.clone())?;

    // Check if build section exists
    let build_config = file_config.build.ok_or_else(|| {
        Box::new(ParsingError::MissingBuildSection(Some(config_path.clone())))
            as Box<dyn Error + Send>
    })?;

    build_config_mapper(&build_config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_resolver_config_mapper_reads_readme() {
        // Create a temporary directory with a README file
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let readme_path = temp_dir.path().join("README.md");
        let mut readme_file = std::fs::File::create(&readme_path).expect("Failed to create README");
        readme_file
            .write_all(b"# Test Resolver\n\nThis is a test resolver.")
            .expect("Failed to write README");

        let config = CyanResolverFileConfig {
            username: "testuser".to_string(),
            name: "test-resolver".to_string(),
            description: "A test resolver".to_string(),
            project: "test-project".to_string(),
            source: "github.com/test/resolvers".to_string(),
            email: "test@test.com".to_string(),
            tags: vec!["test".to_string()],
            readme: readme_path.to_string_lossy().to_string(),
        };

        let result = resolver_config_mapper(&config);
        assert!(result.is_ok(), "resolver_config_mapper should succeed");

        let domain_config = result.unwrap();
        assert_eq!(domain_config.username, "testuser");
        assert_eq!(domain_config.name, "test-resolver");
        assert_eq!(domain_config.description, "A test resolver");
        assert_eq!(domain_config.project, "test-project");
        assert_eq!(domain_config.source, "github.com/test/resolvers");
        assert_eq!(domain_config.email, "test@test.com");
        assert_eq!(domain_config.tags, vec!["test"]);
        assert_eq!(
            domain_config.readme,
            "# Test Resolver\n\nThis is a test resolver."
        );
    }

    #[test]
    fn test_resolver_config_mapper_fails_on_missing_readme() {
        let config = CyanResolverFileConfig {
            username: "testuser".to_string(),
            name: "test-resolver".to_string(),
            description: "A test resolver".to_string(),
            project: "test-project".to_string(),
            source: "github.com/test/resolvers".to_string(),
            email: "test@test.com".to_string(),
            tags: vec!["test".to_string()],
            readme: "/nonexistent/path/README.md".to_string(),
        };

        let result = resolver_config_mapper(&config);
        assert!(
            result.is_err(),
            "resolver_config_mapper should fail for missing README"
        );
    }

    #[test]
    fn test_resolver_reference_parse() {
        // With version
        let (username, name, version) = resolver_reference_parse("atomi/json-merger:1")
            .unwrap()
            .unwrap();
        assert_eq!(username, "atomi");
        assert_eq!(name, "json-merger");
        assert_eq!(version, Some(1u64));

        // Without version
        let (username, name, version) = resolver_reference_parse("atomi/json-merger")
            .unwrap()
            .unwrap();
        assert_eq!(username, "atomi");
        assert_eq!(name, "json-merger");
        assert_eq!(version, None);
    }

    #[test]
    fn test_resolver_ref_mapper() {
        let config = CyanResolverRefFileConfig {
            resolver: "atomi/json-merger:1".to_string(),
            config: serde_json::json!({"strategy": "deep-merge"}),
            files: vec!["package.json".to_string(), "**/tsconfig.json".to_string()],
        };

        let result = resolver_ref_mapper(&config).unwrap();
        assert_eq!(result.username, "atomi");
        assert_eq!(result.name, "json-merger");
        assert_eq!(result.version, Some(1u64));
        assert_eq!(result.files, vec!["package.json", "**/tsconfig.json"]);
    }

    // ===== Build Config Mapper Tests =====

    #[test]
    fn test_read_build_config_missing_build_section() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config_path = temp_dir.path().join("cyan.yaml");
        let mut config_file = std::fs::File::create(&config_path).expect("Failed to create config");
        // Write YAML without build section
        config_file
            .write_all(b"some_field: value\n")
            .expect("Failed to write config");

        let result = read_build_config(config_path.to_string_lossy().to_string());
        assert!(result.is_err(), "Should fail for missing build section");

        let err = result.unwrap_err();
        let err_msg = err.to_string();
        assert!(err_msg.contains("No build configuration found in"));
    }

    #[test]
    fn test_build_config_mapper_missing_registry() {
        use crate::cli::models::build_config::{BuildConfig, ImagesConfig};

        let config = BuildConfig {
            registry: None, // Missing registry
            platforms: None,
            images: Some(ImagesConfig {
                template: Some(crate::cli::models::build_config::ImageConfig {
                    image: Some("my-template".to_string()),
                    dockerfile: "Dockerfile".to_string(),
                    context: ".".to_string(),
                }),
                blob: None,
                processor: None,
                plugin: None,
                resolver: None,
            }),
        };

        let result = build_config_mapper(&config);
        assert!(result.is_err(), "Should fail for missing registry");

        let err = result.unwrap_err();
        assert_eq!(err.to_string(), "build.registry is required");
    }

    #[test]
    fn test_build_config_mapper_no_images() {
        use crate::cli::models::build_config::{BuildConfig, ImagesConfig};

        let config = BuildConfig {
            registry: Some("ghcr.io/atomicloud".to_string()),
            platforms: None,
            images: Some(ImagesConfig::default()), // No images defined
        };

        let result = build_config_mapper(&config);
        assert!(result.is_err(), "Should fail for no images defined");

        let err = result.unwrap_err();
        assert_eq!(
            err.to_string(),
            "At least one image must be defined in build.images"
        );
    }

    #[test]
    fn test_build_config_mapper_valid_config() {
        use crate::cli::models::build_config::{BuildConfig, ImageConfig, ImagesConfig};

        let config = BuildConfig {
            registry: Some("ghcr.io/atomicloud".to_string()),
            platforms: Some(vec!["linux/amd64".to_string()]),
            images: Some(ImagesConfig {
                template: Some(ImageConfig {
                    image: Some("my-template".to_string()),
                    dockerfile: "Dockerfile".to_string(),
                    context: ".".to_string(),
                }),
                blob: None,
                processor: None,
                plugin: None,
                resolver: None,
            }),
        };

        let result = build_config_mapper(&config);
        assert!(result.is_ok(), "Should succeed with valid config");

        let parsed = result.unwrap();
        assert_eq!(parsed.registry, Some("ghcr.io/atomicloud".to_string()));
        assert_eq!(parsed.platforms, Some(vec!["linux/amd64".to_string()]));
    }

    #[test]
    fn test_read_build_config_missing_registry_field() {
        // Test that YAML missing registry field returns proper error message
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config_path = temp_dir.path().join("cyan.yaml");
        let mut config_file = std::fs::File::create(&config_path).expect("Failed to create config");
        // Write YAML with build section but missing registry
        config_file
            .write_all(
                r#"build:
  images:
    template:
      image: my-template
      dockerfile: Dockerfile
      context: .
"#
                .as_bytes(),
            )
            .expect("Failed to write config");

        let result = read_build_config(config_path.to_string_lossy().to_string());
        assert!(result.is_err(), "Should fail for missing registry field");

        let err = result.unwrap_err();
        let err_msg = err.to_string();
        assert_eq!(
            err_msg, "build.registry is required",
            "Error message should match spec, got: {err_msg}"
        );
    }

    #[test]
    fn test_read_build_config_missing_images_field() {
        // Test that YAML missing images field returns proper error message
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config_path = temp_dir.path().join("cyan.yaml");
        let mut config_file = std::fs::File::create(&config_path).expect("Failed to create config");
        // Write YAML with build section but missing images
        config_file
            .write_all(
                r#"build:
  registry: ghcr.io/atomicloud
"#
                .as_bytes(),
            )
            .expect("Failed to write config");

        let result = read_build_config(config_path.to_string_lossy().to_string());
        assert!(result.is_err(), "Should fail for missing images field");

        let err = result.unwrap_err();
        let err_msg = err.to_string();
        assert_eq!(
            err_msg, "At least one image must be defined in build.images",
            "Error message should match spec, got: {err_msg}"
        );
    }

    #[test]
    fn test_read_build_config_empty_images_field() {
        // Test that YAML with empty images field returns proper error message
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config_path = temp_dir.path().join("cyan.yaml");
        let mut config_file = std::fs::File::create(&config_path).expect("Failed to create config");
        // Write YAML with build section but empty images
        config_file
            .write_all(
                r#"build:
  registry: ghcr.io/atomicloud
  images:
"#
                .as_bytes(),
            )
            .expect("Failed to write config");

        let result = read_build_config(config_path.to_string_lossy().to_string());
        assert!(result.is_err(), "Should fail for empty images field");

        let err = result.unwrap_err();
        let err_msg = err.to_string();
        assert_eq!(
            err_msg, "At least one image must be defined in build.images",
            "Error message should match spec, got: {err_msg}"
        );
    }

    #[test]
    fn test_read_build_config_missing_image_field() {
        // Test that YAML missing image field returns proper error message
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config_path = temp_dir.path().join("cyan.yaml");
        let mut config_file = std::fs::File::create(&config_path).expect("Failed to create config");
        // Write YAML with build section but missing image field
        config_file
            .write_all(
                r#"build:
  registry: ghcr.io/atomicloud
  images:
    template:
      dockerfile: Dockerfile
      context: .
"#
                .as_bytes(),
            )
            .expect("Failed to write config");

        let result = read_build_config(config_path.to_string_lossy().to_string());
        assert!(result.is_err(), "Should fail for missing image field");

        let err = result.unwrap_err();
        let err_msg = err.to_string();
        assert_eq!(
            err_msg, "build.images.template.image is required",
            "Error message should match spec, got: {err_msg}"
        );
    }

    #[test]
    fn test_read_build_config_empty_image_field() {
        // Test that YAML with empty image field returns proper error message
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config_path = temp_dir.path().join("cyan.yaml");
        let mut config_file = std::fs::File::create(&config_path).expect("Failed to create config");
        // Write YAML with build section but empty image field
        config_file
            .write_all(
                r#"build:
  registry: ghcr.io/atomicloud
  images:
    template:
      image: ""
      dockerfile: Dockerfile
      context: .
"#
                .as_bytes(),
            )
            .expect("Failed to write config");

        let result = read_build_config(config_path.to_string_lossy().to_string());
        assert!(result.is_err(), "Should fail for empty image field");

        let err = result.unwrap_err();
        let err_msg = err.to_string();
        assert_eq!(
            err_msg, "build.images.template.image is required",
            "Error message should match spec, got: {err_msg}"
        );
    }

    #[test]
    fn test_read_build_config_valid_with_image_field() {
        // Test that valid YAML with image field parses correctly
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config_path = temp_dir.path().join("cyan.yaml");
        let mut config_file = std::fs::File::create(&config_path).expect("Failed to create config");
        config_file
            .write_all(
                r#"build:
  registry: ghcr.io/atomicloud
  images:
    template:
      image: my-template
      dockerfile: Dockerfile
      context: .
"#
                .as_bytes(),
            )
            .expect("Failed to write config");

        let result = read_build_config(config_path.to_string_lossy().to_string());
        assert!(result.is_ok(), "Should succeed with valid config");

        let config = result.unwrap();
        assert_eq!(config.registry, Some("ghcr.io/atomicloud".to_string()));
        let images = config.images.unwrap();
        let template = images.template.unwrap();
        assert_eq!(template.image, Some("my-template".to_string()));
        assert_eq!(template.dockerfile, "Dockerfile");
        assert_eq!(template.context, ".");
    }
}
