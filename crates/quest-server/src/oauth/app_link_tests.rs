use super::*;

const RELEASE: &str = "1F:2E:3D:4C:5B:6A:79:88:97:A6:B5:C4:D3:E2:F1:00:0F:1E:2D:3C:4B:5A:69:78:87:96:A5:B4:C3:D2:E1:F0";

#[test]
fn fingerprints_are_32_hex_bytes_stored_in_upper_case() {
    let parsed: CertFingerprint = RELEASE.to_ascii_lowercase().parse().unwrap();
    assert_eq!(parsed.to_string(), RELEASE);
    assert!(" AB:CD ".parse::<CertFingerprint>().is_err());
    assert!(RELEASE.replace(':', "").parse::<CertFingerprint>().is_err());
    assert!(RELEASE.replacen("4B", "4G", 1).parse::<CertFingerprint>().is_err());
    assert!(format!("{RELEASE}:00").parse::<CertFingerprint>().is_err());
    assert!("".parse::<CertFingerprint>().is_err());
}

#[test]
fn asset_links_name_the_android_package_and_its_certificates() {
    let fingerprints = [RELEASE.parse().unwrap()];
    assert_eq!(
        asset_links(&fingerprints),
        json!([{
            "relation": ["delegate_permission/common.handle_all_urls"],
            "target": {
                "namespace": "android_app",
                "package_name": "app.quest.android",
                "sha256_cert_fingerprints": [RELEASE],
            },
        }])
    );
}
