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

/// Config instance containing user-defined configuration for this crate.
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

/// Configuration builder struct, used for building a `Config` instance
pub struct ConfigBuilder {
    api_key: String,
    root: Option<String>,
    env: Option<String>,
    hostname: Option<String>,
    endpoint: Option<String>,
    timeout: Option<Duration>,
    threads: Option<usize>
}

/// Instance containing the client connection and user configuration for this crate.
pub struct Honeybadger {
    client: Client<HttpsConnector<HttpConnector>>,
    config: Config,
    user_agent: String
}

impl ConfigBuilder {

    /// Construct a `ConfigBuilder` to parametrize the Honeybadger client.
    ///
    /// `ConfigBuilder` is populated using environment variables, which will inject
    /// Honeybadger event fields:
    ///   - `HONEYBADGER_ROOT` - project root for each event.
    ///   - `ENV` - environment name for each event.
    ///   - `HOSTNAME` - host name for each event.
    ///   - `HONEYBADGER_ENDPOINT` - override the default endpoint for the HTTPS client.
    ///   - `HONEYBADGER_TIMEOUT` - write timeout for the Honeybadger HTTPS client.
    ///
    /// # Arguments
    ///
    /// * `api_token` - API key for the honeybadger project
    ///
    /// # Example
    ///
    /// ```rust
    /// # use honeybadger::ConfigBuilder;
    /// let api_token = "ffffff";
    /// let config = ConfigBuilder::new(api_token);
    /// ```
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

    pub fn with_root(mut self, project_root: &str) -> Self {
        self.root = Some(project_root.to_owned());
        self
    }

    pub fn with_env(mut self, environment: &str) -> Self {
        self.env = Some(environment.to_owned());
        self
    }

    pub fn with_hostname(mut self, hostname: &str) -> Self {
        self.hostname = Some(hostname.to_owned());
        self
    }

    pub fn with_endpoint(mut self, endpoint: &str) -> Self {
        self.endpoint = Some(endpoint.to_owned());
        self
    }

    pub fn with_timeout(mut self, timeout: &Duration) -> Self {
        self.timeout = Some(timeout.to_owned());
        self
    }

    pub fn with_threads(mut self, threads: usize) -> Self {
        self.threads = Some(threads);
        self
    }

    // TODO pub fn with.. builders

    /// Prepare a `Config` instance for constructing a Honeybadger instance.
    ///
    /// Defaults are set if the `ConfigBuilder` used to construct the `Config` is empty.
    ///
    ///   - _default root_: the current directory
    ///   - _default hostname_: the host name as reported by the operating system
    ///   - _default endpoint_: "https://api.honeybadger.io/v1/notices"
    ///   - _default timeout_: a 5 second client write timeout
    ///   - _default threads_: 4 threads are used in the asynchronous runtime pool
    ///
    /// # Example
    ///
    /// ```rust
    /// # use honeybadger::ConfigBuilder;
    /// # let api_token = "ffffff";
    /// ConfigBuilder::new(api_token).build();
    /// ```
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

