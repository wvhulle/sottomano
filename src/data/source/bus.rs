use std::fmt::{Display, Formatter, Result as FmtResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Connect(String),
    Call(String),
    Absent,
    Panicked,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::Connect(detail) => write!(f, "session bus: {detail}"),
            Error::Call(detail) => write!(f, "{detail}"),
            Error::Absent => write!(f, "service not running"),
            Error::Panicked => write!(f, "bus call panicked"),
        }
    }
}

pub fn service_absent(error: &zbus::Error) -> bool {
    let zbus::Error::MethodError(name, _, _) = error else {
        return false;
    };
    matches!(
        name.as_str(),
        "org.freedesktop.DBus.Error.ServiceUnknown"
            | "org.freedesktop.DBus.Error.NameHasNoOwner"
            | "org.freedesktop.DBus.Error.NoReply"
    )
}

pub fn call<T>(result: zbus::Result<T>) -> Result<Option<T>, Error> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(error) if service_absent(&error) => Ok(None),
        Err(error) => Err(Error::Call(error.to_string())),
    }
}

pub fn require<T>(result: zbus::Result<T>) -> Result<T, Error> {
    call(result)?.ok_or(Error::Absent)
}
