use std::error::Error;
use std::process::{Command, Output, Stdio};

/// Shell-escape an argument for safe inclusion in command strings
fn shell_escape(arg: &str) -> String {
    // Simple shell escaping: wrap in single quotes and escape existing single quotes
    let mut escaped = String::with_capacity(arg.len() + 2);
    escaped.push('\'');
    for c in arg.chars() {
        if c == '\'' {
            escaped.push_str("'\\''");
        } else {
            escaped.push(c);
        }
    }
    escaped.push('\'');
    escaped
}

/// Builder for executing Docker buildx commands
pub struct BuildxBuilder {
    /// Optional builder name to use
    builder: Option<String>,
}

/// Build options for a Docker image
#[derive(Debug, Clone)]
pub struct BuildOptions<'a> {
    /// Container registry URL (e.g., "ghcr.io/atomicloud")
    pub registry: &'a str,
    /// Image name (e.g., "my-template")
    pub image_name: &'a str,
    /// Image tag (e.g., "v1.0.0")
    pub tag: &'a str,
    /// Path to the Dockerfile
    pub dockerfile: &'a str,
    /// Build context directory
    pub context: &'a str,
    /// Target platforms (e.g., ["linux/amd64", "linux/arm64"])
    pub platforms: &'a [String],
    /// Whether to disable cache
    pub no_cache: bool,
    /// If true, print command without executing
    pub dry_run: bool,
}

impl BuildxBuilder {
    /// Create a new BuildxBuilder
    pub fn new() -> Self {
        Self { builder: None }
    }

    /// Set the builder to use
    pub fn with_builder(mut self, builder: impl Into<String>) -> Self {
        self.builder = Some(builder.into());
        self
    }

    /// Check if Docker daemon is running
    pub fn check_docker() -> Result<(), Box<dyn Error + Send>> {
        let output = Command::new("docker")
            .args(["info"])
            .output()
            .map_err(|e| {
                Box::new(std::io::Error::other(format!(
                    "Failed to execute docker: {e}"
                ))) as Box<dyn Error + Send>
            })?;

        if !output.status.success() {
            return Err(Box::new(std::io::Error::other(
                "Docker daemon is not running. Please start Docker.",
            )));
        }

        Ok(())
    }

    /// Check if Docker buildx is available
    pub fn check_buildx() -> Result<(), Box<dyn Error + Send>> {
        let output = Command::new("docker")
            .args(["buildx", "version"])
            .output()
            .map_err(|e| {
                Box::new(std::io::Error::other(format!(
                    "Failed to execute docker buildx: {e}"
                ))) as Box<dyn Error + Send>
            })?;

        if !output.status.success() {
            return Err(Box::new(std::io::Error::other(
                "Docker buildx is not available. Please install buildx.",
            )));
        }

        Ok(())
    }

    /// Build and push a Docker image using buildx
    pub fn build(&self, opts: BuildOptions) -> Result<(), Box<dyn Error + Send>> {
        let full_tag = format!("{}/{}:{}", opts.registry, opts.image_name, opts.tag);

        // Build owned args to avoid lifetime issues
        let mut args: Vec<String> = vec![
            "buildx".to_string(),
            "build".to_string(),
            "--push".to_string(),
        ];

        // Add builder if specified
        if let Some(ref builder) = self.builder {
            args.push("--builder".to_string());
            args.push(builder.clone());
        }

        // Add platforms
        if !opts.platforms.is_empty() {
            args.push("--platform".to_string());
            args.push(opts.platforms.join(","));
        }

        // Add dockerfile
        args.push("--file".to_string());
        args.push(opts.dockerfile.to_string());

        // Add tag
        args.push("--tag".to_string());
        args.push(full_tag.clone());

        // Add no-cache if specified
        if opts.no_cache {
            args.push("--no-cache".to_string());
        }

        // Add context
        args.push(opts.context.to_string());

        if opts.dry_run {
            let escaped_args: Vec<String> = args.iter().map(|s| shell_escape(s)).collect();
            println!("  docker {}", escaped_args.join(" "));
            return Ok(());
        }

        let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let output = self.execute_docker(&args_refs)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Box::new(std::io::Error::other(format!(
                "Build failed for {full_tag}:\n{stderr}"
            ))));
        }

        // Print build output
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.is_empty() {
            println!("{stdout}");
        }

        Ok(())
    }

    /// Print the build command without executing (dry-run mode)
    pub fn dry_run(&self, opts: BuildOptions) -> String {
        let full_tag = format!("{}/{}:{}", opts.registry, opts.image_name, opts.tag);

        let mut args: Vec<String> = vec![
            "buildx".to_string(),
            "build".to_string(),
            "--push".to_string(),
        ];

        if let Some(ref builder) = self.builder {
            args.push("--builder".to_string());
            args.push(builder.clone());
        }

        if !opts.platforms.is_empty() {
            args.push("--platform".to_string());
            args.push(opts.platforms.join(","));
        }

        args.push("--file".to_string());
        args.push(opts.dockerfile.to_string());
        args.push("--tag".to_string());
        args.push(full_tag);

        if opts.no_cache {
            args.push("--no-cache".to_string());
        }

        args.push(opts.context.to_string());

        let escaped_args: Vec<String> = args.iter().map(|s| shell_escape(s)).collect();
        format!("docker {}", escaped_args.join(" "))
    }

    /// Execute docker command and capture output
    fn execute_docker(&self, args: &[&str]) -> Result<Output, Box<dyn Error + Send>> {
        Command::new("docker")
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| {
                Box::new(std::io::Error::other(format!(
                    "Failed to execute docker: {e}"
                ))) as Box<dyn Error + Send>
            })
    }
}

