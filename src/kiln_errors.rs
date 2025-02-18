
#[derive(Debug, Clone)]
pub(super) enum KilnErrType {
    FileNotFound,
    TomlParseError,
    Anyhow,
    Unkown,
}

#[derive(Debug, Clone)]
pub(super) struct KilnError {
    err_type: KilnErrType,
    msg: String,
}

pub type KilnResult<T> = Result<T, KilnError>;


impl KilnError {
    pub(super) fn new_unknown(msg: impl Into<String>) -> Self {
        KilnError {
            err_type: KilnErrType::Unkown,
            msg: msg.into(),
        }
    }
}

impl From<std::io::Error> for KilnError {
    fn from(error: std::io::Error) -> Self {
        KilnError {
            err_type: KilnErrType::FileNotFound,
            msg: format!("{:?}", error),
        }
    }
}

impl From<toml::de::Error> for KilnError {
    fn from(error: toml::de::Error) -> Self {
        KilnError {
            err_type: KilnErrType::TomlParseError,
            msg: format!("{:?}", error),
        }
    }
}

impl From<anyhow::Error> for KilnError {
    fn from(error: anyhow::Error) -> Self {
        KilnError {
            err_type: KilnErrType::Anyhow,
            msg: format!("{:?}", error),
        }
    }
}
