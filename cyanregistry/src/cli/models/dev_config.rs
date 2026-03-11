use serde::{Deserialize, Serialize};

/// Dev section configuration from cyan.yaml
/// This is used for dev mode to specify external template server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevConfig {
    /// URL of the external template server
    pub template_url: String,

    /// Path to the blob directory (relative to template_path)
    pub blob_path: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_dev_config_minimal() {
        let yaml = r#"
template_url: http://localhost:8080
blob_path: ./blob
"#;
        let config: DevConfig = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        assert_eq!(config.template_url, "http://localhost:8080");
        assert_eq!(config.blob_path, "./blob");
    }

    #[test]
    fn test_parse_dev_config_with_full_url() {
        let yaml = r#"
template_url: https://example.com:9000
blob_path: /path/to/blob
"#;
        let config: DevConfig = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        assert_eq!(config.template_url, "https://example.com:9000");
        assert_eq!(config.blob_path, "/path/to/blob");
    }

    #[test]
    fn test_parse_dev_config_missing_template_url() {
        let yaml = "blob_path: ./blob\n";
        let result: Result<DevConfig, _> = serde_yaml::from_str(yaml);
        assert!(
            result.is_err(),
            "Should fail for missing template_url field"
        );
    }

    #[test]
    fn test_parse_dev_config_missing_blob_path() {
        let yaml = "template_url: http://localhost:8080\n";
        let result: Result<DevConfig, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err(), "Should fail for missing blob_path field");
    }
}