impl Default for BuildxBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buildx_builder_new() {
        let builder = BuildxBuilder::new();
        assert!(builder.builder.is_none());
    }

    #[test]
    fn test_buildx_builder_with_builder() {
        let builder = BuildxBuilder::new().with_builder("my-builder");
        assert_eq!(builder.builder, Some("my-builder".to_string()));
    }

    #[test]
    fn test_dry_run_basic() {
        let builder = BuildxBuilder::new();
        let cmd = builder.dry_run(BuildOptions {
            registry: "ghcr.io/atomicloud",
            image_name: "my-template",
            tag: "v1.0.0",
            dockerfile: "Dockerfile",
            context: ".",
            platforms: &[],
            no_cache: false,
            dry_run: false,
        });

        // Output is shell-escaped, so check for quoted arguments
        assert!(cmd.contains("'buildx'"));
        assert!(cmd.contains("'build'"));
        assert!(cmd.contains("'--push'"));
        assert!(cmd.contains("ghcr.io/atomicloud/my-template:v1.0.0"));
        assert!(cmd.contains("--file"));
        assert!(cmd.contains("'Dockerfile'"));
        assert!(!cmd.contains("--platform"));
        assert!(!cmd.contains("--no-cache"));
    }

    #[test]
    fn test_dry_run_with_platforms() {
        let builder = BuildxBuilder::new();
        let cmd = builder.dry_run(BuildOptions {
            registry: "ghcr.io/atomicloud",
            image_name: "my-template",
            tag: "v1.0.0",
            dockerfile: "Dockerfile",
            context: ".",
            platforms: &["linux/amd64".to_string(), "linux/arm64".to_string()],
            no_cache: false,
            dry_run: false,
        });

        // Platform string is joined with comma and then quoted
        assert!(cmd.contains("--platform"));
        assert!(cmd.contains("linux/amd64,linux/arm64"));
    }

    #[test]
    fn test_dry_run_with_no_cache() {
        let builder = BuildxBuilder::new();
        let cmd = builder.dry_run(BuildOptions {
            registry: "ghcr.io/atomicloud",
            image_name: "my-template",
            tag: "v1.0.0",
            dockerfile: "Dockerfile",
            context: ".",
            platforms: &[],
            no_cache: true,
            dry_run: false,
        });

        assert!(cmd.contains("--no-cache"));
    }

    #[test]
    fn test_dry_run_with_builder() {
        let builder = BuildxBuilder::new().with_builder("multi-arch");
        let cmd = builder.dry_run(BuildOptions {
            registry: "ghcr.io/atomicloud",
            image_name: "my-template",
            tag: "v1.0.0",
            dockerfile: "Dockerfile",
            context: ".",
            platforms: &[],
            no_cache: false,
            dry_run: false,
        });

        // Builder option and value are quoted separately
        assert!(cmd.contains("--builder"));
        assert!(cmd.contains("multi-arch"));
    }
}
