use std::io;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;

use thiserror::Error;
use url::{ParseError, Url};

#[derive(Debug, Error)]
pub enum HTTPServerError {
    #[error("Could not parse query: {source}")]
    HTTPStreamParsingError { source: io::Error },
    #[error("Could not write response ({response}) to stream: {source}")]
    HTTPResponseWriteFailed { response: String, source: io::Error },
    #[error("Could not get stream: {source}")]
    HTTPStreamRecoverError { source: io::Error },
    #[error(transparent)]
    RequestParsingError {
        #[from]
        source: RequestParsingError,
    },
}

#[derive(Debug, Error)]
pub enum HttpInitError {
    #[error("Could not initialize server: {source}")]
    HTTPInitError {
        #[from]
        source: io::Error,
    },
}

#[derive(Debug, Error)]
#[error("request an available port from OS: {source}")]
pub struct HttpPortRequestError {
    #[from]
    source: std::io::Error,
}

pub struct Server {
    listener: TcpListener,
    handler: Arc<dyn Fn(Request) -> (Response, bool) + Send + 'static + Sync>,
}

impl Server {
    pub fn new(
        port: u32,
        handler: Arc<dyn Fn(Request) -> (Response, bool) + Send + 'static + Sync>,
    ) -> Result<Server, HttpInitError> {
        let listener = TcpListener::bind(format!("localhost:{}", port))?;
        Ok(Server { listener, handler })
    }

    pub fn listen(&self) -> Result<(), HTTPServerError> {
        for stream in self.listener.incoming() {
            let shutdown = self.handle(
                stream.map_err(|source| HTTPServerError::HTTPStreamRecoverError { source })?,
            )?;
            if shutdown {
                break;
            }
        }

        Ok(())
    }

    #[allow(dead_code)] // Used by the tests, which are based on randomly assigned port by the OS
    pub fn port(&self) -> Result<u16, HttpPortRequestError> {
        let addr = self.listener.local_addr()?;
        Ok(addr.port())
    }

    fn handle(&self, mut stream: TcpStream) -> Result<bool, HTTPServerError> {
        let reader = BufReader::new(&mut stream);

        let mut lines = vec![];
        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(source) => {
                    return Err(HTTPServerError::HTTPStreamParsingError { source });
                }
            };
            if line.is_empty() {
                break;
            }
            lines.push(line)
        }

        let request = Request::from(&lines)?;
        let (resp, shutdown) = self.handler.call((request,));

        let response = resp.to_http_protocol();
        stream
            .write_all(response.as_bytes())
            .map_err(|source| HTTPServerError::HTTPResponseWriteFailed { response, source })?;

        Ok(shutdown)
    }
}

#[derive(Debug, Error)]
#[error("Method not supported: {method}")]
pub struct HTTPRequestMethodNotSupported {
    method: String,
}

#[derive(Debug, PartialEq)]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
}

impl Method {
    fn from(protocol: &str) -> Result<Self, HTTPRequestMethodNotSupported> {
        match protocol {
            "GET" => Ok(Method::Get),
            "POST" => Ok(Method::Post),
            "PUT" => Ok(Method::Put),
            "DELETE" => Ok(Method::Delete),
            proto => Err(HTTPRequestMethodNotSupported {
                method: proto.to_string(),
            }),
        }
    }
}

#[derive(Debug, Error)]
pub enum RequestParsingError {
    #[error("No lines were passed when trying to initialize request")]
    HTTPRequestNoHTTPLines,
    #[error("Protocol not found: {line}")]
    HTTPRequestProtocolNotFound { line: String },
    #[error("Path not found: {line}")]
    HTTPRequestPathNotFound { line: String },
    #[error("Host not found")]
    HTTPRequestHostNotFound { line: Option<String> },
    #[error("Could not parse URL: {source}")]
    URLParsingError {
        #[from]
        source: ParseError,
    },
    #[error(transparent)]
    HTTPRequestMethodNotSupported {
        #[from]
        source: HTTPRequestMethodNotSupported,
    },
}

#[derive(Debug, PartialEq)]
pub struct Request {
    pub method: Method,
    pub url: Url,
}

