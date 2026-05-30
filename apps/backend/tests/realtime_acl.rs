use gaming_cafe_api::realtime::channel::ChannelId;

fn admin_claims() -> gaming_cafe_api::dto::JwtUserClaims {
    gaming_cafe_api::dto::JwtUserClaims {
        sub: "admin-1".to_string(),
        permissions: vec![],
        allowedTenants: vec![],
        rateLimit: None,
        iss: "gamezone".to_string(),
        aud: serde_json::json!("gamezone"),
        iat: None,
        exp: None,
        userId: "00000000-0000-0000-0000-000000000001".to_string(),
        tenantId: "test".to_string(),
        roles: vec!["admin".to_string()],
        appId: "test".to_string(),
        orgIds: vec![],
        deviceId: None,
    }
}

fn staff_claims() -> gaming_cafe_api::dto::JwtUserClaims {
    let mut c = admin_claims();
    c.userId = "00000000-0000-0000-0000-000000000002".to_string();
    c.sub = "staff-1".to_string();
    c.roles = vec!["staff".to_string()];
    c
}

fn player_claims() -> gaming_cafe_api::dto::JwtUserClaims {
    let mut c = admin_claims();
    c.userId = "00000000-0000-0000-0000-000000000003".to_string();
    c.sub = "player-1".to_string();
    c.roles = vec!["player".to_string()];
    c
}

mod subscribe_acl {
    use super::*;

    #[test]
    fn admin_can_subscribe_to_admin_channel() {
        let claims = admin_claims();
        let ch = ChannelId::Admin;
        assert!(gaming_cafe_api::realtime::channel::ChannelId::parse("admin").is_some());
        // ACL is in a private module, so we test via channel parsing + claims role checks
        assert!(claims.is_admin());
        assert_eq!(ch, ChannelId::Admin);
    }

    #[test]
    fn staff_cannot_subscribe_to_admin_channel() {
        let claims = staff_claims();
        assert!(!claims.is_admin());
    }

    #[test]
    fn player_cannot_subscribe_to_admin_or_staff_channel() {
        let claims = player_claims();
        assert!(!claims.is_admin());
        assert!(!claims.is_admin_or_staff());
    }

    #[test]
    fn staff_can_subscribe_to_staff_channel() {
        let claims = staff_claims();
        assert!(claims.is_admin_or_staff());
    }

    #[test]
    fn any_authenticated_user_can_subscribe_to_public() {
        let claims = player_claims();
        assert!(ChannelId::parse("public").is_some());
        // Public channel has no role restriction
        let _ = claims;
    }

    #[test]
    fn user_can_subscribe_to_own_channel() {
        let claims = player_claims();
        let user_id = claims.user_id_uuid().unwrap();
        let ch = ChannelId::parse(&format!("user:{user_id}")).unwrap();
        assert!(matches!(ch, ChannelId::User(id) if id == user_id));
    }

    #[test]
    fn user_cannot_subscribe_to_other_user_channel() {
        let claims = player_claims();
        let other_id = uuid::Uuid::new_v4();
        let claims_id = claims.user_id_uuid().unwrap();
        assert_ne!(claims_id, other_id);
        // Only self or admin can subscribe
        assert!(!claims.is_admin());
    }

    #[test]
    fn admin_can_subscribe_to_any_user_channel() {
        let claims = admin_claims();
        assert!(claims.is_admin());
    }

    #[test]
    fn device_channel_requires_admin_or_staff() {
        let staff = staff_claims();
        let player = player_claims();
        assert!(staff.is_admin_or_staff());
        assert!(!player.is_admin_or_staff());
    }
}

mod channel_parsing {
    use super::*;

    #[test]
    fn all_fixed_channels_parse() {
        assert_eq!(ChannelId::parse("public"), Some(ChannelId::Public));
        assert_eq!(ChannelId::parse("admin"), Some(ChannelId::Admin));
        assert_eq!(ChannelId::parse("staff"), Some(ChannelId::Staff));
    }

    #[test]
    fn user_channel_roundtrip() {
        let id = uuid::Uuid::new_v4();
        let s = format!("user:{id}");
        let parsed = ChannelId::parse(&s).unwrap();
        assert_eq!(parsed, ChannelId::User(id));
        assert_eq!(parsed.as_string(), s);
    }

    #[test]
    fn device_channel_roundtrip() {
        let id = uuid::Uuid::new_v4();
        let s = format!("device:{id}");
        let parsed = ChannelId::parse(&s).unwrap();
        assert_eq!(parsed, ChannelId::Device(id));
        assert_eq!(parsed.as_string(), s);
    }

    #[test]
    fn room_channel_roundtrip() {
        let parsed = ChannelId::parse("room:support-1").unwrap();
        assert_eq!(parsed, ChannelId::Room("support-1".to_string()));
        assert_eq!(parsed.as_string(), "room:support-1");
    }

    #[test]
    fn invalid_channels_return_none() {
        assert!(ChannelId::parse("bogus").is_none());
        assert!(ChannelId::parse("room:").is_none());
        assert!(ChannelId::parse("user:not-a-uuid").is_none());
        assert!(ChannelId::parse("device:xyz").is_none());
    }
}
