use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolverVersionPrincipalRes {
    pub id: String,
    pub version: i64,
    pub created_at: String,
    pub description: String,
    pub docker_reference: String,
    pub docker_tag: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolverVersionRes {
    pub principal: ResolverVersionPrincipalRes,
    pub resolver: ResolverRes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolverRes {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolver_version_principal_res_deserialization() {
        let json = r#"{
            "id": "abc123",
            "version": 1,
            "createdAt": "2024-01-01T00:00:00Z",
            "description": "Initial version",
            "dockerReference": "atomi/json-merger",
            "dockerTag": "1.0.0"
        }"#;

        let res: ResolverVersionPrincipalRes =
            serde_json::from_str(json).expect("Deserialization should succeed");

        assert_eq!(res.id, "abc123");
        assert_eq!(res.version, 1);
        assert_eq!(res.description, "Initial version");
        assert_eq!(res.docker_reference, "atomi/json-merger");
        assert_eq!(res.docker_tag, "1.0.0");
    }

    #[test]
    fn test_resolver_res_deserialization() {
        let json = r##"{
            "id": "resolver-123",
            "name": "json-merger",
            "project": "atomi",
            "source": "github.com/atomi/resolvers",
            "email": "dev@atomi.com",
            "tags": ["json", "merge"],
            "description": "Deep merge JSON files",
            "readme": "# JSON Merger",
            "userId": "user-456"
        }"##;

        let res: ResolverRes = serde_json::from_str(json).expect("Deserialization should succeed");

        assert_eq!(res.id, "resolver-123");
        assert_eq!(res.name, "json-merger");
        assert_eq!(res.project, "atomi");
        assert_eq!(res.tags, vec!["json", "merge"]);
        assert_eq!(res.user_id, "user-456");
        assert_eq!(res.readme, "# JSON Merger");
    }
}
