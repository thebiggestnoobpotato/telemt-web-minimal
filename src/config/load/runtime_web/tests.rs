use base64::Engine as _;

use super::*;

#[test]
fn capability_matches_reference_vectors() {
    let secret = hex::decode("000102030405060708090a0b0c0d0e0f").unwrap();
    let mut dd_secret = vec![0xdd];
    dd_secret.extend_from_slice(&secret);
    for (client_secret, base_path, expected) in [
        (
            secret.as_slice(),
            b"".as_slice(),
            "MHLEY5PmW1GWqJkSrlmJpvJUiLhBH_QKy6yKg8a0JPk",
        ),
        (
            dd_secret.as_slice(),
            b"".as_slice(),
            "IpJrt3e7sKtzPyoXy6w-Zj6GGEvsvclN66JzQEfPYLA",
        ),
        (
            secret.as_slice(),
            b"dobry-cola-super-app".as_slice(),
            "hHz99Xs93EN1j91G9gpNepXwGNNt5YdAFkEVk_LlqdQ",
        ),
        (
            dd_secret.as_slice(),
            b"dobry-cola-super-app".as_slice(),
            "TGUkZaevsavLbHvlNWipnRoYxgzZ51ioWvbxgGT3wHo",
        ),
    ] {
        let capability =
            derive_web_capability(client_secret, b"proxy.example.com", base_path).unwrap();
        assert_eq!(
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(capability),
            expected
        );
    }
}

#[test]
fn capability_binds_the_exact_host_and_base_path_identity() {
    let secret = hex::decode("000102030405060708090a0b0c0d0e0f").unwrap();
    let root = derive_web_capability(&secret, b"proxy.example.com", b"").unwrap();
    let mixed = derive_web_capability(&secret, b"proxy.example.com", b"MixedCase/path").unwrap();
    let lower = derive_web_capability(&secret, b"proxy.example.com", b"mixedcase/path").unwrap();
    let other_path =
        derive_web_capability(&secret, b"proxy.example.com", b"MixedCase/other").unwrap();
    let other_host =
        derive_web_capability(&secret, b"other.example.com", b"MixedCase/path").unwrap();

    let identities = [root, mixed, lower, other_path, other_host]
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(identities.len(), 5);
}

