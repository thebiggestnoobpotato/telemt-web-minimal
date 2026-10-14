#[cfg(test)]
mod tests {
    use crate::api::model::PatchUserRequest;

    #[test]
    fn patch_user_request_deserializes_secret() {
        let raw = r#"{
            "secret": "00112233445566778899aabbccddeeff"
        }"#;
        let req: PatchUserRequest = serde_json::from_str(raw).expect("valid json");
        assert_eq!(
            req.secret.as_deref(),
            Some("00112233445566778899aabbccddeeff")
        );
    }
}
