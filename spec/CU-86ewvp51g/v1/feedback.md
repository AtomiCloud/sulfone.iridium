# Feedback for v1

## Issue 1: Build registry should be separate from image name

The current implementation concatenates the artifact name directly to the registry path, but the `build.registry` field should be just the registry (e.g., `kirinnee` for Docker Hub).

The image name should be specified separately in the `images:` section:

```yaml
build:
  registry: kirinnee # Just the registry, not the full image path
  platforms:
    - linux/amd64
  images:
    plugin:
      image: plugin2 # Image name, will be concatenated: kirinnee/plugin2
      dockerfile: Dockerfile
      context: .
```

Expected behavior: `registry + "/" + image` = `kirinnee/plugin2`

## Issue 2: Build and push should target a specific folder

Currently the build command builds from the context directory. Add support for targeting a specific folder:

```yaml
build:
  registry: kirinnee
  platforms:
    - linux/amd64
  images:
    plugin:
      image: plugin2
      dockerfile: Dockerfile
      context: .
      target: ./some-folder # Optional: specific folder to target
```

This would allow more fine-grained control over what gets built.
