use std::collections::HashMap;
use std::convert::From;
use std::env;
use std::fmt;
use std::iter::FromIterator;
use std::process;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use http::StatusCode;
use hyper::client::{HttpConnector};
use hyper::{Body, Client, Request};
use hyper_tls::HttpsConnector;

use crate::errors::*;
use crate::notice;
use notice::{Notice, Notifier};

const HONEYBADGER_ENDPOINT: &'static str = "/v1/notices";
const HONEYBADGER_DEFAULT_TIMEOUT: u64 = 5;
const HONEYBADGER_DEFAULT_THREADS: usize = 4;
const HONEYBADGER_SERVER_URL: &'static str = "https://api.honeybadger.io";

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
    threads: usize,
}

/// Configuration builder struct, used for building a `Config` instance
pub struct ConfigBuilder {
    api_key: String,
    root: Option<String>,
    env: Option<String>,
    hostname: Option<String>,
    endpoint: Option<String>,
    timeout: Option<Duration>,
    threads: Option<usize>,
}

/// Instance containing the client connection and user configuration for this crate.
pub struct Honeybadger {
    client: Arc<Client<HttpsConnector<HttpConnector>>>,
    config: Config,
    user_agent: String,
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
            timeout: env::var("HONEYBADGER_TIMEOUT")
                .ok()
                .and_then(|s| s.parse().ok())
                .map(|t| Duration::new(t, 0)),
            threads: None,
        }
    }

    /// Override the project root property for events posted to the Honeybadger API. Consumes the
    /// `ConfigBuilder` and returns a new value.
    ///
    /// # Arguments
    ///
    /// * `project_root` - The directory where your code lives.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use honeybadger::ConfigBuilder;
    /// let api_token = "ffffff";
    /// let config = ConfigBuilder::new(api_token).with_root("/tmp/my_project_root");
    /// ```
    pub fn with_root(mut self, project_root: &str) -> Self {
        self.root = Some(project_root.to_owned());
        self
    }

    /// Add an environment name property for events posted to the Honeybadger API, which will then
    /// be categorized accordingly in the UI. Consumes the `ConfigBuilder` and returns a new
    /// value.
    ///
    /// # Arguments
    ///
    /// * `environment` - The directory where your code lives.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use honeybadger::ConfigBuilder;
    /// let api_token = "ffffff";
    /// let config = ConfigBuilder::new(api_token).with_env("production");
    /// ```
    pub fn with_env(mut self, environment: &str) -> Self {
        self.env = Some(environment.to_owned());
        self
    }

    /// Override the hostname property for events posted to the Honeybadger API. Consumes the
    /// `ConfigBuilder` and returns a new value.
    ///
    /// # Arguments
    ///
    /// * `hostname` - The server's hostname
    ///
    /// # Example
    ///
    /// ```rust
    /// # use honeybadger::ConfigBuilder;
    /// let api_token = "ffffff";
    /// let config = ConfigBuilder::new(api_token).with_hostname("localhost");
    /// ```
    pub fn with_hostname(mut self, hostname: &str) -> Self {
        self.hostname = Some(hostname.to_owned());
        self
    }

    /// Override the Honeybadger endpoint used to post HTTP payloads. Consumes the `ConfigBuilder`
    /// and returns a new value.
    ///
    /// # Arguments
    ///
    /// * `endpoint` - A custom honeybadger endpoint to query
    ///
    /// # Example
    ///
    /// ```rust
    /// # use honeybadger::ConfigBuilder;
    /// let api_token = "ffffff";
    /// let config = ConfigBuilder::new(api_token).with_endpoint("http://proxy.example.com:5050/");
    /// ```
    pub fn with_endpoint(mut self, endpoint: &str) -> Self {
        self.endpoint = Some(endpoint.to_owned());
        self
    }

    /// Override the HTTP write timeout for the client used to post events to Honeybadger.
    /// Consumes the `ConfigBuilder` and returns a new value.
    ///
    /// # Arguments
    ///
    /// * `timeout` - A `Duration` reference specifying the HTTP timeout for the write request
    ///
    /// # Example
    ///
    /// ```rust
    /// # use honeybadger::ConfigBuilder;
    /// # use std::time::Duration;
    /// let api_token = "ffffff";
    /// let config = ConfigBuilder::new(api_token).with_timeout(&Duration::new(20, 0));
    /// ```
    pub fn with_timeout(mut self, timeout: &Duration) -> Self {
        self.timeout = Some(timeout.to_owned());
        self
    }

    /// Override the number of threads the async HTTP connection should use to queue Honeybadger
    /// payloads.  Consumes the `ConfigBuilder` and returns a new reference.
    ///
    /// # Arguments
    ///
    /// * `threads` - The number of threads to configure the hyper connector
    ///
    /// # Example
    ///
    /// ```rust
    /// # use honeybadger::ConfigBuilder;
    /// let api_token = "ffffff";
    /// let config = ConfigBuilder::new(api_token).with_threads(8);
    /// ```
    pub fn with_threads(mut self, threads: usize) -> Self {
        self.threads = Some(threads);
        self
    }

    /// Prepare a `Config` instance for constructing a Honeybadger instance.
    ///
    /// Defaults are set if the `ConfigBuilder` used to construct the `Config` is empty.
    ///
    ///   - _default root_: the current directory
    ///   - _default hostname_: the host name as reported by the operating system
    ///   - _default endpoint_: `https://api.honeybadger.io/v1/notices`
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
            root: self
                .root
                .or(env::current_dir()
                    .ok()
                    .and_then(|x| x.to_str().map(|x| x.to_owned())))
                .unwrap_or_else(|| "".to_owned()),
            env: self.env.unwrap_or_else(|| "".to_owned()),
            hostname: self
                .hostname
                .or(hostname::get().ok().map(|s| s.to_string_lossy().to_string()))
                .unwrap_or_else(|| "".to_owned()),
            endpoint: self
                .endpoint
                .unwrap_or_else(|| format!("{}{}", if cfg!(test) { mockito::server_url() } else { String::from(HONEYBADGER_SERVER_URL) }, HONEYBADGER_ENDPOINT ) ),
            timeout: self
                .timeout
                .unwrap_or_else(|| Duration::new(HONEYBADGER_DEFAULT_TIMEOUT, 0)),
            threads: self.threads.unwrap_or(HONEYBADGER_DEFAULT_THREADS),
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
        let https = HttpsConnector::new();

        let builder = Client::builder();

        let os = os_type::current_platform();
        let user_agent: String = fmt::format(format_args!(
            "HB-rust {}; {:?}/{}",
            VERSION, os.os_type, os.version
        ));

        debug!(
            "Constructed honeybadger instance with configuration: {:?}",
            config
        );

        Ok(Honeybadger {
            config: config,
            client: Arc::new(builder.build(https)),
            user_agent: user_agent,
        })
    }

    fn serialize<'req>(
        config: &Config,
        error: notice::Error,
        context: Option<HashMap<&'req str, &'req str>>,
    ) -> serde_json::Result<Vec<u8>> {
        let notifier = Notifier {
            name: NOTIFIER_NAME,
            url: NOTIFIER_URL,
            version: VERSION,
        };

        let request = notice::Request {
            context: context,
            cgi_data: HashMap::<String, String>::from_iter(env::vars()),
        };

        let server = notice::Server {
            project_root: &config.root,
            environment_name: &config.env,
            hostname: &config.hostname,
            time: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|v| v.as_secs())
                .unwrap_or(0),
            pid: process::id(),
        };

        let notice = Notice {
            api_key: &config.api_key,
            notifier: notifier,
            error: error,
            request: request,
            server: server,
        };

        serde_json::to_vec(&notice)
    }

    fn create_payload_with_config<'req>(
        config: &Config,
        user_agent: &str,
        error: notice::Error,
        context: Option<HashMap<&'req str, &'req str>>,
    ) -> Result<Request<Body>> {
        let api_key: &str = config.api_key.as_ref();
        let user_agent: &str = user_agent.as_ref();

        let data = Honeybadger::serialize(config, error, context)?;
        let r = Request::builder()
            .uri(config.endpoint.clone())
            .method(http::Method::POST)
            .header(http::header::ACCEPT, "application/json")
            .header("X-API-Key", api_key)
            .header(http::header::USER_AGENT, user_agent)
            .body(Body::from(data))?;

        Ok(r)
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
    /// * `error` - a struct that implements the [`From`][4] trait for a
    /// [`notice::Error`][5].
    /// * `context` - Optional [`HashMap`][7] to pass to the [Honeybadger context][6] API
    ///
    /// # Examples
    ///
    /// ## With `chained_error::Error`
    ///
    /// ```rust
    /// #[macro_use] extern crate error_chain;
    /// error_chain! {
    ///   errors {
    ///     MyCustomError
    ///   }
    /// }
    /// #
    /// # fn main() {
    /// # use honeybadger::{ConfigBuilder, Honeybadger};
    /// # use tokio::runtime::Runtime;
    /// # let api_token = "ffffff";
    /// # let config = ConfigBuilder::new(api_token).build();
    /// # let mut honeybadger = Honeybadger::new(config).unwrap();
    ///
    /// let error : Result<()> = Err(ErrorKind::MyCustomError.into());
    ///
    /// let mut rt = Runtime::new().unwrap();
    /// let future = honeybadger.notify(
    ///   honeybadger::notice::Error::new(&error.unwrap_err()),
    ///   None);
    ///
    /// rt.block_on(future);
    /// #
    /// # }
    /// ```
    ///
    /// ## With `failure::Error`
    ///
    /// ```rust, no_run
    /// #[macro_use] extern crate failure;
    /// #[derive(Fail, Debug)]
    /// #[fail(display = "Failure error")]
    /// struct MyCustomError;
    /// # fn main() {
    /// # use honeybadger::{ConfigBuilder, Honeybadger};
    /// # use tokio::runtime::Runtime;
    /// # let api_token = "ffffff";
    /// # let config = ConfigBuilder::new(api_token).build();
    /// # let mut honeybadger = Honeybadger::new(config).unwrap();
    ///
    /// let error: Result<(), failure::Error> = Err(MyCustomError {}.into());
    ///
    /// let mut rt = Runtime::new().unwrap();
    /// let future = honeybadger.notify(
    ///   error.unwrap_err(),
    ///   None);
    ///
    /// rt.block_on(future).unwrap();
    /// #
    /// # }
    /// ```
    ///
    /// ## With `Box<std::error::Error>`.
    ///
    /// Note that [`std::error::Error`](8) does not implement [Sync](9), and it's not possible to
    /// use the error type across future combinators, so it's recommended to convert into a
    /// `Box<std::error::Error>` in the same closure as the Honeybadger API call.
    ///
    /// ```rust, no_run
    /// # fn main() {
    /// # use honeybadger::{ConfigBuilder, Honeybadger};
    /// # use tokio::runtime::Runtime;
    /// # let api_token = "ffffff";
    /// # let config = ConfigBuilder::new(api_token).build();
    /// # let mut honeybadger = Honeybadger::new(config).unwrap();
    ///
    /// let error: Result<(), Box<std::error::Error>> = Err(
    ///   std::io::Error::new(
    ///     std::io::ErrorKind::Other, "std Error"
    ///   ).into()
    /// );
    ///
    /// let mut rt = Runtime::new().unwrap();
    /// let future = honeybadger.notify(
    ///   error.unwrap_err(),
    ///   None);
    ///
    /// rt.block_on(future).unwrap();
    /// #
    /// # }
    /// ```
    ///
    /// [1]: https://github.com/tokio-rs/tokio
    /// [2]: https://docs.rs/futures/0.2.1/futures/future/index.html
    /// [3]: https://docs.rs/hyper/0.12.5/hyper/struct.Request.html
    /// [4]: https://doc.rust-lang.org/std/convert/trait.From.html
    /// [5]: notice/struct.Error.html
    /// [6]: https://docs.honeybadger.io/ruby/getting-started/adding-context-to-errors.html#context-in-honeybadger-notify
    /// [7]: https://doc.rust-lang.org/std/collections/struct.HashMap.html
    /// [8]: https://doc.rust-lang.org/std/error/trait.Error.html
    /// [9]: https://doc.rust-lang.org/std/marker/trait.Sync.html
    pub async fn notify<'req, E: Into<notice::Error>>(
        self,
        error: E,
        context: Option<HashMap<&'req str, &'req str>>,
    ) -> Result<()>
    where
        notice::Error: From<E>,
    {
        let t = self.config.timeout.as_secs();
        let request = Honeybadger::create_payload_with_config(
            &self.config,
            &self.user_agent,
            error.into(),
            context,
        )?;
        Ok(Honeybadger::notify_with_client(&self.client, t, request).await?)
    }

    async fn notify_with_client<'req, C>(
        client: &Client<C>,
        timeout: u64,
        request: Request<Body>,
    ) -> Result<()>
    where
        C: hyper::client::connect::Connect + Sync + 'static + Clone + Send,
    {
        let req = client
            .request(request);

        let response = match tokio::time::timeout(Duration::from_secs(timeout), req).await {
            Ok(v) => v.map_err(|err| Honeybadger::convert_error(ErrorKind::Hyper(err))),
            Err(_) => Err(Honeybadger::convert_error(ErrorKind::TimeoutError(timeout))),
        }?;

        let (parts, _) = response.into_parts();
        debug!("Honeybadger API returned status: {}", parts.status);
        match parts.status {
            s if s.is_success() => Ok(()),
            s if s.is_redirection() => Err(ErrorKind::RedirectionError.into()),
            StatusCode::UNAUTHORIZED => Err(ErrorKind::UnauthorizedError.into()),
            StatusCode::UNPROCESSABLE_ENTITY => Err(ErrorKind::NotProcessedError.into()),
            StatusCode::TOO_MANY_REQUESTS => Err(ErrorKind::RateExceededError.into()),
            StatusCode::INTERNAL_SERVER_ERROR => Err(ErrorKind::ServerError.into()),
            _ => Err(ErrorKind::UnknownStatusCodeError(parts.status.as_u16()).into()),
        }
    }
}

