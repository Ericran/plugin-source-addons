//! plugins_meta.json — per-plugin comments and groups, keyed by plugin file
//! stem (`funcommands` for funcommands.smx). Panel-owned, but parsing
//! preserves any fields this plugin does not understand so a future writer
//! (or a hand edit) is not clobbered.

use serde_json::{Map, Value};

#[derive(Debug, Default, Clone)]
pub struct Manifest {
    root: Map<String, Value>,
}

impl Manifest {
    /// Lossy parse: a malformed or non-object file yields an empty manifest
    /// rather than blocking every operation on the tab.
    pub fn parse(bytes: &[u8]) -> Manifest {
        let root = serde_json::from_slice::<Value>(bytes)
            .ok()
            .and_then(|value| match value {
                Value::Object(map) => Some(map),
                _ => None,
            })
            .unwrap_or_default();
        Manifest { root }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = serde_json::to_vec_pretty(&Value::Object(self.root.clone()))
            .unwrap_or_else(|_| b"{}".to_vec());
        out.push(b'\n');
        out
    }

    /// Case-insensitive entry lookup returning the canonical key.
    fn find_key(&self, name: &str) -> Option<String> {
        if self.root.contains_key(name) {
            return Some(name.to_string());
        }
        self.root
            .keys()
            .find(|key| key.eq_ignore_ascii_case(name))
            .cloned()
    }

    pub fn contains(&self, name: &str) -> bool {
        self.find_key(name).is_some()
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.root.keys().map(String::as_str)
    }

    fn field(&self, name: &str, field: &str) -> Option<String> {
        let key = self.find_key(name)?;
        let text = self.root.get(&key)?.get(field)?.as_str()?.trim();
        (!text.is_empty()).then(|| text.to_string())
    }

    pub fn comment(&self, name: &str) -> Option<String> {
        self.field(name, "Comment")
    }

    pub fn group(&self, name: &str) -> Option<String> {
        self.field(name, "Group")
    }

    /// Ensures an entry exists.
    pub fn ensure(&mut self, name: &str) {
        let key = self.find_key(name).unwrap_or_else(|| name.to_string());
        let entry = self
            .root
            .entry(key)
            .or_insert_with(|| Value::Object(Map::new()));
        if !entry.is_object() {
            *entry = Value::Object(Map::new());
        }
        if let Some(map) = entry.as_object_mut() {
            map.entry("Comment").or_insert_with(|| Value::String(String::new()));
            map.entry("Group").or_insert_with(|| Value::String(String::new()));
        }
    }

    fn set_field(&mut self, name: &str, field: &str, value: Option<&str>) {
        self.ensure(name);
        let Some(key) = self.find_key(name) else {
            return;
        };
        if let Some(Value::Object(map)) = self.root.get_mut(&key) {
            map.insert(
                field.to_string(),
                Value::String(value.unwrap_or_default().to_string()),
            );
        }
    }

    pub fn set_comment(&mut self, name: &str, comment: Option<&str>) {
        self.set_field(name, "Comment", comment);
    }

    pub fn set_group(&mut self, name: &str, group: Option<&str>) {
        self.set_field(name, "Group", group);
    }

    /// Removes an entry; returns whether it existed.
    pub fn remove(&mut self, name: &str) -> bool {
        match self.find_key(name) {
            Some(key) => self.root.remove(&key).is_some(),
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_manifest_shape() {
        let json = br#"{
            "sourcebans": { "Comment": "shared ban list", "Group": "admin" },
            "funcommands": { "Comment": "", "Group": "" }
        }"#;
        let manifest = Manifest::parse(json);
        assert_eq!(manifest.comment("sourcebans").as_deref(), Some("shared ban list"));
        assert_eq!(manifest.group("sourcebans").as_deref(), Some("admin"));
        assert_eq!(manifest.comment("funcommands"), None);
        assert!(manifest.contains("SourceBans"));
        assert!(!manifest.contains("unknown"));
    }

    #[test]
    fn malformed_input_is_empty() {
        assert_eq!(Manifest::parse(b"not json").names().count(), 0);
        assert_eq!(Manifest::parse(b"[1,2]").names().count(), 0);
        assert_eq!(Manifest::parse(b"").names().count(), 0);
    }

    #[test]
    fn set_and_remove_roundtrip() {
        let mut manifest = Manifest::parse(b"{}");
        manifest.set_comment("rtd", Some("hi"));
        manifest.set_group("rtd", Some("fun"));
        let reparsed = Manifest::parse(&manifest.to_bytes());
        assert_eq!(reparsed.comment("rtd").as_deref(), Some("hi"));
        assert_eq!(reparsed.group("rtd").as_deref(), Some("fun"));

        let mut manifest = reparsed;
        assert!(manifest.remove("RTD"));
        assert!(!manifest.remove("rtd"));
    }

    #[test]
    fn preserves_unknown_fields() {
        let json = br#"{"rtd": {"Comment": "x", "Group": "", "Pinned": true}}"#;
        let mut manifest = Manifest::parse(json);
        manifest.set_comment("rtd", Some("y"));
        let out = String::from_utf8(manifest.to_bytes()).expect("utf8");
        assert!(out.contains("\"Pinned\": true"));
    }
}
