use http;
use hyper;
use hyper_tls;
use std::io;
use serde_json;

error_chain! {
    foreign_links {
        Hyper(hyper::Error);
        HyperTls(hyper_tls::Error);
        Http(http::Error);
        Io(io::Error);
        SerdeJson(serde_json::Error);
    }

    errors {
        UnauthorizedError {
            description("API key is incorrect or the account is deactivated")
        }
        RateExceededError {
            description("Honeybadger rate limit exceeded")
        }
        NotProcessedError {
            description("The payload couldn't be processed")
        }
        RedirectionError {
            description("The endpoint replied with a redirect")
        }
        ServerError {
            description("The honeybadger API replied with a '500 Internal Server Error'")
        }
        TimeoutError(timeout: u64) {
            description("Honeybadger client timed out")
            display("Honeybadger timed out after {} seconds", timeout)
        }
        UnknownStatusCodeError(status_code: u16) {
            description("Honeybadger responded with an unknown status code")
            display("Honeybadger responded with an unknown status code: {}", status_code)
        }
    }
}