#[cfg(test)]
mod tests {

    use crate::honeybadger::*;
    use hyper::client::Client;
    use hyper::Body;
    use std::time::Duration;
    use tokio::runtime::Runtime;
    use mockito::mock;

    fn test_client_with_response(status: usize, config: &Config) -> Result<()> {
        let _m = mock("POST", HONEYBADGER_ENDPOINT)
            .with_status(status)
            .with_header("Content-Type", "application/json")
            .create();

        let mut http_connector = HttpConnector::new();
        http_connector.enforce_http(false);
        let client = Client::builder().build::<HttpConnector, Body>(http_connector);

        let mut rt = Runtime::new().unwrap();

        let error: Result<()> = Err(ErrorKind::RedirectionError.into());
        let error = notice::Error::new(&error.unwrap_err());
        let req =
            Honeybadger::create_payload_with_config(config, "test-client", error, None).unwrap();
        let t = config.timeout.as_secs();
        let res = Honeybadger::notify_with_client(&client, t, req);

        rt.block_on(res)
    }

    #[test]
    fn test_notify_ok() {
        let config = ConfigBuilder::new("dummy-api-key").build();
        let res = test_client_with_response(201,
            &config,
        );

        assert_eq!((), res.unwrap());
    }

    #[test]
    fn test_notify_rate_exceeded() {
        let config = ConfigBuilder::new("dummy-api-key").build();
        let res = test_client_with_response(
            429,
            &config,
        );

        match res {
            Err(Error(ErrorKind::RateExceededError, _)) => assert!(true),
            _ => assert_eq!("", "expected rate exceeded error, but was not"),
        }
    }

