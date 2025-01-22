use std::{collections::HashMap, io::Error, net::SocketAddr, str::FromStr};

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{
    body::{self, Body, Buf},
    header::{
        self, HeaderValue, ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS,
        ACCESS_CONTROL_ALLOW_ORIGIN, AUTHORIZATION,
    },
    server::conn::http1,
    Method, Request, Response, StatusCode,
};
use hyper_util::{rt::TokioIo, service::TowerToHyperService};
use jsonwebtoken::{decode_header, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::{
    application::api::{person::person_router, speech::speech_router, token::Permissions},
    domain::{person::PersonManager, speech::manager::SpeechManager},
};

use super::{
    keycloak::get_keycloak_keys,
    token::{self, AuthToken},
};

type BoxBody = http_body_util::combinators::BoxBody<Bytes, hyper::Error>;

#[derive(Debug, Serialize)]
pub struct HttpError<'a> {
    code: u16,
    error: &'a str,
    details: &'a str,
}
impl<'a> HttpError<'a> {
    pub fn new(code: u16, error: &'a str, details: &'a str) -> Self {
        HttpError {
            code,
            error,
            details,
        }
    }
}

pub const INTERNAL_ERROR: HttpError = HttpError {
    code: 500,
    error: "InternalError",
    details: "An internal error occured, please contact our technical service",
};

pub const NOT_FOUND_ERROR: HttpError = HttpError {
    code: 404,
    error: "NotFound",
    details: "The requested resource is not found",
};

pub const ACCESS_DENIED_ERROR: HttpError = HttpError {
    code: 403,
    error: "AccessDenied",
    details: "You cannot access to this ressource",
};
pub enum APIError {
    ConfigurationError(String),
    RequestError(HttpError<'static>),
}

impl From<APIError> for Response<BoxBody> {
    fn from(value: APIError) -> Self {
        match value {
            APIError::RequestError(err) => {
                return Response::builder()
                    .status(err.code)
                    .body(full(serde_json::to_string(&err).expect("Should not fail")))
                    .expect("Should not fail");
            }
            _ => {
                panic!("A fatal error occured")
            }
        }
    }
}

pub struct MainRouter {
    person_manager: PersonManager,
    speech_manager: SpeechManager,
}

impl MainRouter {
    pub fn new(person_manager: PersonManager, speech_manager: SpeechManager) -> Self {
        return Self {
            person_manager,
            speech_manager,
        };
    }

    pub async fn run(&self) -> Result<(), APIError> {
        let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| APIError::ConfigurationError(e.to_string()))?;
        // We start a loop to continuously accept incoming connections
        loop {
            let (stream, _) = listener
                .accept()
                .await
                .map_err(|e| APIError::ConfigurationError(e.to_string()))?;

            // Use an adapter to access something implementing `tokio::io` traits as if they implement
            // `hyper::rt` IO traits.
            let io = TokioIo::new(stream);

            let person_manager_cloned = self.person_manager.clone();
            let speech_manager_cloned = self.speech_manager.clone();
            tokio::task::spawn(async move {
                let cors = CorsLayer::new()
                    .allow_origin(AllowOrigin::any()) // Autoriser toutes les origines (pour le développement)
                    .allow_methods(vec![Method::GET, Method::POST, Method::OPTIONS]) // Autoriser certaines méthodes HTTP
                    .allow_headers(vec![header::CONTENT_TYPE, AUTHORIZATION]);
                let service = ServiceBuilder::new().layer(cors).service_fn(|r| {
                    let person_manager_cloned = person_manager_cloned.clone();
                    let speech_manager_cloned = speech_manager_cloned.clone();
                    async {
                        let res =
                            match route_requests(r, person_manager_cloned, speech_manager_cloned)
                                .await
                            {
                                Ok(r) => r,
                                Err(e) => e.into(),
                            };
                        Ok::<
                            Response<
                                http_body_util::combinators::BoxBody<bytes::Bytes, hyper::Error>,
                            >,
                            Error,
                        >(res)
                    }
                });
                if let Err(err) = http1::Builder::new()
                    .serve_connection(io, TowerToHyperService::new(service))
                    .await
                {
                    eprintln!("Error serving connection: {:?}", err);
                }
            });
        }
    }
}

