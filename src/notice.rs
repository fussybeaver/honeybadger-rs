use std::collections::HashMap;

use error_chain::ChainedError;

/// Serializable root notice event, for use with the notify endpoint of the Honeybadger API.
#[derive(Serialize)]
pub struct Notice<'req> {
    pub api_key: &'req str,
    pub notifier: Notifier,
    pub error: Error<'req>,
    pub request: Request<'req>,
    pub server: Server<'req>
}

/// Serializable leaf node representing the error to notify on.
#[derive(Serialize)]
pub struct Error<'req> {
    class: &'req str,
    message: Option<String>,
    causes: Option<Vec<Error<'req>>>
}

impl<'req> Error<'req> {
    /// Internal API to create a new Error instance for serialization purposes.
    pub fn new<E>(error: &E) -> Error 
        where E: ChainedError {
        Error {
            class: error.description(),
            message: Some(error.display_chain().to_string()),
            causes: Some(error.iter()
                         .map(|cause| Error::std_err(cause))
                         .collect())
        }
    }

    fn std_err(error: &::std::error::Error) -> Error {
        Error {
            class: error.description(),
            message: None,
            causes: error.cause().map(|cause| vec![Error::std_err(cause)])
        }
    }
}

/// Serializable leaf node representing the meta details on this crate
#[derive(Serialize)]
pub struct Notifier {
    pub name: &'static str,
    pub url: &'static str,
    pub version: &'static str
}

/// Leaf node containing the context hash and environment at the time of
/// serialization.
#[derive(Serialize)]
pub struct Request<'req> {
    pub context: Option<HashMap<&'req str, &'req str>>,
    pub cgi_data: HashMap<String, String>
}

/// Leaf node containing OS system information at the time of serialization
#[derive(Serialize)]
pub struct Server<'req> {
    pub project_root: &'req str,
    pub environment_name: &'req str,
    pub hostname: &'req str,
    pub time: u64,
    pub pid: u32
}