    #[test]
    fn test_with_root() {
        let config = ConfigBuilder::new("dummy-api-key").build();

        assert_ne!("/tmp/build", config.root);

        let config = ConfigBuilder::new("dummy-api-key")
            .with_root("/tmp/build")
            .build();

        assert_eq!("/tmp/build", config.root);
    }

    #[test]
    fn test_with_env() {
        let config = ConfigBuilder::new("dummy-api-key").build();

        assert_eq!("", config.env);

        let config = ConfigBuilder::new("dummy-api-key").with_env("test").build();

        assert_eq!("test", config.env);
    }

    #[test]
    fn test_with_hostname() {
        let config = ConfigBuilder::new("dummy-api-key").build();

        assert_ne!("hickyblue", config.hostname);

        let config = ConfigBuilder::new("dummy-api-key")
            .with_hostname("hickyblue")
            .build();

        assert_eq!("hickyblue", config.hostname);
    }

    #[test]
    fn test_with_endpoint() {
        let config = ConfigBuilder::new("dummy-api-key").build();

        assert_eq!(format!("{}{}", mockito::server_url(), HONEYBADGER_ENDPOINT), config.endpoint);

        let config = ConfigBuilder::new("dummy-api-key")
            .with_endpoint("http://example.com/")
            .build();

        assert_eq!("http://example.com/", config.endpoint);
    }

    #[test]
    fn test_with_timeout() {
        let config = ConfigBuilder::new("dummy-api-key").build();

        assert_eq!(
            Duration::new(HONEYBADGER_DEFAULT_TIMEOUT, 0),
            config.timeout
        );

        let config = ConfigBuilder::new("dummy-api-key")
            .with_timeout(&Duration::new(20, 0))
            .build();

        assert_eq!(Duration::new(20, 0), config.timeout);
    }

    #[test]
    fn test_with_threads() {
        let config = ConfigBuilder::new("dummy-api-key").build();

        assert_eq!(HONEYBADGER_DEFAULT_THREADS, config.threads);

        let config = ConfigBuilder::new("dummy-api-key")
            .with_threads(128)
            .build();

        assert_eq!(128, config.threads);
    }
}
