use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Connection info for a host
/// Can be:
/// - Some("localhost") for local rebuilds
/// - Some("user@host") or Some("host") for remote rebuilds
/// - None for unconfigured hosts (serialized as empty array [])
#[derive(Debug, Clone, PartialEq)]
pub enum Connection {
    Local,
    Remote(String),
    Unconfigured,
}

impl Connection {
    pub fn is_configured(&self) -> bool {
        !matches!(self, Connection::Unconfigured)
    }

    pub fn display(&self) -> String {
        match self {
            Connection::Local => "localhost".to_string(),
            Connection::Remote(s) => s.clone(),
            Connection::Unconfigured => "[]".to_string(),
        }
    }
}

impl Serialize for Connection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Connection::Local => serializer.serialize_str("localhost"),
            Connection::Remote(s) => serializer.serialize_str(s),
            Connection::Unconfigured => {
                use serde::ser::SerializeSeq;
                let seq = serializer.serialize_seq(Some(0))?;
                seq.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for Connection {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum ConnectionHelper {
            String(String),
            Array(()),
        }

        match ConnectionHelper::deserialize(deserializer)? {
            ConnectionHelper::String(s) => {
                if s == "localhost" {
                    Ok(Connection::Local)
                } else {
                    Ok(Connection::Remote(s))
                }
            }
            ConnectionHelper::Array(_) => Ok(Connection::Unconfigured),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostConfig {
    pub connection: Connection,
    #[serde(default)]
    pub extra_args: Vec<String>,
}

impl HostConfig {
    pub fn unconfigured() -> Self {
        Self {
            connection: Connection::Unconfigured,
            extra_args: Vec::new(),
        }
    }

    pub fn local() -> Self {
        Self {
            connection: Connection::Local,
            extra_args: Vec::new(),
        }
    }
}
