//! The curator's JSON schema is generated from these types; check it matches serde's shape.

use sunset_domain::ProposedChange;

#[test]
fn proposed_change_schema_uses_serde_names() {
    let schema = serde_json::to_string(&schemars::schema_for!(ProposedChange)).unwrap();
    for name in [
        "\"op\"",
        "upsert_concept",
        "bodyMd",
        "add_visual",
        "concept_flow",
        "IS_A",
        "\"kind\"",
    ] {
        assert!(schema.contains(name), "schema lacks {name}: {schema}");
    }
    assert!(!schema.contains("body_md"), "fields must be camelCase");
}
