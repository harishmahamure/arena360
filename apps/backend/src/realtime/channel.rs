use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ChannelId {
    Public,
    Admin,
    Staff,
    User(Uuid),
    Device(Uuid),
    Room(String),
}

impl ChannelId {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "public" => Some(Self::Public),
            "admin" => Some(Self::Admin),
            "staff" => Some(Self::Staff),
            _ => {
                if let Some(id_str) = raw.strip_prefix("user:") {
                    Uuid::parse_str(id_str).ok().map(Self::User)
                } else if let Some(id_str) = raw.strip_prefix("device:") {
                    Uuid::parse_str(id_str).ok().map(Self::Device)
                } else if let Some(name) = raw.strip_prefix("room:") {
                    if name.is_empty() {
                        None
                    } else {
                        Some(Self::Room(name.to_string()))
                    }
                } else {
                    None
                }
            }
        }
    }

    pub fn as_string(&self) -> String {
        match self {
            Self::Public => "public".to_string(),
            Self::Admin => "admin".to_string(),
            Self::Staff => "staff".to_string(),
            Self::User(id) => format!("user:{id}"),
            Self::Device(id) => format!("device:{id}"),
            Self::Room(name) => format!("room:{name}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_fixed_channels() {
        assert_eq!(ChannelId::parse("public"), Some(ChannelId::Public));
        assert_eq!(ChannelId::parse("admin"), Some(ChannelId::Admin));
        assert_eq!(ChannelId::parse("staff"), Some(ChannelId::Staff));
    }

    #[test]
    fn parse_user_channel() {
        let id = Uuid::new_v4();
        let raw = format!("user:{id}");
        assert_eq!(ChannelId::parse(&raw), Some(ChannelId::User(id)));
    }

    #[test]
    fn parse_device_channel() {
        let id = Uuid::new_v4();
        let raw = format!("device:{id}");
        assert_eq!(ChannelId::parse(&raw), Some(ChannelId::Device(id)));
    }

    #[test]
    fn parse_room_channel() {
        assert_eq!(
            ChannelId::parse("room:support-1"),
            Some(ChannelId::Room("support-1".to_string()))
        );
    }

    #[test]
    fn parse_invalid() {
        assert_eq!(ChannelId::parse("unknown"), None);
        assert_eq!(ChannelId::parse("room:"), None);
        assert_eq!(ChannelId::parse("user:not-a-uuid"), None);
    }

    #[test]
    fn roundtrip() {
        let cases = vec![
            ChannelId::Public,
            ChannelId::Admin,
            ChannelId::Staff,
            ChannelId::User(Uuid::new_v4()),
            ChannelId::Device(Uuid::new_v4()),
            ChannelId::Room("test-room".to_string()),
        ];
        for ch in cases {
            assert_eq!(ChannelId::parse(&ch.as_string()), Some(ch));
        }
    }
}
