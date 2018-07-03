use std::collections::HashMap;

use error_chain::ChainedError;

#[derive(Serialize)]
pub struct Notice<'req> {
    pub api_key: &'req str,
    pub notifier: Notifier,
    pub error: Error<'req>,
    pub request: Request<'req>,
    pub server: Server<'req>
}

#[derive(Serialize)]
pub struct Error<'req> {
    class: &'req str,
    message: Option<String>,
    causes: Option<Vec<Error<'req>>>
}

impl<'req> Error<'req> {
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

#[derive(Serialize)]
pub struct Notifier {
    pub name: &'static str,
    pub url: &'static str,
    pub version: &'static str
}

#[derive(Serialize)]
pub struct Request<'req> {
    pub context: Option<HashMap<&'req str, &'req str>>,
    pub cgi_data: HashMap<String, String>
}

#[derive(Serialize)]
pub struct Server<'req> {
    pub project_root: &'req str,
    pub environment_name: &'req str,
    pub hostname: &'req str,
    pub time: u64,
    pub pid: u32
}
