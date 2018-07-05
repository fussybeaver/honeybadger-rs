use std::collections::HashMap;
use std::env;
use std::fmt;
use std::iter::FromIterator;
use std::process;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use hyper::client::{Client, HttpConnector};
use hyper::{Body, Request};
use hyper::rt::Future;
use hyper_tls::HttpsConnector;
use http::StatusCode;
use http;
use os_type;
use tokio::util::FutureExt;

use errors::*;
use error_chain::ChainedError;

use hostname;
use notice;
use notice::{Notice, Notifier};

use serde_json;

const HONEYBADGER_ENDPOINT: &'static str = "https://api.honeybadger.io/v1/notices";
const HONEYBADGER_DEFAULT_TIMEOUT: u64 = 5;
const HONEYBADGER_DEFAULT_THREADS: usize = 4;

const NOTIFIER_NAME: &'static str = "honeybadger";
const NOTIFIER_URL: &'static str = "https://github.com/fussybeaver/honeybader-rs";

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

#[derive(Debug)]
pub struct Config {
    api_key: String,
    root: String,
    env: String,
    hostname: String,
    endpoint: String,
    timeout: Duration,
    threads: usize
}

pub struct ConfigBuilder {
    api_key: String,
    root: Option<String>,
    env: Option<String>,
    hostname: Option<String>,
    endpoint: Option<String>,
    timeout: Option<Duration>,
    threads: Option<usize>
}

pub struct Honeybadger {
    client: Client<HttpsConnector<HttpConnector>>,
    config: Config,
    user_agent: String
}

impl ConfigBuilder {
    pub fn new(api_token: &str) -> Self {
        Self {
            api_key: api_token.to_owned(),
            root: env::var("HONEYBADGER_ROOT").ok(),
            env: env::var("ENV").ok(),
            hostname: env::var("HOSTNAME").ok(),
            endpoint: env::var("HONEYBADGER_ENDPOINT").ok(),
            timeout: env::var("HONEYBADGER_TIMEOUT").ok().and_then(|s| s.parse().ok()).map(|t| Duration::new(t, 0)),
            threads: None
        }
    }

    // TODO pub fn with.. builders

    pub fn build(self) -> Config {
        Config {
            api_key: self.api_key,
            root: self.root
                .or(env::current_dir().ok().and_then(|x| x.to_str().map(|x| x.to_owned())))
                .unwrap_or_else(|| "".to_owned()),
            env: self.env.unwrap_or_else(|| "".to_owned()),
            hostname: self.hostname
                .or(hostname::get_hostname())
                .unwrap_or_else(|| "".to_owned()),
            endpoint: self.endpoint
                .unwrap_or_else(|| HONEYBADGER_ENDPOINT.to_owned()),
            timeout: self.timeout
                .unwrap_or_else(|| Duration::new(HONEYBADGER_DEFAULT_TIMEOUT, 0)),
            threads: self.threads
                .unwrap_or(HONEYBADGER_DEFAULT_THREADS)
        }
    }
}

impl Honeybadger {

    pub fn new(config: Config) -> Result<Self> {

        let https = HttpsConnector::new(config.threads)?;

        let builder = Client::builder();

        let os = os_type::current_platform();
        let user_agent: String = fmt::format(
            format_args!("HB-rust {:?}; {:?}/{:?}", 
                         VERSION, os.os_type, os.version));

        debug!("Constructed honeybadger instance with configuration: {:?}", config);

        Ok(Honeybadger {
            config: config,
            client: builder.build(https),
            user_agent: user_agent
        })
    }

