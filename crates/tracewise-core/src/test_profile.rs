use super::*;

#[test]
fn test_profile_defaults_to_core() {
    let p = ProfileConfig::default();
    assert!(p.core);
    assert!(!p.coordination);
    assert!(!p.policy);
    assert!(!p.strict);
    assert!(!p.share);
    assert_eq!(p.description(), "core");
}

#[test]
fn test_profile_full() {
    let p = ProfileConfig::full();
    assert!(p.is_full());
    assert_eq!(p.description(), "full");
}

#[test]
fn test_parse_trailconfig_profile() {
    let content = r#"
[profile]
coordination = true
share = yes
"#;
    let mut p = ProfileConfig::core();
    ProfileConfig::parse_trailconfig_content(content, &mut p);
    assert!(p.coordination);
    assert!(p.share);
    assert!(!p.policy);
    assert!(!p.strict);
    assert_eq!(p.description(), "core + coordination + share");
}

#[test]
fn test_parse_trailconfig_name_full() {
    let content = r#"
[profile]
name = "full"
share = false
"#;
    let mut p = ProfileConfig::core();
    ProfileConfig::parse_trailconfig_content(content, &mut p);
    assert!(p.coordination);
    assert!(p.policy);
    assert!(p.strict);
    assert!(!p.share);
    assert_eq!(p.description(), "core + coordination + policy + strict");
}