    /// Constructs a Honeybadger instance, which may be used to send API notify requests.
    ///
    /// # Arguments
    ///
    /// * `config` - `Config` instance, which is built using the `ConfigBuilder`
    ///
    /// # Example
    ///
    /// ```
    /// # use honeybadger::{ConfigBuilder, Honeybadger};
    /// # let api_token = "ffffff";
    /// let config = ConfigBuilder::new(api_token).build();
    ///
    /// assert_eq!(true, Honeybadger::new(config).is_ok());
    /// ```
    pub fn new(config: Config) -> Result<Self> {

        let https = HttpsConnector::new(config.threads)?;

        let builder = Client::builder();

        let os = os_type::current_platform();
        let user_agent: String = fmt::format(
            format_args!("HB-rust {}; {:?}/{}",
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

    /// Prepare a payload for the notify request.
    ///
    /// Requires the use of the [error_chain][1] crate.
    ///
    /// # Arguments
    ///
    /// * `error`   - `ChainedError` compatible with an [error_chain][1] crate
    /// * `context` - Optional `HashMap` to pass to the [Honeybadger context][2] API
    ///
    /// # Example
    ///
    /// ```rust
    /// # #[macro_use] extern crate error_chain;
    /// # extern crate honeybadger;
    /// error_chain! {
    ///   errors {
    ///     MyCustomError
    ///   }
    /// }
    /// #
    /// # fn main() {
    /// # use honeybadger::{ConfigBuilder, Honeybadger};
    /// # let api_token = "ffffff";
    /// # let config = ConfigBuilder::new(api_token).build();
    /// # let mut honeybadger = Honeybadger::new(config).unwrap();
    ///
    /// let error : Result<()> = Err(ErrorKind::MyCustomError.into());
    /// honeybadger.create_payload(&error.unwrap_err(), None);
    /// # }
    /// ```
    ///
    /// [1]: https://rust-lang-nursery.github.io/error-chain/error_chain/index.html
    /// [2]: https://docs.honeybadger.io/ruby/getting-started/adding-context-to-errors.html#context-in-honeybadger-notify
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

    /// Trigger the notify request using an async HTTPS request.
    ///
    /// Requires an initialized [Tokio][1] `Runtime`, and returns a [Future][2] that must be
    /// resolved using the Tokio framework orchestration methods.
    ///
    /// # Arguments
    ///
    /// * `request` - [Request][3] instance constructed with `create_payload`
    ///
    /// # Example
    ///
    /// ```rust
    /// # #[macro_use] extern crate error_chain;
    /// # extern crate honeybadger;
    /// # extern crate tokio;
    /// # error_chain! {
    /// #   errors {
    /// #     MyCustomError
    /// #   }
    /// # }
    /// #
    /// # fn main() {
    /// # use honeybadger::{ConfigBuilder, Honeybadger};
    /// # use tokio::runtime::current_thread;
    /// # let api_token = "ffffff";
    /// # let config = ConfigBuilder::new(api_token).build();
    /// # let mut honeybadger = Honeybadger::new(config).unwrap();
    /// #
    /// # let error : Result<()> = Err(ErrorKind::MyCustomError.into());
    /// #
    /// let mut rt = current_thread::Runtime::new().unwrap();
    /// let payload = honeybadger.create_payload(&error.unwrap_err(), None).unwrap();
    /// let future = honeybadger.notify(payload);
    ///
    /// // note: blocks the current thread!
    /// rt.block_on(future);
    /// #
    /// # }
    /// ```
    /// [1]: https://github.com/tokio-rs/tokio
    /// [2]: https://docs.rs/futures/0.2.1/futures/future/index.html
    /// [3]: https://docs.rs/hyper/0.12.5/hyper/struct.Request.html
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

        debug!("Sending honeybadger payload with user agent: {}", user_agent);

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

    use honeybadger::*;
    use hyper::Body;
    use hyper::client::Client;
    use hyper_mock::SequentialConnector;
    use std::time::Duration;
    use tokio::runtime::current_thread;

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
            _ => assert_eq!("", "expected rate exceeded error, but was not")
        }
    }

    #[test]
    fn test_with_root() {
        let config = ConfigBuilder::new("dummy-api-key").build();

        assert_ne!("/tmp/build", config.root);

        let config = ConfigBuilder::new("dummy-api-key")
            .with_root("/tmp/build").build();

        assert_eq!("/tmp/build", config.root);
    }

    #[test]
    fn test_with_env() {
        let config = ConfigBuilder::new("dummy-api-key").build();

        assert_eq!("", config.env);

        let config = ConfigBuilder::new("dummy-api-key")
            .with_env("test").build();

        assert_eq!("test", config.env);
    }

    #[test]
    fn test_with_hostname() {
        let config = ConfigBuilder::new("dummy-api-key").build();

        assert_ne!("hickyblue", config.hostname);

        let config = ConfigBuilder::new("dummy-api-key")
            .with_hostname("hickyblue").build();

        assert_eq!("hickyblue", config.hostname);
    }

    #[test]
    fn test_with_endpoint() {
        let config = ConfigBuilder::new("dummy-api-key").build();

        assert_eq!(HONEYBADGER_ENDPOINT, config.endpoint);

        let config = ConfigBuilder::new("dummy-api-key")
            .with_endpoint("http://example.com/").build();

        assert_eq!("http://example.com/", config.endpoint);
    }

    #[test]
    fn test_with_timeout() {
        let config = ConfigBuilder::new("dummy-api-key").build();

        assert_eq!(Duration::new(HONEYBADGER_DEFAULT_TIMEOUT, 0), config.timeout);

        let config = ConfigBuilder::new("dummy-api-key")
            .with_timeout(&Duration::new(20, 0)).build();

        assert_eq!(Duration::new(20, 0), config.timeout);
    }

    #[test]
    fn test_with_threads() {
        let config = ConfigBuilder::new("dummy-api-key").build();

        assert_eq!(HONEYBADGER_DEFAULT_THREADS, config.threads);

        let config = ConfigBuilder::new("dummy-api-key")
            .with_threads(128).build();

        assert_eq!(128, config.threads);
    }
}
