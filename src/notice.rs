//! Data structures for marshaling to honeybadger's API
use error_chain::ChainedError;
use failure;

use std::collections::HashMap;
use std::convert::From;

/// Serializable root notice event, for use with the notify endpoint of the Honeybadger API.
#[derive(Serialize)]
pub struct Notice<'req> {
    pub api_key: &'req str,
    pub notifier: Notifier,
    pub error: Error,
    pub request: Request<'req>,
    pub server: Server<'req>,
}

/// Serializable leaf node representing the error to notify on.
#[derive(Serialize)]
pub struct Error {
    pub class: String,
    pub message: Option<String>,
    pub causes: Option<Vec<Error>>,
}

/// Implementation of the `From` trait for `failure::Error`, which allows bastic failure
/// functionality to be used with the `Honeybadger::into_payload` API, to marshal a payload for
/// Honeybadger's Exceptions API. 
impl From<failure::Error> for Error {
    fn from(error: failure::Error) -> Error {
        Error {
            class: format!("{}", error),
            message: Some(format!("{:?}", error)),
            causes: Some(
                error
                    .iter_causes()
                    .map(|cause| Error {
                        class: format!("{}", cause),
                        message: Some(format!("{:?}", cause)),
                        causes: None,
                    })
                    .collect(),
            ),
        }
    }
}

impl From<&failure::Error> for Error {
    fn from(error: &failure::Error) -> Error {
        Error {
            class: format!("{}", error),
            message: Some(format!("{:?}", error)),
            causes: Some(
                error
                    .iter_causes()
                    .map(|cause| Error {
                        class: format!("{}", cause),
                        message: Some(format!("{:?}", cause)),
                        causes: None,
                    })
                    .collect(),
            ),
        }
    }
}

impl From<Box<std::error::Error>> for Error {
    fn from(error: Box<std::error::Error>) -> Error {
        Error {
            class: format!("{}", error),
            message: Some(format!("{:?}", error)),
            causes: None,
        }
    }
}

impl Error {
    /// Internal API to create a new Error instance for serialization purposes.
    pub fn new<E>(error: &E) -> Error
    where
        E: ChainedError,
    {
        Error {
            class: error.description().to_string(),
            message: Some(error.display_chain().to_string()),
            causes: Some(error.iter().map(|cause| Error::std_err(cause)).collect()),
        }
    }

    fn std_err(error: &::std::error::Error) -> Error {
        Error {
            class: error.description().to_string(),
            message: None,
            causes: error.cause().map(|cause| vec![Error::std_err(cause)]),
        }
    }
}

/// Serializable leaf node representing the meta details on this crate
#[derive(Serialize)]
pub struct Notifier {
    pub name: &'static str,
    pub url: &'static str,
    pub version: &'static str,
}

/// Leaf node containing the context hash and environment at the time of
/// serialization.
#[derive(Serialize)]
pub struct Request<'req> {
    pub context: Option<HashMap<&'req str, &'req str>>,
    pub cgi_data: HashMap<String, String>,
}

/// Leaf node containing OS system information at the time of serialization
#[derive(Serialize)]
pub struct Server<'req> {
    pub project_root: &'req str,
    pub environment_name: &'req str,
    pub hostname: &'req str,
    pub time: u64,
    pub pid: u32,
}

#[cfg(test)]
mod tests {

    use errors::*;
    use failure;
    use notice;


    #[test]
    fn test_chained_err() {
        let error : Result<()> = Err(ErrorKind::RedirectionError.into());
        let chain = error.chain_err(|| ErrorKind::RateExceededError);
        let notice = ::notice::Error::new(&chain.unwrap_err());

        assert_eq!("Honeybadger rate limit exceeded", notice.class);
        if let Some(causes) = notice.causes {
            assert_eq!(2, causes.len());
        } else {
            assert_eq!("", "Missing causes in ::notice::Error");
        }
    }

    #[test]
    fn test_failure_err() {

        let error : failure::Error = failure::err_msg("test_error_message");
        let notice : notice::Error = notice::From::from(error);
        assert_eq!("test_error_message", notice.class);
    }
}
