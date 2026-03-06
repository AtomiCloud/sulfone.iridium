use crate::cli::models::plugin_config::CyanPluginFileConfig;
use crate::cli::models::resolver_config::CyanResolverFileConfig;
use serde::de::DeserializeOwned;
use std::error::Error;
use std::fs::File;
use std::{fmt, fs};

use crate::cli::models::processor_config::CyanProcessorFileConfig;
use crate::cli::models::template_config::CyanTemplateFileConfig;
use crate::domain::config::plugin_config::CyanPluginConfig;
use crate::domain::config::processor_config::CyanProcessorConfig;
use crate::domain::config::resolver_config::CyanResolverConfig;
use crate::domain::config::template_config::{
    CyanPluginRef, CyanProcessorRef, CyanTemplateConfig, CyanTemplateRef,
};

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

#[derive(Debug)]
pub enum ParsingError {
    FailedParsingPluginReference(String),
    FailedParsingProcessorReference(String),
    FailedParsingTemplateReference(String),
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

    let readme_result: Result<String, Box<dyn Error + Send>> =
        fs::read_to_string(r.readme.clone()).map_err(|e| Box::new(e) as Box<dyn Error + Send>);

    proc.and_then(|proc_result| {
        plug.and_then(|plug_result| {
            temp.and_then(|temp_result| {
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
}