    fn serialize<'req, E>(config: &Config,
                          error: &E, 
                          context: Option<HashMap<&'req str, &'req str>>) 
        -> serde_json::Result<Vec<u8>> 
        where E: ChainedError {
            
        let notifier = Notifier {
            name: NOTIFIER_NAME,
            url: NOTIFIER_URL,
            version: VERSION
        };

        let error = notice::Error::new(error);
        let request = notice::Request {
            context: context,
            cgi_data: HashMap::<String, String>::from_iter(env::vars())
        };

        let server = notice::Server {
            project_root: &config.root,
            environment_name: &config.env,
            hostname: &config.hostname,
            time: SystemTime::now().duration_since(UNIX_EPOCH)
                .map(|v| v.as_secs()).unwrap_or(0),
            pid: process::id()
        };

        let notice = Notice {
            api_key: &config.api_key,
            notifier: notifier,
            error: error,
            request: request,
            server: server
        };

        serde_json::to_vec(&notice)
    }

    fn create_payload_with_config<'req, E>(config: &Config,
                                           user_agent: &str,
                                           error: &E,
                                           context: Option<HashMap<&'req str, &'req str>>) 
        -> Result<Request<Body>>
        where E: ChainedError {

            let mut request = Request::builder();

            let api_key: &str = config.api_key.as_ref();
            let user_agent: &str = user_agent.as_ref();

            request.uri(config.endpoint.clone())
                .method(http::Method::POST)
                .header(http::header::ACCEPT, "application/json")
                .header("X-API-Key", api_key)
                .header(http::header::USER_AGENT, user_agent);

            let data = Honeybadger::serialize(config, error, context)?;

            debug!("Serialized Honeybadger notify payload: {}", error);

            let r = request.body(Body::from(data))?;
            Ok(r)
        }
    
    pub fn create_payload<'req, E>(&mut self, 
                             error: &E,
                             context: Option<HashMap<&'req str, &'req str>>) 
        -> Result<Request<Body>>
        where E: ChainedError {
            Honeybadger::create_payload_with_config(&self.config, &self.user_agent, error, context)
        }

    fn convert_error(kind: ErrorKind) -> Error {
        let e: Result<()> = Err(kind.into());
        e.err().unwrap()
    }

    pub fn notify<'req>(&mut self, 
                        request: Request<Body>) -> impl Future<Item=(), Error=Error> {

        Honeybadger::notify_with_client(&self.client, &self.config, &self.user_agent, request)
    }

    fn notify_with_client<'req, C>(client: &Client<C>,
                                   config: &Config,
                                   user_agent: &str,
                                   request: Request<Body>) -> impl Future<Item=(), Error=Error>
        where C: ::hyper::client::connect::Connect + Sync + 'static,
              C::Error: 'static,
              C::Transport: 'static {

        let now = ::std::time::Instant::now();
        let t = config.timeout.as_secs();

        debug!("Sending honebadger payload with user agent: {}", user_agent);

        client.request(request)
            .map_err(move |e| {
                error!("Honeybadger client error: {}", e);
                Honeybadger::convert_error(ErrorKind::Hyper(e))
            })
            .deadline(now + config.timeout)
            .map_err(move |e| {
                error!("Honeybadger request timed-out!: {}", e);
                Honeybadger::convert_error(ErrorKind::TimeoutError(t))
            })
            .and_then(|response| {
                let (parts, _) = response.into_parts();
                debug!("Honeybadger API returned status: {}", parts.status);
                match parts.status {
                    s if s.is_success() => Ok(()),
                    s if s.is_redirection() => Err(ErrorKind::RedirectionError.into()),
                    StatusCode::UNAUTHORIZED => Err(ErrorKind::UnauthorizedError.into()),
                    StatusCode::UNPROCESSABLE_ENTITY => Err(ErrorKind::NotProcessedError.into()),
                    StatusCode::TOO_MANY_REQUESTS => Err(ErrorKind::RateExceededError.into()),
                    StatusCode::INTERNAL_SERVER_ERROR => Err(ErrorKind::ServerError.into()),
                    _ => {
                        Err(ErrorKind::UnknownStatusCodeError(parts.status.as_u16()).into())
                    }
                }
            })
    }
}

#[cfg(test)]
mod tests {

    use honeybadger::{ConfigBuilder, Config, Honeybadger};
    use hyper::Body;
    use hyper::client::Client;
    use hyper_mock::SequentialConnector;
    use tokio::runtime::current_thread;
    use errors::*;

    fn test_client_with_response(res: String, config: &Config) -> Result<()> {
        let mut c = SequentialConnector::default();
        c.content.push(res);

        let client = Client::builder()
            .build::<SequentialConnector, Body>(c);

        let mut rt = current_thread::Runtime::new().unwrap();

        let error : Result<()> = Err(ErrorKind::RedirectionError.into());
        let req = Honeybadger::create_payload_with_config(config, "test-client", &error.unwrap_err(), None).unwrap();
        let res = Honeybadger::notify_with_client(&client, config, "test-client", req);

        rt.block_on(res)
    }

    #[test]
    fn test_notify_ok() {
        let config = ConfigBuilder::new("dummy-api-key").build();
        let res = test_client_with_response("HTTP/1.1 201 Created\r\n\
                                             Server: mock1\r\n\
                                             \r\n\
                                             ".to_string(), &config);

        assert_eq!((), res.unwrap());
    }

    #[test]
    fn test_notify_timeout() {
        let config = ConfigBuilder::new("dummy-api-key").build();
        let res = test_client_with_response("HTTP/1.1 201 Created\r\n".to_string(), &config);

        match res {
            Err(Error(ErrorKind::TimeoutError(5), _)) => assert!(true),
            _ => 
                assert_eq!("", "expected timeout error, but was not")
        }
    }

    #[test]
    fn test_notify_rate_exceeded() {
        let config = ConfigBuilder::new("dummy-api-key").build();
        let res = test_client_with_response("HTTP/1.1 429 Too Many Requests\r\n\
                                             Server: mock1\r\n\
                                             \r\n\
                                             ".to_string(), &config);

        match res {
            Err(Error(ErrorKind::RateExceededError, _)) => assert!(true),
            _ => assert_eq!("", "expected timeout error, but was not")
        }
    }
}

