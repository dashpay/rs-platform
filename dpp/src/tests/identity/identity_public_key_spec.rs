mod from_raw_object {
    use serde_json::json;

    use crate::identity::{KeyType, Purpose, SecurityLevel};
    use crate::prelude::IdentityPublicKey;

    #[test]
    pub fn should_parse_raw_key() {
        let public_key_json = json!({
            "id": 0,
            "type": 0,
            "purpose": 0,
            "securityLevel": 0,
            "data": "AuryIuMtRrl/VviQuyLD1l4nmxi9ogPzC9LT7tdpo0di",
            "readOnly": false
        });

        let public_key = IdentityPublicKey::from_json_object(public_key_json).unwrap();

        assert_eq!(public_key.id, 0);
        assert_eq!(public_key.key_type, KeyType::ECDSA_SECP256K1);
        assert_eq!(public_key.purpose, Purpose::AUTHENTICATION);
        assert_eq!(public_key.security_level, SecurityLevel::MASTER);
        assert_eq!(public_key.read_only, false);
        assert_eq!(
            public_key.data,
            [
                2, 234, 242, 34, 227, 45, 70, 185, 127, 86, 248, 144, 187, 34, 195, 214, 94, 39,
                155, 24, 189, 162, 3, 243, 11, 210, 211, 238, 215, 105, 163, 71, 98
            ]
        );
    }

    #[test]
    pub fn should_parse_key_of_withdraw_purpose() {
        let public_key_json = json!({
            "id": 0,
            "type": 0,
            "purpose": 3,
            "securityLevel": 0,
            "data": "AuryIuMtRrl/VviQuyLD1l4nmxi9ogPzC9LT7tdpo0di",
            "readOnly": false
        });

        let public_key = IdentityPublicKey::from_json_object(public_key_json).unwrap();

        assert_eq!(public_key.id, 0);
        assert_eq!(public_key.key_type, KeyType::ECDSA_SECP256K1);
        assert_eq!(public_key.purpose, Purpose::WITHDRAW);
        assert_eq!(public_key.security_level, SecurityLevel::MASTER);
        assert!(!public_key.read_only);
    }

    #[test]
    #[ignore = "not sure how the test should work"]
    pub fn should_parse_key_object_from_dpp() {
        let pk_str = "{\"id\":0,\"type\":0,\"data\":{\"0\":97,\"1\":97,\"2\":97,\"3\":97,\"4\":97,\"5\":97,\"6\":97,\"7\":97,\"8\":97,\"9\":97,\"10\":97,\"11\":97,\"12\":97,\"13\":97,\"14\":97,\"15\":97,\"16\":97,\"17\":97,\"18\":97,\"19\":97,\"20\":97,\"21\":97,\"22\":97,\"23\":97,\"24\":97,\"25\":97,\"26\":97,\"27\":97,\"28\":97,\"29\":97,\"30\":97,\"31\":97,\"32\":97,\"33\":97,\"34\":97,\"35\":97}";
        let public_key: IdentityPublicKey = serde_json::from_str(pk_str).unwrap();

        // let public_key = IdentityPublicKey::from_raw_object(&public_key_json).unwrap();

        assert_eq!(public_key.id, 0);
        assert_eq!(public_key.key_type, KeyType::ECDSA_SECP256K1);
        assert_eq!(public_key.purpose, Purpose::AUTHENTICATION);
        assert_eq!(public_key.security_level, SecurityLevel::MASTER);
        assert_eq!(public_key.read_only, false);
        assert_eq!(
            public_key.data,
            [
                2, 234, 242, 34, 227, 45, 70, 185, 127, 86, 248, 144, 187, 34, 195, 214, 94, 39,
                155, 24, 189, 162, 3, 243, 11, 210, 211, 238, 215, 105, 163, 71, 98
            ]
        );
    }

    #[test]
    pub fn should_return_data_in_case_bip13_script_hash() {
        let public_key_json = json!({
            "id": 0,
            "type": KeyType::BIP13_SCRIPT_HASH,
            "purpose": Purpose::AUTHENTICATION,
            "securityLevel": SecurityLevel::MASTER,
            "data": "AuryIuMtRrl/VviQuyLD1l4nmxi9ogPzC9LT7tdpo0di",
            "readOnly": false
        });

        let public_key = IdentityPublicKey::from_json_object(public_key_json)
            .expect("the public key should be created");
        assert_eq!(public_key.get_type(), KeyType::BIP13_SCRIPT_HASH);
        assert_eq!(
            public_key.hash().unwrap(),
            [
                2, 234, 242, 34, 227, 45, 70, 185, 127, 86, 248, 144, 187, 34, 195, 214, 94, 39,
                155, 24, 189, 162, 3, 243, 11, 210, 211, 238, 215, 105, 163, 71, 98
            ]
        );
    }

    #[test]
    pub fn should_return_true_if_public_key_is_master() {
        let public_key_json = json!({
            "id": 0,
            "type": KeyType::BIP13_SCRIPT_HASH,
            "purpose": Purpose::AUTHENTICATION,
            "securityLevel": SecurityLevel::MASTER,
            "data": "AuryIuMtRrl/VviQuyLD1l4nmxi9ogPzC9LT7tdpo0di",
            "readOnly": false
        });

        let public_key = IdentityPublicKey::from_json_object(public_key_json)
            .expect("the public key should be created");
        assert!(public_key.is_master());
    }

    #[test]
    pub fn should_return_false_if_public_key_is_not_master() {
        let public_key_json = json!({
            "id": 0,
            "type": KeyType::BIP13_SCRIPT_HASH,
            "purpose": Purpose::AUTHENTICATION,
            "securityLevel": SecurityLevel::CRITICAL,

            "data": "AuryIuMtRrl/VviQuyLD1l4nmxi9ogPzC9LT7tdpo0di",
            "readOnly": false
        });

        let public_key = IdentityPublicKey::from_json_object(public_key_json)
            .expect("the public key should be created");

        assert!(!public_key.is_master());
    }
}