async fn route_requests(
    request: Request<body::Incoming>,
    person_manager: PersonManager,
    speech_manager: SpeechManager,
) -> Result<Response<BoxBody>, APIError> {
    let path = request.uri().path().to_string();
    let params = match request.uri().query() {
        Some(val) => val.to_string(),
        None => Default::default(),
    };
    let method = request.method().clone();
    println!("Request {}:{}", method.as_str(), path);
    let headers = request.headers().clone();
    let whole_body = request
        .collect()
        .await
        .map_err(|e| {
            println!("An internal error occured: {:?}", e);
            APIError::RequestError(INTERNAL_ERROR)
        })?
        .aggregate();
    let body: serde_json::Value =
        serde_json::from_reader(whole_body.reader()).unwrap_or(serde_json::Value::Null);
    let mut splitted_path = path.split("/").skip(1);
    match splitted_path.next() {
        Some(api_str) => {
            if api_str != "api" {
                return Err(APIError::RequestError(HttpError {
                    code: 400,
                    error: "InvalidRoute",
                    details: "The route format seems invalid",
                }));
            }
        }
        None => return Err(APIError::RequestError(NOT_FOUND_ERROR)),
    }
    let query_params = get_query_params_from_raw(&params);
    let keycloak_keys = get_keycloak_keys().await.map_err(|e| {
        println!("An internal error occured: {}", e);
        APIError::RequestError(INTERNAL_ERROR)
    })?;
    let token = extract_token(
        headers
            .get("Authorization")
            .unwrap_or(&HeaderValue::from_static(""))
            .to_str()
            .unwrap_or(""),
        keycloak_keys,
    )
    .map_err(|e| APIError::RequestError(e))?;
    let resp = match splitted_path.next() {
        Some(val) => {
            let partial_path = &splitted_path.collect::<Vec<&str>>().join("/");
            match val {
                "person" => {
                    person_router::router(
                        partial_path,
                        &query_params,
                        &method,
                        &token,
                        body,
                        &person_manager,
                    )
                    .await
                }
                "speech" => {
                    speech_router::router(
                        partial_path,
                        &query_params,
                        &method,
                        &token,
                        body,
                        &speech_manager,
                    )
                    .await
                }
                _ => return Err(APIError::RequestError(NOT_FOUND_ERROR)),
            }
        }
        None => return Err(APIError::RequestError(NOT_FOUND_ERROR)),
    }
    .map_err(|e| {
        println!("An error occured: {:?}", e);
        APIError::RequestError(e)
    })?;
    return Ok(Response::builder()
        .status(200)
        .body(full(serde_json::to_string(&resp).unwrap()))
        .unwrap());
}

fn full<T: Into<Bytes>>(chunk: T) -> BoxBody {
    Full::new(chunk.into())
        .map_err(|never| match never {})
        .boxed()
}

fn get_query_params_from_raw(raw_params: &str) -> HashMap<String, String> {
    let mut query_params = HashMap::new();
    let query_params_list = raw_params.split("&");
    for query_param in query_params_list {
        let mut param_splitted = query_param.split("=");
        let var = param_splitted.next();
        let val = param_splitted.next();
        if var.is_some() && val.is_some() {
            query_params.insert(var.unwrap().to_string(), val.unwrap().to_string());
        }
    }
    query_params
}

fn extract_token(
    raw_token: &str,
    keys: HashMap<String, DecodingKey>,
) -> Result<AuthToken, HttpError<'static>> {
    let invalid_token = HttpError::new(400, "InvalidToken", "The token you provided is invalid");
    if raw_token.is_empty() {
        return Ok(AuthToken::default());
    }
    let token_part = match raw_token.split("Bearer ").skip(1).next() {
        Some(token) => token,
        None => return Err(invalid_token),
    };
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&["speech-analytics-front-end"]);
    // Décoder l'en-tête du JWT pour récupérer le "kid" (Key ID)
    let header = match decode_header(token_part) {
        Ok(v) => v,
        Err(e) => return Err(invalid_token),
    };
    let kid = match header.kid {
        Some(kid) => kid,
        None => return Err(invalid_token),
    };
    // Trouver la clé correspondant au `kid`
    let decoding_key = match keys.get(&kid) {
        Some(key) => key,
        None => return Err(invalid_token),
    };
    let decoded = match jsonwebtoken::decode(token_part, decoding_key, &validation) {
        Ok(res) => res.claims,
        Err(e) => {
            println!("Token error : {:?}", e);
            return Err(invalid_token);
        }
    };

    Ok(decoded)
}