impl Request {
    fn from(lines: &Vec<String>) -> Result<Self, RequestParsingError> {
        if lines.is_empty() {
            return Err(RequestParsingError::HTTPRequestNoHTTPLines);
        }

        let protocol_line = lines[0].clone();
        let mut protocol = protocol_line.split(' ');
        let method = match protocol.next() {
            None => {
                return Err(RequestParsingError::HTTPRequestProtocolNotFound {
                    line: protocol_line,
                });
            }
            Some(proto) => Method::from(proto)?,
        };
        let path = match protocol.next() {
            None => {
                return Err(RequestParsingError::HTTPRequestPathNotFound {
                    line: protocol_line,
                });
            }
            Some(path) => path,
        };

        let mut host: Option<String> = None;
        for line in lines {
            if line.starts_with("Host:") || line.starts_with("host:") {
                let mut host_split = line.split(' ');
                host_split.next();
                if let Some(found_host) = host_split.next() {
                    host = Some(found_host.to_string())
                } else {
                    return Err(RequestParsingError::HTTPRequestHostNotFound {
                        line: Some(line.clone()),
                    });
                }
                break;
            }
        }

        if host.is_none() {
            return Err(RequestParsingError::HTTPRequestHostNotFound { line: None });
        }

        Ok(Request {
            method,
            url: Url::parse(format!("http://{}{}", host.unwrap(), path).as_str())?,
        })
    }
}

pub struct Response {
    pub status: HttpStatus,
    pub body: Option<String>,
}

impl Response {
    pub(crate) fn to_http_protocol(&self) -> String {
        let status_line = format!("HTTP/1.1 {} {}", self.status.as_u16(), self.status.as_str());

        match &self.body {
            None => {
                let response_str = format!("{status_line}\r\n\r\n");
                response_str
            }
            Some(body) => {
                let content_len = body.len();
                let response_str = format!(
                    "{status_line}\r\nContent-Length: {content_len}\r\n\r\n{}",
                    body
                );
                response_str
            }
        }
    }
}

pub enum HttpStatus {
    OK,
    InternalServerError,
}

impl HttpStatus {
    fn as_u16(&self) -> u16 {
        match self {
            HttpStatus::OK => 200,
            HttpStatus::InternalServerError => 500,
        }
    }

    fn as_str(&self) -> String {
        match self {
            HttpStatus::OK => "OK".to_string(),
            HttpStatus::InternalServerError => "Internal Server Error".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn test_requete() {
        let req = Request::from(&vec![
            "GET /path?query=test&test=x HTTP/1.1".to_string(),
            "Host: localhost".to_string(),
        ])
        .unwrap();

        assert_eq!(req.method, Method::Get);
        assert_eq!(
            req.url,
            Url::from_str("http://localhost/path?query=test&test=x").unwrap()
        );

        let req = Request::from(&vec![
            "POST /path?query=test&test=x HTTP/1.1".to_string(),
            "Host: www.google.com".to_string(),
        ])
        .unwrap();

        assert_eq!(req.method, Method::Post);
        assert_eq!(
            req.url,
            Url::from_str("http://www.google.com/path?query=test&test=x").unwrap()
        );

        let req = Request::from(&vec![
            "PUT /path?query=test&test=x HTTP/1.1".to_string(),
            "Host: www.google.com".to_string(),
        ])
        .unwrap();

        assert_eq!(req.method, Method::Put);
        assert_eq!(
            req.url,
            Url::from_str("http://www.google.com/path?query=test&test=x").unwrap()
        );

        let req = Request::from(&vec![
            "DELETE /path?query=test&test=x HTTP/1.1".to_string(),
            "Host: www.google.com".to_string(),
        ])
        .unwrap();

        assert_eq!(req.method, Method::Delete);
        assert_eq!(
            req.url,
            Url::from_str("http://www.google.com/path?query=test&test=x").unwrap()
        );
    }

    #[test]
    fn test_response_to_http_protocol() {
        let resp_str = Response {
            status: HttpStatus::OK,
            body: Some("test".to_string()),
        }
        .to_http_protocol();
        assert_eq!(resp_str, "HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\ntest");

        let resp_str = Response {
            status: HttpStatus::OK,
            body: None,
        }
        .to_http_protocol();
        assert_eq!(resp_str, "HTTP/1.1 200 OK\r\n\r\n");

        let resp_str = Response {
            status: HttpStatus::InternalServerError,
            body: None,
        }
        .to_http_protocol();
        assert_eq!(resp_str, "HTTP/1.1 500 Internal Server Error\r\n\r\n");

        let resp_str = Response {
            status: HttpStatus::InternalServerError,
            body: Some("Error".to_string()),
        }
        .to_http_protocol();
        assert_eq!(
            resp_str,
            "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 5\r\n\r\nError"
        );
    }
}
