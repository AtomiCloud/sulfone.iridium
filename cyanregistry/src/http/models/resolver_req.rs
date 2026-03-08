use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolverReq {
    pub name: String,
    pub project: String,
    pub source: String,
    pub email: String,
    pub tags: Vec<String>,
    pub description: String,
    pub readme: String,
    pub version_description: String,
    pub docker_reference: String,
    pub docker_tag: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolver_req_serialization_camel_case() {
        let req = ResolverReq {
            name: "json-merger".to_string(),
            project: "atomi".to_string(),
            source: "github.com/atomi/resolvers".to_string(),
            email: "dev@atomi.com".to_string(),
            tags: vec!["json".to_string(), "merge".to_string()],
            description: "Deep merge JSON files".to_string(),
            readme: "JSON Merger".to_string(),
            version_description: "Initial version".to_string(),
            docker_reference: "atomi/json-merger".to_string(),
            docker_tag: "1.0.0".to_string(),
        };

        let json = serde_json::to_string(&req).expect("Serialization should succeed");

        // Verify camelCase serialization
        assert!(
            json.contains("\"name\":\"json-merger\""),
            "name should be camelCase"
        );
        assert!(
            json.contains("\"dockerReference\":\"atomi/json-merger\""),
            "dockerReference should be camelCase"
        );
        assert!(
            json.contains("\"dockerTag\":\"1.0.0\""),
            "dockerTag should be camelCase"
        );
        assert!(
            json.contains("\"versionDescription\":\"Initial version\""),
            "versionDescription should be camelCase"
        );
    }

    #[test]
    fn test_resolver_req_deserialization() {
        let json = r##"{
            "name": "json-merger",
            "project": "atomi",
            "source": "github.com/atomi/resolvers",
            "email": "dev@atomi.com",
            "tags": ["json", "merge"],
            "description": "Deep merge JSON files",
            "readme": "# JSON Merger",
            "versionDescription": "Initial version",
            "dockerReference": "atomi/json-merger",
            "dockerTag": "1.0.0"
        }"##;

        let req: ResolverReq = serde_json::from_str(json).expect("Deserialization should succeed");

        assert_eq!(req.name, "json-merger");
        assert_eq!(req.project, "atomi");
        assert_eq!(req.docker_reference, "atomi/json-merger");
        assert_eq!(req.docker_tag, "1.0.0");
        assert_eq!(req.tags, vec!["json", "merge"]);
        assert_eq!(req.readme, "# JSON Merger");
    }
}
