use std::collections::HashMap;

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
    backtrace: Vec<Frame>,
    causes: Option<Vec<Error<'req>>>
}

impl<'req> Error<'req> {
    pub fn new<E>(error: &E) -> Error 
        where E: ::error_chain::ChainedError {
        Error {
            class: error.description(),
            message: Some(error.display_chain().to_string()),
            backtrace: Frame::generate_stack(error),
            causes: Some(error.iter()
                         .map(|cause| Error::std_err(cause))
                         .collect())
        }
    }

    fn std_err(error: &::std::error::Error) -> Error {
        Error {
            class: error.description(),
            message: None,
            backtrace: vec![],
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

/// Frame represents a stack frame inside of a Honeybadger backtrace
#[derive(Serialize)]
pub struct Frame {
    number: String,
    file: String,
    method: String,
}

impl Frame {
    pub fn generate_stack<E>(error: &E) -> Vec<Frame> 
        where E: ::error_chain::ChainedError {
        let backtrace = match error.backtrace() {
            Some(backtrace) => backtrace,
            None => return vec![]
        };

        let mut frames: Vec<Frame> = Vec::new();
        let mut frames_iter = backtrace.frames().iter();
        while let Some(backtrace_frame) = frames_iter.next() {

            // retrieve the outermost symbol in a frame, if it exists
            let s = backtrace_frame.symbols();
            let symbol = match s {
                [] => { continue; },
                _ => &s[s.len()-1]
            };

            frames.push(
                Frame{
                    number: symbol.lineno().map(|num| num.to_string())
                        .unwrap_or_else(String::new),
                    file: symbol.filename()
                        .and_then(|path| path.to_str().map(|s| s.to_owned()))
                        .unwrap_or_else(String::new),
                    method: symbol.name()
                        .and_then(|symbol_name| symbol_name.as_str().map(|s| s.to_owned()))
                        .unwrap_or_else(String::new)
                });
        }

        frames
    }
}
