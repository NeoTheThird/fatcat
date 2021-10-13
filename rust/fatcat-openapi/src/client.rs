#![allow(unused_extern_crates)]
extern crate chrono;
extern crate hyper_openssl;
extern crate url;

use self::hyper_openssl::openssl;
use self::url::percent_encoding::{utf8_percent_encode, PATH_SEGMENT_ENCODE_SET, QUERY_ENCODE_SET};
use futures;
use futures::{future, stream};
use futures::{Future, Stream};
use hyper;
use hyper::client::IntoUrl;
use hyper::header::{ContentType, Headers};
use hyper::mime;
use hyper::mime::{Attr, Mime, SubLevel, TopLevel, Value};
use hyper::Url;
use std::borrow::Cow;
use std::error;
use std::fmt;
use std::io::{Error, Read};
use std::path::Path;
use std::str;
use std::sync::Arc;

use crate::mimetypes;

use serde_json;

#[allow(unused_imports)]
use std::collections::{BTreeMap, HashMap};
#[allow(unused_imports)]
use swagger;

use swagger::{ApiError, Context, XSpanId};

use crate::models;
use crate::{
    AcceptEditgroupResponse, Api, AuthCheckResponse, AuthOidcResponse, CreateAuthTokenResponse, CreateContainerAutoBatchResponse, CreateContainerResponse, CreateCreatorAutoBatchResponse,
    CreateCreatorResponse, CreateEditgroupAnnotationResponse, CreateEditgroupResponse, CreateFileAutoBatchResponse, CreateFileResponse, CreateFilesetAutoBatchResponse, CreateFilesetResponse,
    CreateReleaseAutoBatchResponse, CreateReleaseResponse, CreateWebcaptureAutoBatchResponse, CreateWebcaptureResponse, CreateWorkAutoBatchResponse, CreateWorkResponse, DeleteContainerEditResponse,
    DeleteContainerResponse, DeleteCreatorEditResponse, DeleteCreatorResponse, DeleteFileEditResponse, DeleteFileResponse, DeleteFilesetEditResponse, DeleteFilesetResponse, DeleteReleaseEditResponse,
    DeleteReleaseResponse, DeleteWebcaptureEditResponse, DeleteWebcaptureResponse, DeleteWorkEditResponse, DeleteWorkResponse, GetChangelogEntryResponse, GetChangelogResponse,
    GetContainerEditResponse, GetContainerHistoryResponse, GetContainerRedirectsResponse, GetContainerResponse, GetContainerRevisionResponse, GetCreatorEditResponse, GetCreatorHistoryResponse,
    GetCreatorRedirectsResponse, GetCreatorReleasesResponse, GetCreatorResponse, GetCreatorRevisionResponse, GetEditgroupAnnotationsResponse, GetEditgroupResponse, GetEditgroupsReviewableResponse,
    GetEditorAnnotationsResponse, GetEditorEditgroupsResponse, GetEditorResponse, GetFileEditResponse, GetFileHistoryResponse, GetFileRedirectsResponse, GetFileResponse, GetFileRevisionResponse,
    GetFilesetEditResponse, GetFilesetHistoryResponse, GetFilesetRedirectsResponse, GetFilesetResponse, GetFilesetRevisionResponse, GetReleaseEditResponse, GetReleaseFilesResponse,
    GetReleaseFilesetsResponse, GetReleaseHistoryResponse, GetReleaseRedirectsResponse, GetReleaseResponse, GetReleaseRevisionResponse, GetReleaseWebcapturesResponse, GetWebcaptureEditResponse,
    GetWebcaptureHistoryResponse, GetWebcaptureRedirectsResponse, GetWebcaptureResponse, GetWebcaptureRevisionResponse, GetWorkEditResponse, GetWorkHistoryResponse, GetWorkRedirectsResponse,
    GetWorkReleasesResponse, GetWorkResponse, GetWorkRevisionResponse, LookupContainerResponse, LookupCreatorResponse, LookupEditorResponse, LookupFileResponse, LookupReleaseResponse,
    UpdateContainerResponse, UpdateCreatorResponse, UpdateEditgroupResponse, UpdateEditorResponse, UpdateFileResponse, UpdateFilesetResponse, UpdateReleaseResponse, UpdateWebcaptureResponse,
    UpdateWorkResponse,
};

/// Convert input into a base path, e.g. "http://example:123". Also checks the scheme as it goes.
fn into_base_path<T: IntoUrl>(input: T, correct_scheme: Option<&'static str>) -> Result<String, ClientInitError> {
    // First convert to Url, since a base path is a subset of Url.
    let url = input.into_url()?;

    let scheme = url.scheme();

    // Check the scheme if necessary
    if let Some(correct_scheme) = correct_scheme {
        if scheme != correct_scheme {
            return Err(ClientInitError::InvalidScheme);
        }
    }

    let host = url.host().ok_or_else(|| ClientInitError::MissingHost)?;
    let port = url.port().map(|x| format!(":{}", x)).unwrap_or_default();
    Ok(format!("{}://{}{}", scheme, host, port))
}

/// A client that implements the API by making HTTP calls out to a server.
#[derive(Clone)]
pub struct Client {
    base_path: String,
    hyper_client: Arc<Fn() -> hyper::client::Client + Sync + Send>,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Client {{ base_path: {} }}", self.base_path)
    }
}

impl Client {
    pub fn try_new_http<T>(base_path: T) -> Result<Client, ClientInitError>
    where
        T: IntoUrl,
    {
        Ok(Client {
            base_path: into_base_path(base_path, Some("http"))?,
            hyper_client: Arc::new(hyper::client::Client::new),
        })
    }

    pub fn try_new_https<T, CA>(base_path: T, ca_certificate: CA) -> Result<Client, ClientInitError>
    where
        T: IntoUrl,
        CA: AsRef<Path>,
    {
        let ca_certificate = ca_certificate.as_ref().to_owned();

        let https_hyper_client = move || {
            // SSL implementation
            let mut ssl = openssl::ssl::SslConnectorBuilder::new(openssl::ssl::SslMethod::tls()).unwrap();

            // Server authentication
            ssl.set_ca_file(ca_certificate.clone()).unwrap();

            let ssl = hyper_openssl::OpensslClient::from(ssl.build());
            let connector = hyper::net::HttpsConnector::new(ssl);
            hyper::client::Client::with_connector(connector)
        };

        Ok(Client {
            base_path: into_base_path(base_path, Some("https"))?,
            hyper_client: Arc::new(https_hyper_client),
        })
    }

    pub fn try_new_https_mutual<T, CA, K, C>(base_path: T, ca_certificate: CA, client_key: K, client_certificate: C) -> Result<Client, ClientInitError>
    where
        T: IntoUrl,
        CA: AsRef<Path>,
        K: AsRef<Path>,
        C: AsRef<Path>,
    {
        let ca_certificate = ca_certificate.as_ref().to_owned();
        let client_key = client_key.as_ref().to_owned();
        let client_certificate = client_certificate.as_ref().to_owned();

        let https_mutual_hyper_client = move || {
            // SSL implementation
            let mut ssl = openssl::ssl::SslConnectorBuilder::new(openssl::ssl::SslMethod::tls()).unwrap();

            // Server authentication
            ssl.set_ca_file(ca_certificate.clone()).unwrap();

            // Client authentication
            ssl.set_private_key_file(client_key.clone(), openssl::x509::X509_FILETYPE_PEM).unwrap();
            ssl.set_certificate_chain_file(client_certificate.clone()).unwrap();
            ssl.check_private_key().unwrap();

            let ssl = hyper_openssl::OpensslClient::from(ssl.build());
            let connector = hyper::net::HttpsConnector::new(ssl);
            hyper::client::Client::with_connector(connector)
        };

        Ok(Client {
            base_path: into_base_path(base_path, Some("https"))?,
            hyper_client: Arc::new(https_mutual_hyper_client),
        })
    }

    /// Constructor for creating a `Client` by passing in a pre-made `hyper` client.
    ///
    /// One should avoid relying on this function if possible, since it adds a dependency on the underlying transport
    /// implementation, which it would be better to abstract away. Therefore, using this function may lead to a loss of
    /// code generality, which may make it harder to move the application to a serverless environment, for example.
    ///
    /// The reason for this function's existence is to support legacy test code, which did mocking at the hyper layer.
    /// This is not a recommended way to write new tests. If other reasons are found for using this function, they
    /// should be mentioned here.
    pub fn try_new_with_hyper_client<T>(base_path: T, hyper_client: Arc<Fn() -> hyper::client::Client + Sync + Send>) -> Result<Client, ClientInitError>
    where
        T: IntoUrl,
    {
        Ok(Client {
            base_path: into_base_path(base_path, None)?,
            hyper_client: hyper_client,
        })
    }
}

impl Api for Client {
    fn auth_check(&self, param_role: Option<String>, context: &Context) -> Box<Future<Item = AuthCheckResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_role = param_role.map_or_else(String::new, |query| format!("role={role}&", role = query.to_string()));

        let url = format!("{}/v0/auth/check?{role}", self.base_path, role = utf8_percent_encode(&query_role, QUERY_ENCODE_SET));

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<AuthCheckResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::Success>(&buf)?;

                    Ok(AuthCheckResponse::Success(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(AuthCheckResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(AuthCheckResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(AuthCheckResponse::Forbidden(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(AuthCheckResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn auth_oidc(&self, param_oidc_params: models::AuthOidc, context: &Context) -> Box<Future<Item = AuthOidcResponse, Error = ApiError> + Send> {
        let url = format!("{}/v0/auth/oidc", self.base_path);

        let body = serde_json::to_string(&param_oidc_params).expect("impossible to fail to serialize");

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Post, &url);
        let mut custom_headers = hyper::header::Headers::new();

        let request = request.body(&body);

        custom_headers.set(ContentType(mimetypes::requests::AUTH_OIDC.clone()));
        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<AuthOidcResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::AuthOidcResult>(&buf)?;

                    Ok(AuthOidcResponse::Found(body))
                }
                201 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::AuthOidcResult>(&buf)?;

                    Ok(AuthOidcResponse::Created(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(AuthOidcResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(AuthOidcResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(AuthOidcResponse::Forbidden(body))
                }
                409 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(AuthOidcResponse::Conflict(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(AuthOidcResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn create_auth_token(&self, param_editor_id: String, param_duration_seconds: Option<i32>, context: &Context) -> Box<Future<Item = CreateAuthTokenResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_duration_seconds = param_duration_seconds.map_or_else(String::new, |query| format!("duration_seconds={duration_seconds}&", duration_seconds = query.to_string()));

        let url = format!(
            "{}/v0/auth/token/{editor_id}?{duration_seconds}",
            self.base_path,
            editor_id = utf8_percent_encode(&param_editor_id.to_string(), PATH_SEGMENT_ENCODE_SET),
            duration_seconds = utf8_percent_encode(&query_duration_seconds, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Post, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<CreateAuthTokenResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::AuthTokenResult>(&buf)?;

                    Ok(CreateAuthTokenResponse::Success(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateAuthTokenResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(CreateAuthTokenResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateAuthTokenResponse::Forbidden(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateAuthTokenResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_changelog(&self, param_limit: Option<i64>, context: &Context) -> Box<Future<Item = GetChangelogResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_limit = param_limit.map_or_else(String::new, |query| format!("limit={limit}&", limit = query.to_string()));

        let url = format!("{}/v0/changelog?{limit}", self.base_path, limit = utf8_percent_encode(&query_limit, QUERY_ENCODE_SET));

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetChangelogResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<Vec<models::ChangelogEntry>>(&buf)?;

                    Ok(GetChangelogResponse::Success(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetChangelogResponse::BadRequest(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetChangelogResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_changelog_entry(&self, param_index: i64, context: &Context) -> Box<Future<Item = GetChangelogEntryResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/changelog/{index}",
            self.base_path,
            index = utf8_percent_encode(&param_index.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetChangelogEntryResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ChangelogEntry>(&buf)?;

                    Ok(GetChangelogEntryResponse::FoundChangelogEntry(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetChangelogEntryResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetChangelogEntryResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetChangelogEntryResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn create_container(&self, param_editgroup_id: String, param_entity: models::ContainerEntity, context: &Context) -> Box<Future<Item = CreateContainerResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/editgroup/{editgroup_id}/container",
            self.base_path,
            editgroup_id = utf8_percent_encode(&param_editgroup_id.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let body = serde_json::to_string(&param_entity).expect("impossible to fail to serialize");

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Post, &url);
        let mut custom_headers = hyper::header::Headers::new();

        let request = request.body(&body);

        custom_headers.set(ContentType(mimetypes::requests::CREATE_CONTAINER.clone()));
        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<CreateContainerResponse, ApiError> {
            match response.status.to_u16() {
                201 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::EntityEdit>(&buf)?;

                    Ok(CreateContainerResponse::CreatedEntity(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateContainerResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(CreateContainerResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateContainerResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateContainerResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateContainerResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn create_container_auto_batch(&self, param_auto_batch: models::ContainerAutoBatch, context: &Context) -> Box<Future<Item = CreateContainerAutoBatchResponse, Error = ApiError> + Send> {
        let url = format!("{}/v0/editgroup/auto/container/batch", self.base_path);

        let body = serde_json::to_string(&param_auto_batch).expect("impossible to fail to serialize");

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Post, &url);
        let mut custom_headers = hyper::header::Headers::new();

        let request = request.body(&body);

        custom_headers.set(ContentType(mimetypes::requests::CREATE_CONTAINER_AUTO_BATCH.clone()));
        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<CreateContainerAutoBatchResponse, ApiError> {
            match response.status.to_u16() {
                201 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::Editgroup>(&buf)?;

                    Ok(CreateContainerAutoBatchResponse::CreatedEditgroup(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateContainerAutoBatchResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(CreateContainerAutoBatchResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateContainerAutoBatchResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateContainerAutoBatchResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateContainerAutoBatchResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn delete_container(&self, param_editgroup_id: String, param_ident: String, context: &Context) -> Box<Future<Item = DeleteContainerResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/editgroup/{editgroup_id}/container/{ident}",
            self.base_path,
            editgroup_id = utf8_percent_encode(&param_editgroup_id.to_string(), PATH_SEGMENT_ENCODE_SET),
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Delete, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<DeleteContainerResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::EntityEdit>(&buf)?;

                    Ok(DeleteContainerResponse::DeletedEntity(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteContainerResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(DeleteContainerResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteContainerResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteContainerResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteContainerResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn delete_container_edit(&self, param_editgroup_id: String, param_edit_id: String, context: &Context) -> Box<Future<Item = DeleteContainerEditResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/editgroup/{editgroup_id}/container/edit/{edit_id}",
            self.base_path,
            editgroup_id = utf8_percent_encode(&param_editgroup_id.to_string(), PATH_SEGMENT_ENCODE_SET),
            edit_id = utf8_percent_encode(&param_edit_id.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Delete, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<DeleteContainerEditResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::Success>(&buf)?;

                    Ok(DeleteContainerEditResponse::DeletedEdit(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteContainerEditResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(DeleteContainerEditResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteContainerEditResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteContainerEditResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteContainerEditResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_container(&self, param_ident: String, param_expand: Option<String>, param_hide: Option<String>, context: &Context) -> Box<Future<Item = GetContainerResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_expand = param_expand.map_or_else(String::new, |query| format!("expand={expand}&", expand = query.to_string()));
        let query_hide = param_hide.map_or_else(String::new, |query| format!("hide={hide}&", hide = query.to_string()));

        let url = format!(
            "{}/v0/container/{ident}?{expand}{hide}",
            self.base_path,
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET),
            expand = utf8_percent_encode(&query_expand, QUERY_ENCODE_SET),
            hide = utf8_percent_encode(&query_hide, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetContainerResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ContainerEntity>(&buf)?;

                    Ok(GetContainerResponse::FoundEntity(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetContainerResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetContainerResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetContainerResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_container_edit(&self, param_edit_id: String, context: &Context) -> Box<Future<Item = GetContainerEditResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/container/edit/{edit_id}",
            self.base_path,
            edit_id = utf8_percent_encode(&param_edit_id.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetContainerEditResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::EntityEdit>(&buf)?;

                    Ok(GetContainerEditResponse::FoundEdit(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetContainerEditResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetContainerEditResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetContainerEditResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_container_history(&self, param_ident: String, param_limit: Option<i64>, context: &Context) -> Box<Future<Item = GetContainerHistoryResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_limit = param_limit.map_or_else(String::new, |query| format!("limit={limit}&", limit = query.to_string()));

        let url = format!(
            "{}/v0/container/{ident}/history?{limit}",
            self.base_path,
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET),
            limit = utf8_percent_encode(&query_limit, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetContainerHistoryResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<Vec<models::EntityHistoryEntry>>(&buf)?;

                    Ok(GetContainerHistoryResponse::FoundEntityHistory(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetContainerHistoryResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetContainerHistoryResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetContainerHistoryResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_container_redirects(&self, param_ident: String, context: &Context) -> Box<Future<Item = GetContainerRedirectsResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/container/{ident}/redirects",
            self.base_path,
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetContainerRedirectsResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<Vec<String>>(&buf)?;

                    Ok(GetContainerRedirectsResponse::FoundEntityRedirects(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetContainerRedirectsResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetContainerRedirectsResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetContainerRedirectsResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_container_revision(
        &self,
        param_rev_id: String,
        param_expand: Option<String>,
        param_hide: Option<String>,
        context: &Context,
    ) -> Box<Future<Item = GetContainerRevisionResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_expand = param_expand.map_or_else(String::new, |query| format!("expand={expand}&", expand = query.to_string()));
        let query_hide = param_hide.map_or_else(String::new, |query| format!("hide={hide}&", hide = query.to_string()));

        let url = format!(
            "{}/v0/container/rev/{rev_id}?{expand}{hide}",
            self.base_path,
            rev_id = utf8_percent_encode(&param_rev_id.to_string(), PATH_SEGMENT_ENCODE_SET),
            expand = utf8_percent_encode(&query_expand, QUERY_ENCODE_SET),
            hide = utf8_percent_encode(&query_hide, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetContainerRevisionResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ContainerEntity>(&buf)?;

                    Ok(GetContainerRevisionResponse::FoundEntityRevision(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetContainerRevisionResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetContainerRevisionResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetContainerRevisionResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn lookup_container(
        &self,
        param_issnl: Option<String>,
        param_issne: Option<String>,
        param_issnp: Option<String>,
        param_issn: Option<String>,
        param_wikidata_qid: Option<String>,
        param_expand: Option<String>,
        param_hide: Option<String>,
        context: &Context,
    ) -> Box<Future<Item = LookupContainerResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_issnl = param_issnl.map_or_else(String::new, |query| format!("issnl={issnl}&", issnl = query.to_string()));
        let query_issne = param_issne.map_or_else(String::new, |query| format!("issne={issne}&", issne = query.to_string()));
        let query_issnp = param_issnp.map_or_else(String::new, |query| format!("issnp={issnp}&", issnp = query.to_string()));
        let query_issn = param_issn.map_or_else(String::new, |query| format!("issn={issn}&", issn = query.to_string()));
        let query_wikidata_qid = param_wikidata_qid.map_or_else(String::new, |query| format!("wikidata_qid={wikidata_qid}&", wikidata_qid = query.to_string()));
        let query_expand = param_expand.map_or_else(String::new, |query| format!("expand={expand}&", expand = query.to_string()));
        let query_hide = param_hide.map_or_else(String::new, |query| format!("hide={hide}&", hide = query.to_string()));

        let url = format!(
            "{}/v0/container/lookup?{issnl}{issne}{issnp}{issn}{wikidata_qid}{expand}{hide}",
            self.base_path,
            issnl = utf8_percent_encode(&query_issnl, QUERY_ENCODE_SET),
            issne = utf8_percent_encode(&query_issne, QUERY_ENCODE_SET),
            issnp = utf8_percent_encode(&query_issnp, QUERY_ENCODE_SET),
            issn = utf8_percent_encode(&query_issn, QUERY_ENCODE_SET),
            wikidata_qid = utf8_percent_encode(&query_wikidata_qid, QUERY_ENCODE_SET),
            expand = utf8_percent_encode(&query_expand, QUERY_ENCODE_SET),
            hide = utf8_percent_encode(&query_hide, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<LookupContainerResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ContainerEntity>(&buf)?;

                    Ok(LookupContainerResponse::FoundEntity(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(LookupContainerResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(LookupContainerResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(LookupContainerResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn update_container(
        &self,
        param_editgroup_id: String,
        param_ident: String,
        param_entity: models::ContainerEntity,
        context: &Context,
    ) -> Box<Future<Item = UpdateContainerResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/editgroup/{editgroup_id}/container/{ident}",
            self.base_path,
            editgroup_id = utf8_percent_encode(&param_editgroup_id.to_string(), PATH_SEGMENT_ENCODE_SET),
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let body = serde_json::to_string(&param_entity).expect("impossible to fail to serialize");

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Put, &url);
        let mut custom_headers = hyper::header::Headers::new();

        let request = request.body(&body);

        custom_headers.set(ContentType(mimetypes::requests::UPDATE_CONTAINER.clone()));
        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<UpdateContainerResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::EntityEdit>(&buf)?;

                    Ok(UpdateContainerResponse::UpdatedEntity(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateContainerResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(UpdateContainerResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateContainerResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateContainerResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateContainerResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn create_creator(&self, param_editgroup_id: String, param_entity: models::CreatorEntity, context: &Context) -> Box<Future<Item = CreateCreatorResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/editgroup/{editgroup_id}/creator",
            self.base_path,
            editgroup_id = utf8_percent_encode(&param_editgroup_id.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let body = serde_json::to_string(&param_entity).expect("impossible to fail to serialize");

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Post, &url);
        let mut custom_headers = hyper::header::Headers::new();

        let request = request.body(&body);

        custom_headers.set(ContentType(mimetypes::requests::CREATE_CREATOR.clone()));
        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<CreateCreatorResponse, ApiError> {
            match response.status.to_u16() {
                201 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::EntityEdit>(&buf)?;

                    Ok(CreateCreatorResponse::CreatedEntity(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateCreatorResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(CreateCreatorResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateCreatorResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateCreatorResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateCreatorResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn create_creator_auto_batch(&self, param_auto_batch: models::CreatorAutoBatch, context: &Context) -> Box<Future<Item = CreateCreatorAutoBatchResponse, Error = ApiError> + Send> {
        let url = format!("{}/v0/editgroup/auto/creator/batch", self.base_path);

        let body = serde_json::to_string(&param_auto_batch).expect("impossible to fail to serialize");

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Post, &url);
        let mut custom_headers = hyper::header::Headers::new();

        let request = request.body(&body);

        custom_headers.set(ContentType(mimetypes::requests::CREATE_CREATOR_AUTO_BATCH.clone()));
        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<CreateCreatorAutoBatchResponse, ApiError> {
            match response.status.to_u16() {
                201 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::Editgroup>(&buf)?;

                    Ok(CreateCreatorAutoBatchResponse::CreatedEditgroup(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateCreatorAutoBatchResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(CreateCreatorAutoBatchResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateCreatorAutoBatchResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateCreatorAutoBatchResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateCreatorAutoBatchResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn delete_creator(&self, param_editgroup_id: String, param_ident: String, context: &Context) -> Box<Future<Item = DeleteCreatorResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/editgroup/{editgroup_id}/creator/{ident}",
            self.base_path,
            editgroup_id = utf8_percent_encode(&param_editgroup_id.to_string(), PATH_SEGMENT_ENCODE_SET),
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Delete, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<DeleteCreatorResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::EntityEdit>(&buf)?;

                    Ok(DeleteCreatorResponse::DeletedEntity(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteCreatorResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(DeleteCreatorResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteCreatorResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteCreatorResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteCreatorResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn delete_creator_edit(&self, param_editgroup_id: String, param_edit_id: String, context: &Context) -> Box<Future<Item = DeleteCreatorEditResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/editgroup/{editgroup_id}/creator/edit/{edit_id}",
            self.base_path,
            editgroup_id = utf8_percent_encode(&param_editgroup_id.to_string(), PATH_SEGMENT_ENCODE_SET),
            edit_id = utf8_percent_encode(&param_edit_id.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Delete, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<DeleteCreatorEditResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::Success>(&buf)?;

                    Ok(DeleteCreatorEditResponse::DeletedEdit(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteCreatorEditResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(DeleteCreatorEditResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteCreatorEditResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteCreatorEditResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteCreatorEditResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_creator(&self, param_ident: String, param_expand: Option<String>, param_hide: Option<String>, context: &Context) -> Box<Future<Item = GetCreatorResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_expand = param_expand.map_or_else(String::new, |query| format!("expand={expand}&", expand = query.to_string()));
        let query_hide = param_hide.map_or_else(String::new, |query| format!("hide={hide}&", hide = query.to_string()));

        let url = format!(
            "{}/v0/creator/{ident}?{expand}{hide}",
            self.base_path,
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET),
            expand = utf8_percent_encode(&query_expand, QUERY_ENCODE_SET),
            hide = utf8_percent_encode(&query_hide, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetCreatorResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::CreatorEntity>(&buf)?;

                    Ok(GetCreatorResponse::FoundEntity(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetCreatorResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetCreatorResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetCreatorResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_creator_edit(&self, param_edit_id: String, context: &Context) -> Box<Future<Item = GetCreatorEditResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/creator/edit/{edit_id}",
            self.base_path,
            edit_id = utf8_percent_encode(&param_edit_id.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetCreatorEditResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::EntityEdit>(&buf)?;

                    Ok(GetCreatorEditResponse::FoundEdit(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetCreatorEditResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetCreatorEditResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetCreatorEditResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_creator_history(&self, param_ident: String, param_limit: Option<i64>, context: &Context) -> Box<Future<Item = GetCreatorHistoryResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_limit = param_limit.map_or_else(String::new, |query| format!("limit={limit}&", limit = query.to_string()));

        let url = format!(
            "{}/v0/creator/{ident}/history?{limit}",
            self.base_path,
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET),
            limit = utf8_percent_encode(&query_limit, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetCreatorHistoryResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<Vec<models::EntityHistoryEntry>>(&buf)?;

                    Ok(GetCreatorHistoryResponse::FoundEntityHistory(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetCreatorHistoryResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetCreatorHistoryResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetCreatorHistoryResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_creator_redirects(&self, param_ident: String, context: &Context) -> Box<Future<Item = GetCreatorRedirectsResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/creator/{ident}/redirects",
            self.base_path,
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetCreatorRedirectsResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<Vec<String>>(&buf)?;

                    Ok(GetCreatorRedirectsResponse::FoundEntityRedirects(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetCreatorRedirectsResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetCreatorRedirectsResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetCreatorRedirectsResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_creator_releases(&self, param_ident: String, param_hide: Option<String>, context: &Context) -> Box<Future<Item = GetCreatorReleasesResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_hide = param_hide.map_or_else(String::new, |query| format!("hide={hide}&", hide = query.to_string()));

        let url = format!(
            "{}/v0/creator/{ident}/releases?{hide}",
            self.base_path,
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET),
            hide = utf8_percent_encode(&query_hide, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetCreatorReleasesResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<Vec<models::ReleaseEntity>>(&buf)?;

                    Ok(GetCreatorReleasesResponse::Found(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetCreatorReleasesResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetCreatorReleasesResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetCreatorReleasesResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_creator_revision(
        &self,
        param_rev_id: String,
        param_expand: Option<String>,
        param_hide: Option<String>,
        context: &Context,
    ) -> Box<Future<Item = GetCreatorRevisionResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_expand = param_expand.map_or_else(String::new, |query| format!("expand={expand}&", expand = query.to_string()));
        let query_hide = param_hide.map_or_else(String::new, |query| format!("hide={hide}&", hide = query.to_string()));

        let url = format!(
            "{}/v0/creator/rev/{rev_id}?{expand}{hide}",
            self.base_path,
            rev_id = utf8_percent_encode(&param_rev_id.to_string(), PATH_SEGMENT_ENCODE_SET),
            expand = utf8_percent_encode(&query_expand, QUERY_ENCODE_SET),
            hide = utf8_percent_encode(&query_hide, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetCreatorRevisionResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::CreatorEntity>(&buf)?;

                    Ok(GetCreatorRevisionResponse::FoundEntityRevision(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetCreatorRevisionResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetCreatorRevisionResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetCreatorRevisionResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn lookup_creator(
        &self,
        param_orcid: Option<String>,
        param_wikidata_qid: Option<String>,
        param_expand: Option<String>,
        param_hide: Option<String>,
        context: &Context,
    ) -> Box<Future<Item = LookupCreatorResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_orcid = param_orcid.map_or_else(String::new, |query| format!("orcid={orcid}&", orcid = query.to_string()));
        let query_wikidata_qid = param_wikidata_qid.map_or_else(String::new, |query| format!("wikidata_qid={wikidata_qid}&", wikidata_qid = query.to_string()));
        let query_expand = param_expand.map_or_else(String::new, |query| format!("expand={expand}&", expand = query.to_string()));
        let query_hide = param_hide.map_or_else(String::new, |query| format!("hide={hide}&", hide = query.to_string()));

        let url = format!(
            "{}/v0/creator/lookup?{orcid}{wikidata_qid}{expand}{hide}",
            self.base_path,
            orcid = utf8_percent_encode(&query_orcid, QUERY_ENCODE_SET),
            wikidata_qid = utf8_percent_encode(&query_wikidata_qid, QUERY_ENCODE_SET),
            expand = utf8_percent_encode(&query_expand, QUERY_ENCODE_SET),
            hide = utf8_percent_encode(&query_hide, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<LookupCreatorResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::CreatorEntity>(&buf)?;

                    Ok(LookupCreatorResponse::FoundEntity(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(LookupCreatorResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(LookupCreatorResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(LookupCreatorResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn update_creator(
        &self,
        param_editgroup_id: String,
        param_ident: String,
        param_entity: models::CreatorEntity,
        context: &Context,
    ) -> Box<Future<Item = UpdateCreatorResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/editgroup/{editgroup_id}/creator/{ident}",
            self.base_path,
            editgroup_id = utf8_percent_encode(&param_editgroup_id.to_string(), PATH_SEGMENT_ENCODE_SET),
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let body = serde_json::to_string(&param_entity).expect("impossible to fail to serialize");

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Put, &url);
        let mut custom_headers = hyper::header::Headers::new();

        let request = request.body(&body);

        custom_headers.set(ContentType(mimetypes::requests::UPDATE_CREATOR.clone()));
        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<UpdateCreatorResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::EntityEdit>(&buf)?;

                    Ok(UpdateCreatorResponse::UpdatedEntity(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateCreatorResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(UpdateCreatorResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateCreatorResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateCreatorResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateCreatorResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn accept_editgroup(&self, param_editgroup_id: String, context: &Context) -> Box<Future<Item = AcceptEditgroupResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/editgroup/{editgroup_id}/accept",
            self.base_path,
            editgroup_id = utf8_percent_encode(&param_editgroup_id.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Post, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<AcceptEditgroupResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::Success>(&buf)?;

                    Ok(AcceptEditgroupResponse::MergedSuccessfully(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(AcceptEditgroupResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(AcceptEditgroupResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(AcceptEditgroupResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(AcceptEditgroupResponse::NotFound(body))
                }
                409 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(AcceptEditgroupResponse::EditConflict(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(AcceptEditgroupResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn create_editgroup(&self, param_editgroup: models::Editgroup, context: &Context) -> Box<Future<Item = CreateEditgroupResponse, Error = ApiError> + Send> {
        let url = format!("{}/v0/editgroup", self.base_path);

        let body = serde_json::to_string(&param_editgroup).expect("impossible to fail to serialize");

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Post, &url);
        let mut custom_headers = hyper::header::Headers::new();

        let request = request.body(&body);

        custom_headers.set(ContentType(mimetypes::requests::CREATE_EDITGROUP.clone()));
        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<CreateEditgroupResponse, ApiError> {
            match response.status.to_u16() {
                201 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::Editgroup>(&buf)?;

                    Ok(CreateEditgroupResponse::SuccessfullyCreated(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateEditgroupResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(CreateEditgroupResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateEditgroupResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateEditgroupResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateEditgroupResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn create_editgroup_annotation(
        &self,
        param_editgroup_id: String,
        param_annotation: models::EditgroupAnnotation,
        context: &Context,
    ) -> Box<Future<Item = CreateEditgroupAnnotationResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/editgroup/{editgroup_id}/annotation",
            self.base_path,
            editgroup_id = utf8_percent_encode(&param_editgroup_id.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let body = serde_json::to_string(&param_annotation).expect("impossible to fail to serialize");

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Post, &url);
        let mut custom_headers = hyper::header::Headers::new();

        let request = request.body(&body);

        custom_headers.set(ContentType(mimetypes::requests::CREATE_EDITGROUP_ANNOTATION.clone()));
        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<CreateEditgroupAnnotationResponse, ApiError> {
            match response.status.to_u16() {
                201 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::EditgroupAnnotation>(&buf)?;

                    Ok(CreateEditgroupAnnotationResponse::Created(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateEditgroupAnnotationResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(CreateEditgroupAnnotationResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateEditgroupAnnotationResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateEditgroupAnnotationResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateEditgroupAnnotationResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_editgroup(&self, param_editgroup_id: String, context: &Context) -> Box<Future<Item = GetEditgroupResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/editgroup/{editgroup_id}",
            self.base_path,
            editgroup_id = utf8_percent_encode(&param_editgroup_id.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetEditgroupResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::Editgroup>(&buf)?;

                    Ok(GetEditgroupResponse::Found(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetEditgroupResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetEditgroupResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetEditgroupResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_editgroup_annotations(&self, param_editgroup_id: String, param_expand: Option<String>, context: &Context) -> Box<Future<Item = GetEditgroupAnnotationsResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_expand = param_expand.map_or_else(String::new, |query| format!("expand={expand}&", expand = query.to_string()));

        let url = format!(
            "{}/v0/editgroup/{editgroup_id}/annotations?{expand}",
            self.base_path,
            editgroup_id = utf8_percent_encode(&param_editgroup_id.to_string(), PATH_SEGMENT_ENCODE_SET),
            expand = utf8_percent_encode(&query_expand, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetEditgroupAnnotationsResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<Vec<models::EditgroupAnnotation>>(&buf)?;

                    Ok(GetEditgroupAnnotationsResponse::Success(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetEditgroupAnnotationsResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(GetEditgroupAnnotationsResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetEditgroupAnnotationsResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetEditgroupAnnotationsResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetEditgroupAnnotationsResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_editgroups_reviewable(
        &self,
        param_expand: Option<String>,
        param_limit: Option<i64>,
        param_before: Option<chrono::DateTime<chrono::Utc>>,
        param_since: Option<chrono::DateTime<chrono::Utc>>,
        context: &Context,
    ) -> Box<Future<Item = GetEditgroupsReviewableResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_expand = param_expand.map_or_else(String::new, |query| format!("expand={expand}&", expand = query.to_string()));
        let query_limit = param_limit.map_or_else(String::new, |query| format!("limit={limit}&", limit = query.to_string()));
        let query_before = param_before.map_or_else(String::new, |query| format!("before={before}&", before = query.to_string()));
        let query_since = param_since.map_or_else(String::new, |query| format!("since={since}&", since = query.to_string()));

        let url = format!(
            "{}/v0/editgroup/reviewable?{expand}{limit}{before}{since}",
            self.base_path,
            expand = utf8_percent_encode(&query_expand, QUERY_ENCODE_SET),
            limit = utf8_percent_encode(&query_limit, QUERY_ENCODE_SET),
            before = utf8_percent_encode(&query_before, QUERY_ENCODE_SET),
            since = utf8_percent_encode(&query_since, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetEditgroupsReviewableResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<Vec<models::Editgroup>>(&buf)?;

                    Ok(GetEditgroupsReviewableResponse::Found(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetEditgroupsReviewableResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetEditgroupsReviewableResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetEditgroupsReviewableResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn update_editgroup(
        &self,
        param_editgroup_id: String,
        param_editgroup: models::Editgroup,
        param_submit: Option<bool>,
        context: &Context,
    ) -> Box<Future<Item = UpdateEditgroupResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_submit = param_submit.map_or_else(String::new, |query| format!("submit={submit}&", submit = query.to_string()));

        let url = format!(
            "{}/v0/editgroup/{editgroup_id}?{submit}",
            self.base_path,
            editgroup_id = utf8_percent_encode(&param_editgroup_id.to_string(), PATH_SEGMENT_ENCODE_SET),
            submit = utf8_percent_encode(&query_submit, QUERY_ENCODE_SET)
        );

        let body = serde_json::to_string(&param_editgroup).expect("impossible to fail to serialize");

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Put, &url);
        let mut custom_headers = hyper::header::Headers::new();

        let request = request.body(&body);

        custom_headers.set(ContentType(mimetypes::requests::UPDATE_EDITGROUP.clone()));
        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<UpdateEditgroupResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::Editgroup>(&buf)?;

                    Ok(UpdateEditgroupResponse::UpdatedEditgroup(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateEditgroupResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(UpdateEditgroupResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateEditgroupResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateEditgroupResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateEditgroupResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_editor(&self, param_editor_id: String, context: &Context) -> Box<Future<Item = GetEditorResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/editor/{editor_id}",
            self.base_path,
            editor_id = utf8_percent_encode(&param_editor_id.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetEditorResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::Editor>(&buf)?;

                    Ok(GetEditorResponse::Found(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetEditorResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetEditorResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetEditorResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_editor_annotations(
        &self,
        param_editor_id: String,
        param_limit: Option<i64>,
        param_before: Option<chrono::DateTime<chrono::Utc>>,
        param_since: Option<chrono::DateTime<chrono::Utc>>,
        context: &Context,
    ) -> Box<Future<Item = GetEditorAnnotationsResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_limit = param_limit.map_or_else(String::new, |query| format!("limit={limit}&", limit = query.to_string()));
        let query_before = param_before.map_or_else(String::new, |query| format!("before={before}&", before = query.to_string()));
        let query_since = param_since.map_or_else(String::new, |query| format!("since={since}&", since = query.to_string()));

        let url = format!(
            "{}/v0/editor/{editor_id}/annotations?{limit}{before}{since}",
            self.base_path,
            editor_id = utf8_percent_encode(&param_editor_id.to_string(), PATH_SEGMENT_ENCODE_SET),
            limit = utf8_percent_encode(&query_limit, QUERY_ENCODE_SET),
            before = utf8_percent_encode(&query_before, QUERY_ENCODE_SET),
            since = utf8_percent_encode(&query_since, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetEditorAnnotationsResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<Vec<models::EditgroupAnnotation>>(&buf)?;

                    Ok(GetEditorAnnotationsResponse::Success(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetEditorAnnotationsResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(GetEditorAnnotationsResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetEditorAnnotationsResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetEditorAnnotationsResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetEditorAnnotationsResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_editor_editgroups(
        &self,
        param_editor_id: String,
        param_limit: Option<i64>,
        param_before: Option<chrono::DateTime<chrono::Utc>>,
        param_since: Option<chrono::DateTime<chrono::Utc>>,
        context: &Context,
    ) -> Box<Future<Item = GetEditorEditgroupsResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_limit = param_limit.map_or_else(String::new, |query| format!("limit={limit}&", limit = query.to_string()));
        let query_before = param_before.map_or_else(String::new, |query| format!("before={before}&", before = query.to_string()));
        let query_since = param_since.map_or_else(String::new, |query| format!("since={since}&", since = query.to_string()));

        let url = format!(
            "{}/v0/editor/{editor_id}/editgroups?{limit}{before}{since}",
            self.base_path,
            editor_id = utf8_percent_encode(&param_editor_id.to_string(), PATH_SEGMENT_ENCODE_SET),
            limit = utf8_percent_encode(&query_limit, QUERY_ENCODE_SET),
            before = utf8_percent_encode(&query_before, QUERY_ENCODE_SET),
            since = utf8_percent_encode(&query_since, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetEditorEditgroupsResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<Vec<models::Editgroup>>(&buf)?;

                    Ok(GetEditorEditgroupsResponse::Found(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetEditorEditgroupsResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetEditorEditgroupsResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetEditorEditgroupsResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn lookup_editor(&self, param_username: Option<String>, context: &Context) -> Box<Future<Item = LookupEditorResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_username = param_username.map_or_else(String::new, |query| format!("username={username}&", username = query.to_string()));

        let url = format!("{}/v0/editor/lookup?{username}", self.base_path, username = utf8_percent_encode(&query_username, QUERY_ENCODE_SET));

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<LookupEditorResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::Editor>(&buf)?;

                    Ok(LookupEditorResponse::Found(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(LookupEditorResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(LookupEditorResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(LookupEditorResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn update_editor(&self, param_editor_id: String, param_editor: models::Editor, context: &Context) -> Box<Future<Item = UpdateEditorResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/editor/{editor_id}",
            self.base_path,
            editor_id = utf8_percent_encode(&param_editor_id.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let body = serde_json::to_string(&param_editor).expect("impossible to fail to serialize");

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Put, &url);
        let mut custom_headers = hyper::header::Headers::new();

        let request = request.body(&body);

        custom_headers.set(ContentType(mimetypes::requests::UPDATE_EDITOR.clone()));
        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<UpdateEditorResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::Editor>(&buf)?;

                    Ok(UpdateEditorResponse::UpdatedEditor(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateEditorResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(UpdateEditorResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateEditorResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateEditorResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateEditorResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn create_file(&self, param_editgroup_id: String, param_entity: models::FileEntity, context: &Context) -> Box<Future<Item = CreateFileResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/editgroup/{editgroup_id}/file",
            self.base_path,
            editgroup_id = utf8_percent_encode(&param_editgroup_id.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let body = serde_json::to_string(&param_entity).expect("impossible to fail to serialize");

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Post, &url);
        let mut custom_headers = hyper::header::Headers::new();

        let request = request.body(&body);

        custom_headers.set(ContentType(mimetypes::requests::CREATE_FILE.clone()));
        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<CreateFileResponse, ApiError> {
            match response.status.to_u16() {
                201 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::EntityEdit>(&buf)?;

                    Ok(CreateFileResponse::CreatedEntity(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateFileResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(CreateFileResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateFileResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateFileResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateFileResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn create_file_auto_batch(&self, param_auto_batch: models::FileAutoBatch, context: &Context) -> Box<Future<Item = CreateFileAutoBatchResponse, Error = ApiError> + Send> {
        let url = format!("{}/v0/editgroup/auto/file/batch", self.base_path);

        let body = serde_json::to_string(&param_auto_batch).expect("impossible to fail to serialize");

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Post, &url);
        let mut custom_headers = hyper::header::Headers::new();

        let request = request.body(&body);

        custom_headers.set(ContentType(mimetypes::requests::CREATE_FILE_AUTO_BATCH.clone()));
        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<CreateFileAutoBatchResponse, ApiError> {
            match response.status.to_u16() {
                201 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::Editgroup>(&buf)?;

                    Ok(CreateFileAutoBatchResponse::CreatedEditgroup(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateFileAutoBatchResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(CreateFileAutoBatchResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateFileAutoBatchResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateFileAutoBatchResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateFileAutoBatchResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn delete_file(&self, param_editgroup_id: String, param_ident: String, context: &Context) -> Box<Future<Item = DeleteFileResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/editgroup/{editgroup_id}/file/{ident}",
            self.base_path,
            editgroup_id = utf8_percent_encode(&param_editgroup_id.to_string(), PATH_SEGMENT_ENCODE_SET),
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Delete, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<DeleteFileResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::EntityEdit>(&buf)?;

                    Ok(DeleteFileResponse::DeletedEntity(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteFileResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(DeleteFileResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteFileResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteFileResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteFileResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn delete_file_edit(&self, param_editgroup_id: String, param_edit_id: String, context: &Context) -> Box<Future<Item = DeleteFileEditResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/editgroup/{editgroup_id}/file/edit/{edit_id}",
            self.base_path,
            editgroup_id = utf8_percent_encode(&param_editgroup_id.to_string(), PATH_SEGMENT_ENCODE_SET),
            edit_id = utf8_percent_encode(&param_edit_id.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Delete, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<DeleteFileEditResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::Success>(&buf)?;

                    Ok(DeleteFileEditResponse::DeletedEdit(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteFileEditResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(DeleteFileEditResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteFileEditResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteFileEditResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteFileEditResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_file(&self, param_ident: String, param_expand: Option<String>, param_hide: Option<String>, context: &Context) -> Box<Future<Item = GetFileResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_expand = param_expand.map_or_else(String::new, |query| format!("expand={expand}&", expand = query.to_string()));
        let query_hide = param_hide.map_or_else(String::new, |query| format!("hide={hide}&", hide = query.to_string()));

        let url = format!(
            "{}/v0/file/{ident}?{expand}{hide}",
            self.base_path,
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET),
            expand = utf8_percent_encode(&query_expand, QUERY_ENCODE_SET),
            hide = utf8_percent_encode(&query_hide, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetFileResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::FileEntity>(&buf)?;

                    Ok(GetFileResponse::FoundEntity(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetFileResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetFileResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetFileResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_file_edit(&self, param_edit_id: String, context: &Context) -> Box<Future<Item = GetFileEditResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/file/edit/{edit_id}",
            self.base_path,
            edit_id = utf8_percent_encode(&param_edit_id.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetFileEditResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::EntityEdit>(&buf)?;

                    Ok(GetFileEditResponse::FoundEdit(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetFileEditResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetFileEditResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetFileEditResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_file_history(&self, param_ident: String, param_limit: Option<i64>, context: &Context) -> Box<Future<Item = GetFileHistoryResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_limit = param_limit.map_or_else(String::new, |query| format!("limit={limit}&", limit = query.to_string()));

        let url = format!(
            "{}/v0/file/{ident}/history?{limit}",
            self.base_path,
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET),
            limit = utf8_percent_encode(&query_limit, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetFileHistoryResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<Vec<models::EntityHistoryEntry>>(&buf)?;

                    Ok(GetFileHistoryResponse::FoundEntityHistory(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetFileHistoryResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetFileHistoryResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetFileHistoryResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_file_redirects(&self, param_ident: String, context: &Context) -> Box<Future<Item = GetFileRedirectsResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/file/{ident}/redirects",
            self.base_path,
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetFileRedirectsResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<Vec<String>>(&buf)?;

                    Ok(GetFileRedirectsResponse::FoundEntityRedirects(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetFileRedirectsResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetFileRedirectsResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetFileRedirectsResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_file_revision(
        &self,
        param_rev_id: String,
        param_expand: Option<String>,
        param_hide: Option<String>,
        context: &Context,
    ) -> Box<Future<Item = GetFileRevisionResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_expand = param_expand.map_or_else(String::new, |query| format!("expand={expand}&", expand = query.to_string()));
        let query_hide = param_hide.map_or_else(String::new, |query| format!("hide={hide}&", hide = query.to_string()));

        let url = format!(
            "{}/v0/file/rev/{rev_id}?{expand}{hide}",
            self.base_path,
            rev_id = utf8_percent_encode(&param_rev_id.to_string(), PATH_SEGMENT_ENCODE_SET),
            expand = utf8_percent_encode(&query_expand, QUERY_ENCODE_SET),
            hide = utf8_percent_encode(&query_hide, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetFileRevisionResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::FileEntity>(&buf)?;

                    Ok(GetFileRevisionResponse::FoundEntityRevision(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetFileRevisionResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetFileRevisionResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetFileRevisionResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn lookup_file(
        &self,
        param_md5: Option<String>,
        param_sha1: Option<String>,
        param_sha256: Option<String>,
        param_expand: Option<String>,
        param_hide: Option<String>,
        context: &Context,
    ) -> Box<Future<Item = LookupFileResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_md5 = param_md5.map_or_else(String::new, |query| format!("md5={md5}&", md5 = query.to_string()));
        let query_sha1 = param_sha1.map_or_else(String::new, |query| format!("sha1={sha1}&", sha1 = query.to_string()));
        let query_sha256 = param_sha256.map_or_else(String::new, |query| format!("sha256={sha256}&", sha256 = query.to_string()));
        let query_expand = param_expand.map_or_else(String::new, |query| format!("expand={expand}&", expand = query.to_string()));
        let query_hide = param_hide.map_or_else(String::new, |query| format!("hide={hide}&", hide = query.to_string()));

        let url = format!(
            "{}/v0/file/lookup?{md5}{sha1}{sha256}{expand}{hide}",
            self.base_path,
            md5 = utf8_percent_encode(&query_md5, QUERY_ENCODE_SET),
            sha1 = utf8_percent_encode(&query_sha1, QUERY_ENCODE_SET),
            sha256 = utf8_percent_encode(&query_sha256, QUERY_ENCODE_SET),
            expand = utf8_percent_encode(&query_expand, QUERY_ENCODE_SET),
            hide = utf8_percent_encode(&query_hide, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<LookupFileResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::FileEntity>(&buf)?;

                    Ok(LookupFileResponse::FoundEntity(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(LookupFileResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(LookupFileResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(LookupFileResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn update_file(&self, param_editgroup_id: String, param_ident: String, param_entity: models::FileEntity, context: &Context) -> Box<Future<Item = UpdateFileResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/editgroup/{editgroup_id}/file/{ident}",
            self.base_path,
            editgroup_id = utf8_percent_encode(&param_editgroup_id.to_string(), PATH_SEGMENT_ENCODE_SET),
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let body = serde_json::to_string(&param_entity).expect("impossible to fail to serialize");

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Put, &url);
        let mut custom_headers = hyper::header::Headers::new();

        let request = request.body(&body);

        custom_headers.set(ContentType(mimetypes::requests::UPDATE_FILE.clone()));
        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<UpdateFileResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::EntityEdit>(&buf)?;

                    Ok(UpdateFileResponse::UpdatedEntity(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateFileResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(UpdateFileResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateFileResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateFileResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateFileResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn create_fileset(&self, param_editgroup_id: String, param_entity: models::FilesetEntity, context: &Context) -> Box<Future<Item = CreateFilesetResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/editgroup/{editgroup_id}/fileset",
            self.base_path,
            editgroup_id = utf8_percent_encode(&param_editgroup_id.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let body = serde_json::to_string(&param_entity).expect("impossible to fail to serialize");

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Post, &url);
        let mut custom_headers = hyper::header::Headers::new();

        let request = request.body(&body);

        custom_headers.set(ContentType(mimetypes::requests::CREATE_FILESET.clone()));
        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<CreateFilesetResponse, ApiError> {
            match response.status.to_u16() {
                201 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::EntityEdit>(&buf)?;

                    Ok(CreateFilesetResponse::CreatedEntity(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateFilesetResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(CreateFilesetResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateFilesetResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateFilesetResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateFilesetResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn create_fileset_auto_batch(&self, param_auto_batch: models::FilesetAutoBatch, context: &Context) -> Box<Future<Item = CreateFilesetAutoBatchResponse, Error = ApiError> + Send> {
        let url = format!("{}/v0/editgroup/auto/fileset/batch", self.base_path);

        let body = serde_json::to_string(&param_auto_batch).expect("impossible to fail to serialize");

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Post, &url);
        let mut custom_headers = hyper::header::Headers::new();

        let request = request.body(&body);

        custom_headers.set(ContentType(mimetypes::requests::CREATE_FILESET_AUTO_BATCH.clone()));
        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<CreateFilesetAutoBatchResponse, ApiError> {
            match response.status.to_u16() {
                201 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::Editgroup>(&buf)?;

                    Ok(CreateFilesetAutoBatchResponse::CreatedEditgroup(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateFilesetAutoBatchResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(CreateFilesetAutoBatchResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateFilesetAutoBatchResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateFilesetAutoBatchResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateFilesetAutoBatchResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn delete_fileset(&self, param_editgroup_id: String, param_ident: String, context: &Context) -> Box<Future<Item = DeleteFilesetResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/editgroup/{editgroup_id}/fileset/{ident}",
            self.base_path,
            editgroup_id = utf8_percent_encode(&param_editgroup_id.to_string(), PATH_SEGMENT_ENCODE_SET),
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Delete, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<DeleteFilesetResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::EntityEdit>(&buf)?;

                    Ok(DeleteFilesetResponse::DeletedEntity(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteFilesetResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(DeleteFilesetResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteFilesetResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteFilesetResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteFilesetResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn delete_fileset_edit(&self, param_editgroup_id: String, param_edit_id: String, context: &Context) -> Box<Future<Item = DeleteFilesetEditResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/editgroup/{editgroup_id}/fileset/edit/{edit_id}",
            self.base_path,
            editgroup_id = utf8_percent_encode(&param_editgroup_id.to_string(), PATH_SEGMENT_ENCODE_SET),
            edit_id = utf8_percent_encode(&param_edit_id.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Delete, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<DeleteFilesetEditResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::Success>(&buf)?;

                    Ok(DeleteFilesetEditResponse::DeletedEdit(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteFilesetEditResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(DeleteFilesetEditResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteFilesetEditResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteFilesetEditResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteFilesetEditResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_fileset(&self, param_ident: String, param_expand: Option<String>, param_hide: Option<String>, context: &Context) -> Box<Future<Item = GetFilesetResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_expand = param_expand.map_or_else(String::new, |query| format!("expand={expand}&", expand = query.to_string()));
        let query_hide = param_hide.map_or_else(String::new, |query| format!("hide={hide}&", hide = query.to_string()));

        let url = format!(
            "{}/v0/fileset/{ident}?{expand}{hide}",
            self.base_path,
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET),
            expand = utf8_percent_encode(&query_expand, QUERY_ENCODE_SET),
            hide = utf8_percent_encode(&query_hide, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetFilesetResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::FilesetEntity>(&buf)?;

                    Ok(GetFilesetResponse::FoundEntity(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetFilesetResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetFilesetResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetFilesetResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_fileset_edit(&self, param_edit_id: String, context: &Context) -> Box<Future<Item = GetFilesetEditResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/fileset/edit/{edit_id}",
            self.base_path,
            edit_id = utf8_percent_encode(&param_edit_id.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetFilesetEditResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::EntityEdit>(&buf)?;

                    Ok(GetFilesetEditResponse::FoundEdit(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetFilesetEditResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetFilesetEditResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetFilesetEditResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_fileset_history(&self, param_ident: String, param_limit: Option<i64>, context: &Context) -> Box<Future<Item = GetFilesetHistoryResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_limit = param_limit.map_or_else(String::new, |query| format!("limit={limit}&", limit = query.to_string()));

        let url = format!(
            "{}/v0/fileset/{ident}/history?{limit}",
            self.base_path,
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET),
            limit = utf8_percent_encode(&query_limit, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetFilesetHistoryResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<Vec<models::EntityHistoryEntry>>(&buf)?;

                    Ok(GetFilesetHistoryResponse::FoundEntityHistory(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetFilesetHistoryResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetFilesetHistoryResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetFilesetHistoryResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_fileset_redirects(&self, param_ident: String, context: &Context) -> Box<Future<Item = GetFilesetRedirectsResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/fileset/{ident}/redirects",
            self.base_path,
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetFilesetRedirectsResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<Vec<String>>(&buf)?;

                    Ok(GetFilesetRedirectsResponse::FoundEntityRedirects(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetFilesetRedirectsResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetFilesetRedirectsResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetFilesetRedirectsResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_fileset_revision(
        &self,
        param_rev_id: String,
        param_expand: Option<String>,
        param_hide: Option<String>,
        context: &Context,
    ) -> Box<Future<Item = GetFilesetRevisionResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_expand = param_expand.map_or_else(String::new, |query| format!("expand={expand}&", expand = query.to_string()));
        let query_hide = param_hide.map_or_else(String::new, |query| format!("hide={hide}&", hide = query.to_string()));

        let url = format!(
            "{}/v0/fileset/rev/{rev_id}?{expand}{hide}",
            self.base_path,
            rev_id = utf8_percent_encode(&param_rev_id.to_string(), PATH_SEGMENT_ENCODE_SET),
            expand = utf8_percent_encode(&query_expand, QUERY_ENCODE_SET),
            hide = utf8_percent_encode(&query_hide, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetFilesetRevisionResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::FilesetEntity>(&buf)?;

                    Ok(GetFilesetRevisionResponse::FoundEntityRevision(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetFilesetRevisionResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetFilesetRevisionResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetFilesetRevisionResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn update_fileset(
        &self,
        param_editgroup_id: String,
        param_ident: String,
        param_entity: models::FilesetEntity,
        context: &Context,
    ) -> Box<Future<Item = UpdateFilesetResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/editgroup/{editgroup_id}/fileset/{ident}",
            self.base_path,
            editgroup_id = utf8_percent_encode(&param_editgroup_id.to_string(), PATH_SEGMENT_ENCODE_SET),
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let body = serde_json::to_string(&param_entity).expect("impossible to fail to serialize");

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Put, &url);
        let mut custom_headers = hyper::header::Headers::new();

        let request = request.body(&body);

        custom_headers.set(ContentType(mimetypes::requests::UPDATE_FILESET.clone()));
        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<UpdateFilesetResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::EntityEdit>(&buf)?;

                    Ok(UpdateFilesetResponse::UpdatedEntity(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateFilesetResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(UpdateFilesetResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateFilesetResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateFilesetResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateFilesetResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn create_release(&self, param_editgroup_id: String, param_entity: models::ReleaseEntity, context: &Context) -> Box<Future<Item = CreateReleaseResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/editgroup/{editgroup_id}/release",
            self.base_path,
            editgroup_id = utf8_percent_encode(&param_editgroup_id.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let body = serde_json::to_string(&param_entity).expect("impossible to fail to serialize");

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Post, &url);
        let mut custom_headers = hyper::header::Headers::new();

        let request = request.body(&body);

        custom_headers.set(ContentType(mimetypes::requests::CREATE_RELEASE.clone()));
        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<CreateReleaseResponse, ApiError> {
            match response.status.to_u16() {
                201 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::EntityEdit>(&buf)?;

                    Ok(CreateReleaseResponse::CreatedEntity(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateReleaseResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(CreateReleaseResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateReleaseResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateReleaseResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateReleaseResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn create_release_auto_batch(&self, param_auto_batch: models::ReleaseAutoBatch, context: &Context) -> Box<Future<Item = CreateReleaseAutoBatchResponse, Error = ApiError> + Send> {
        let url = format!("{}/v0/editgroup/auto/release/batch", self.base_path);

        let body = serde_json::to_string(&param_auto_batch).expect("impossible to fail to serialize");

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Post, &url);
        let mut custom_headers = hyper::header::Headers::new();

        let request = request.body(&body);

        custom_headers.set(ContentType(mimetypes::requests::CREATE_RELEASE_AUTO_BATCH.clone()));
        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<CreateReleaseAutoBatchResponse, ApiError> {
            match response.status.to_u16() {
                201 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::Editgroup>(&buf)?;

                    Ok(CreateReleaseAutoBatchResponse::CreatedEditgroup(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateReleaseAutoBatchResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(CreateReleaseAutoBatchResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateReleaseAutoBatchResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateReleaseAutoBatchResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateReleaseAutoBatchResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn delete_release(&self, param_editgroup_id: String, param_ident: String, context: &Context) -> Box<Future<Item = DeleteReleaseResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/editgroup/{editgroup_id}/release/{ident}",
            self.base_path,
            editgroup_id = utf8_percent_encode(&param_editgroup_id.to_string(), PATH_SEGMENT_ENCODE_SET),
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Delete, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<DeleteReleaseResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::EntityEdit>(&buf)?;

                    Ok(DeleteReleaseResponse::DeletedEntity(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteReleaseResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(DeleteReleaseResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteReleaseResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteReleaseResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteReleaseResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn delete_release_edit(&self, param_editgroup_id: String, param_edit_id: String, context: &Context) -> Box<Future<Item = DeleteReleaseEditResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/editgroup/{editgroup_id}/release/edit/{edit_id}",
            self.base_path,
            editgroup_id = utf8_percent_encode(&param_editgroup_id.to_string(), PATH_SEGMENT_ENCODE_SET),
            edit_id = utf8_percent_encode(&param_edit_id.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Delete, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<DeleteReleaseEditResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::Success>(&buf)?;

                    Ok(DeleteReleaseEditResponse::DeletedEdit(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteReleaseEditResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(DeleteReleaseEditResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteReleaseEditResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteReleaseEditResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteReleaseEditResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_release(&self, param_ident: String, param_expand: Option<String>, param_hide: Option<String>, context: &Context) -> Box<Future<Item = GetReleaseResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_expand = param_expand.map_or_else(String::new, |query| format!("expand={expand}&", expand = query.to_string()));
        let query_hide = param_hide.map_or_else(String::new, |query| format!("hide={hide}&", hide = query.to_string()));

        let url = format!(
            "{}/v0/release/{ident}?{expand}{hide}",
            self.base_path,
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET),
            expand = utf8_percent_encode(&query_expand, QUERY_ENCODE_SET),
            hide = utf8_percent_encode(&query_hide, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetReleaseResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ReleaseEntity>(&buf)?;

                    Ok(GetReleaseResponse::FoundEntity(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetReleaseResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetReleaseResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetReleaseResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_release_edit(&self, param_edit_id: String, context: &Context) -> Box<Future<Item = GetReleaseEditResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/release/edit/{edit_id}",
            self.base_path,
            edit_id = utf8_percent_encode(&param_edit_id.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetReleaseEditResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::EntityEdit>(&buf)?;

                    Ok(GetReleaseEditResponse::FoundEdit(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetReleaseEditResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetReleaseEditResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetReleaseEditResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_release_files(&self, param_ident: String, param_hide: Option<String>, context: &Context) -> Box<Future<Item = GetReleaseFilesResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_hide = param_hide.map_or_else(String::new, |query| format!("hide={hide}&", hide = query.to_string()));

        let url = format!(
            "{}/v0/release/{ident}/files?{hide}",
            self.base_path,
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET),
            hide = utf8_percent_encode(&query_hide, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetReleaseFilesResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<Vec<models::FileEntity>>(&buf)?;

                    Ok(GetReleaseFilesResponse::Found(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetReleaseFilesResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetReleaseFilesResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetReleaseFilesResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_release_filesets(&self, param_ident: String, param_hide: Option<String>, context: &Context) -> Box<Future<Item = GetReleaseFilesetsResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_hide = param_hide.map_or_else(String::new, |query| format!("hide={hide}&", hide = query.to_string()));

        let url = format!(
            "{}/v0/release/{ident}/filesets?{hide}",
            self.base_path,
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET),
            hide = utf8_percent_encode(&query_hide, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetReleaseFilesetsResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<Vec<models::FilesetEntity>>(&buf)?;

                    Ok(GetReleaseFilesetsResponse::Found(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetReleaseFilesetsResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetReleaseFilesetsResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetReleaseFilesetsResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_release_history(&self, param_ident: String, param_limit: Option<i64>, context: &Context) -> Box<Future<Item = GetReleaseHistoryResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_limit = param_limit.map_or_else(String::new, |query| format!("limit={limit}&", limit = query.to_string()));

        let url = format!(
            "{}/v0/release/{ident}/history?{limit}",
            self.base_path,
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET),
            limit = utf8_percent_encode(&query_limit, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetReleaseHistoryResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<Vec<models::EntityHistoryEntry>>(&buf)?;

                    Ok(GetReleaseHistoryResponse::FoundEntityHistory(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetReleaseHistoryResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetReleaseHistoryResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetReleaseHistoryResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_release_redirects(&self, param_ident: String, context: &Context) -> Box<Future<Item = GetReleaseRedirectsResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/release/{ident}/redirects",
            self.base_path,
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetReleaseRedirectsResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<Vec<String>>(&buf)?;

                    Ok(GetReleaseRedirectsResponse::FoundEntityRedirects(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetReleaseRedirectsResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetReleaseRedirectsResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetReleaseRedirectsResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_release_revision(
        &self,
        param_rev_id: String,
        param_expand: Option<String>,
        param_hide: Option<String>,
        context: &Context,
    ) -> Box<Future<Item = GetReleaseRevisionResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_expand = param_expand.map_or_else(String::new, |query| format!("expand={expand}&", expand = query.to_string()));
        let query_hide = param_hide.map_or_else(String::new, |query| format!("hide={hide}&", hide = query.to_string()));

        let url = format!(
            "{}/v0/release/rev/{rev_id}?{expand}{hide}",
            self.base_path,
            rev_id = utf8_percent_encode(&param_rev_id.to_string(), PATH_SEGMENT_ENCODE_SET),
            expand = utf8_percent_encode(&query_expand, QUERY_ENCODE_SET),
            hide = utf8_percent_encode(&query_hide, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetReleaseRevisionResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ReleaseEntity>(&buf)?;

                    Ok(GetReleaseRevisionResponse::FoundEntityRevision(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetReleaseRevisionResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetReleaseRevisionResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetReleaseRevisionResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_release_webcaptures(&self, param_ident: String, param_hide: Option<String>, context: &Context) -> Box<Future<Item = GetReleaseWebcapturesResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_hide = param_hide.map_or_else(String::new, |query| format!("hide={hide}&", hide = query.to_string()));

        let url = format!(
            "{}/v0/release/{ident}/webcaptures?{hide}",
            self.base_path,
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET),
            hide = utf8_percent_encode(&query_hide, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetReleaseWebcapturesResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<Vec<models::WebcaptureEntity>>(&buf)?;

                    Ok(GetReleaseWebcapturesResponse::Found(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetReleaseWebcapturesResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetReleaseWebcapturesResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetReleaseWebcapturesResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn lookup_release(
        &self,
        param_doi: Option<String>,
        param_wikidata_qid: Option<String>,
        param_isbn13: Option<String>,
        param_pmid: Option<String>,
        param_pmcid: Option<String>,
        param_core: Option<String>,
        param_arxiv: Option<String>,
        param_jstor: Option<String>,
        param_ark: Option<String>,
        param_mag: Option<String>,
        param_doaj: Option<String>,
        param_dblp: Option<String>,
        param_oai: Option<String>,
        param_hdl: Option<String>,
        param_expand: Option<String>,
        param_hide: Option<String>,
        context: &Context,
    ) -> Box<Future<Item = LookupReleaseResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_doi = param_doi.map_or_else(String::new, |query| format!("doi={doi}&", doi = query.to_string()));
        let query_wikidata_qid = param_wikidata_qid.map_or_else(String::new, |query| format!("wikidata_qid={wikidata_qid}&", wikidata_qid = query.to_string()));
        let query_isbn13 = param_isbn13.map_or_else(String::new, |query| format!("isbn13={isbn13}&", isbn13 = query.to_string()));
        let query_pmid = param_pmid.map_or_else(String::new, |query| format!("pmid={pmid}&", pmid = query.to_string()));
        let query_pmcid = param_pmcid.map_or_else(String::new, |query| format!("pmcid={pmcid}&", pmcid = query.to_string()));
        let query_core = param_core.map_or_else(String::new, |query| format!("core={core}&", core = query.to_string()));
        let query_arxiv = param_arxiv.map_or_else(String::new, |query| format!("arxiv={arxiv}&", arxiv = query.to_string()));
        let query_jstor = param_jstor.map_or_else(String::new, |query| format!("jstor={jstor}&", jstor = query.to_string()));
        let query_ark = param_ark.map_or_else(String::new, |query| format!("ark={ark}&", ark = query.to_string()));
        let query_mag = param_mag.map_or_else(String::new, |query| format!("mag={mag}&", mag = query.to_string()));
        let query_doaj = param_doaj.map_or_else(String::new, |query| format!("doaj={doaj}&", doaj = query.to_string()));
        let query_dblp = param_dblp.map_or_else(String::new, |query| format!("dblp={dblp}&", dblp = query.to_string()));
        let query_oai = param_oai.map_or_else(String::new, |query| format!("oai={oai}&", oai = query.to_string()));
        let query_hdl = param_hdl.map_or_else(String::new, |query| format!("hdl={hdl}&", hdl = query.to_string()));
        let query_expand = param_expand.map_or_else(String::new, |query| format!("expand={expand}&", expand = query.to_string()));
        let query_hide = param_hide.map_or_else(String::new, |query| format!("hide={hide}&", hide = query.to_string()));

        let url = format!(
            "{}/v0/release/lookup?{doi}{wikidata_qid}{isbn13}{pmid}{pmcid}{core}{arxiv}{jstor}{ark}{mag}{doaj}{dblp}{oai}{hdl}{expand}{hide}",
            self.base_path,
            doi = utf8_percent_encode(&query_doi, QUERY_ENCODE_SET),
            wikidata_qid = utf8_percent_encode(&query_wikidata_qid, QUERY_ENCODE_SET),
            isbn13 = utf8_percent_encode(&query_isbn13, QUERY_ENCODE_SET),
            pmid = utf8_percent_encode(&query_pmid, QUERY_ENCODE_SET),
            pmcid = utf8_percent_encode(&query_pmcid, QUERY_ENCODE_SET),
            core = utf8_percent_encode(&query_core, QUERY_ENCODE_SET),
            arxiv = utf8_percent_encode(&query_arxiv, QUERY_ENCODE_SET),
            jstor = utf8_percent_encode(&query_jstor, QUERY_ENCODE_SET),
            ark = utf8_percent_encode(&query_ark, QUERY_ENCODE_SET),
            mag = utf8_percent_encode(&query_mag, QUERY_ENCODE_SET),
            doaj = utf8_percent_encode(&query_doaj, QUERY_ENCODE_SET),
            dblp = utf8_percent_encode(&query_dblp, QUERY_ENCODE_SET),
            oai = utf8_percent_encode(&query_oai, QUERY_ENCODE_SET),
            hdl = utf8_percent_encode(&query_hdl, QUERY_ENCODE_SET),
            expand = utf8_percent_encode(&query_expand, QUERY_ENCODE_SET),
            hide = utf8_percent_encode(&query_hide, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<LookupReleaseResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ReleaseEntity>(&buf)?;

                    Ok(LookupReleaseResponse::FoundEntity(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(LookupReleaseResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(LookupReleaseResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(LookupReleaseResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn update_release(
        &self,
        param_editgroup_id: String,
        param_ident: String,
        param_entity: models::ReleaseEntity,
        context: &Context,
    ) -> Box<Future<Item = UpdateReleaseResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/editgroup/{editgroup_id}/release/{ident}",
            self.base_path,
            editgroup_id = utf8_percent_encode(&param_editgroup_id.to_string(), PATH_SEGMENT_ENCODE_SET),
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let body = serde_json::to_string(&param_entity).expect("impossible to fail to serialize");

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Put, &url);
        let mut custom_headers = hyper::header::Headers::new();

        let request = request.body(&body);

        custom_headers.set(ContentType(mimetypes::requests::UPDATE_RELEASE.clone()));
        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<UpdateReleaseResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::EntityEdit>(&buf)?;

                    Ok(UpdateReleaseResponse::UpdatedEntity(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateReleaseResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(UpdateReleaseResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateReleaseResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateReleaseResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateReleaseResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn create_webcapture(&self, param_editgroup_id: String, param_entity: models::WebcaptureEntity, context: &Context) -> Box<Future<Item = CreateWebcaptureResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/editgroup/{editgroup_id}/webcapture",
            self.base_path,
            editgroup_id = utf8_percent_encode(&param_editgroup_id.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let body = serde_json::to_string(&param_entity).expect("impossible to fail to serialize");

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Post, &url);
        let mut custom_headers = hyper::header::Headers::new();

        let request = request.body(&body);

        custom_headers.set(ContentType(mimetypes::requests::CREATE_WEBCAPTURE.clone()));
        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<CreateWebcaptureResponse, ApiError> {
            match response.status.to_u16() {
                201 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::EntityEdit>(&buf)?;

                    Ok(CreateWebcaptureResponse::CreatedEntity(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateWebcaptureResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(CreateWebcaptureResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateWebcaptureResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateWebcaptureResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateWebcaptureResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn create_webcapture_auto_batch(&self, param_auto_batch: models::WebcaptureAutoBatch, context: &Context) -> Box<Future<Item = CreateWebcaptureAutoBatchResponse, Error = ApiError> + Send> {
        let url = format!("{}/v0/editgroup/auto/webcapture/batch", self.base_path);

        let body = serde_json::to_string(&param_auto_batch).expect("impossible to fail to serialize");

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Post, &url);
        let mut custom_headers = hyper::header::Headers::new();

        let request = request.body(&body);

        custom_headers.set(ContentType(mimetypes::requests::CREATE_WEBCAPTURE_AUTO_BATCH.clone()));
        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<CreateWebcaptureAutoBatchResponse, ApiError> {
            match response.status.to_u16() {
                201 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::Editgroup>(&buf)?;

                    Ok(CreateWebcaptureAutoBatchResponse::CreatedEditgroup(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateWebcaptureAutoBatchResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(CreateWebcaptureAutoBatchResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateWebcaptureAutoBatchResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateWebcaptureAutoBatchResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateWebcaptureAutoBatchResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn delete_webcapture(&self, param_editgroup_id: String, param_ident: String, context: &Context) -> Box<Future<Item = DeleteWebcaptureResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/editgroup/{editgroup_id}/webcapture/{ident}",
            self.base_path,
            editgroup_id = utf8_percent_encode(&param_editgroup_id.to_string(), PATH_SEGMENT_ENCODE_SET),
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Delete, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<DeleteWebcaptureResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::EntityEdit>(&buf)?;

                    Ok(DeleteWebcaptureResponse::DeletedEntity(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteWebcaptureResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(DeleteWebcaptureResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteWebcaptureResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteWebcaptureResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteWebcaptureResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn delete_webcapture_edit(&self, param_editgroup_id: String, param_edit_id: String, context: &Context) -> Box<Future<Item = DeleteWebcaptureEditResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/editgroup/{editgroup_id}/webcapture/edit/{edit_id}",
            self.base_path,
            editgroup_id = utf8_percent_encode(&param_editgroup_id.to_string(), PATH_SEGMENT_ENCODE_SET),
            edit_id = utf8_percent_encode(&param_edit_id.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Delete, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<DeleteWebcaptureEditResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::Success>(&buf)?;

                    Ok(DeleteWebcaptureEditResponse::DeletedEdit(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteWebcaptureEditResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(DeleteWebcaptureEditResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteWebcaptureEditResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteWebcaptureEditResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteWebcaptureEditResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_webcapture(&self, param_ident: String, param_expand: Option<String>, param_hide: Option<String>, context: &Context) -> Box<Future<Item = GetWebcaptureResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_expand = param_expand.map_or_else(String::new, |query| format!("expand={expand}&", expand = query.to_string()));
        let query_hide = param_hide.map_or_else(String::new, |query| format!("hide={hide}&", hide = query.to_string()));

        let url = format!(
            "{}/v0/webcapture/{ident}?{expand}{hide}",
            self.base_path,
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET),
            expand = utf8_percent_encode(&query_expand, QUERY_ENCODE_SET),
            hide = utf8_percent_encode(&query_hide, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetWebcaptureResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::WebcaptureEntity>(&buf)?;

                    Ok(GetWebcaptureResponse::FoundEntity(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetWebcaptureResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetWebcaptureResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetWebcaptureResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_webcapture_edit(&self, param_edit_id: String, context: &Context) -> Box<Future<Item = GetWebcaptureEditResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/webcapture/edit/{edit_id}",
            self.base_path,
            edit_id = utf8_percent_encode(&param_edit_id.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetWebcaptureEditResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::EntityEdit>(&buf)?;

                    Ok(GetWebcaptureEditResponse::FoundEdit(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetWebcaptureEditResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetWebcaptureEditResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetWebcaptureEditResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_webcapture_history(&self, param_ident: String, param_limit: Option<i64>, context: &Context) -> Box<Future<Item = GetWebcaptureHistoryResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_limit = param_limit.map_or_else(String::new, |query| format!("limit={limit}&", limit = query.to_string()));

        let url = format!(
            "{}/v0/webcapture/{ident}/history?{limit}",
            self.base_path,
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET),
            limit = utf8_percent_encode(&query_limit, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetWebcaptureHistoryResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<Vec<models::EntityHistoryEntry>>(&buf)?;

                    Ok(GetWebcaptureHistoryResponse::FoundEntityHistory(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetWebcaptureHistoryResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetWebcaptureHistoryResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetWebcaptureHistoryResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_webcapture_redirects(&self, param_ident: String, context: &Context) -> Box<Future<Item = GetWebcaptureRedirectsResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/webcapture/{ident}/redirects",
            self.base_path,
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetWebcaptureRedirectsResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<Vec<String>>(&buf)?;

                    Ok(GetWebcaptureRedirectsResponse::FoundEntityRedirects(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetWebcaptureRedirectsResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetWebcaptureRedirectsResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetWebcaptureRedirectsResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_webcapture_revision(
        &self,
        param_rev_id: String,
        param_expand: Option<String>,
        param_hide: Option<String>,
        context: &Context,
    ) -> Box<Future<Item = GetWebcaptureRevisionResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_expand = param_expand.map_or_else(String::new, |query| format!("expand={expand}&", expand = query.to_string()));
        let query_hide = param_hide.map_or_else(String::new, |query| format!("hide={hide}&", hide = query.to_string()));

        let url = format!(
            "{}/v0/webcapture/rev/{rev_id}?{expand}{hide}",
            self.base_path,
            rev_id = utf8_percent_encode(&param_rev_id.to_string(), PATH_SEGMENT_ENCODE_SET),
            expand = utf8_percent_encode(&query_expand, QUERY_ENCODE_SET),
            hide = utf8_percent_encode(&query_hide, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetWebcaptureRevisionResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::WebcaptureEntity>(&buf)?;

                    Ok(GetWebcaptureRevisionResponse::FoundEntityRevision(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetWebcaptureRevisionResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetWebcaptureRevisionResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetWebcaptureRevisionResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn update_webcapture(
        &self,
        param_editgroup_id: String,
        param_ident: String,
        param_entity: models::WebcaptureEntity,
        context: &Context,
    ) -> Box<Future<Item = UpdateWebcaptureResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/editgroup/{editgroup_id}/webcapture/{ident}",
            self.base_path,
            editgroup_id = utf8_percent_encode(&param_editgroup_id.to_string(), PATH_SEGMENT_ENCODE_SET),
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let body = serde_json::to_string(&param_entity).expect("impossible to fail to serialize");

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Put, &url);
        let mut custom_headers = hyper::header::Headers::new();

        let request = request.body(&body);

        custom_headers.set(ContentType(mimetypes::requests::UPDATE_WEBCAPTURE.clone()));
        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<UpdateWebcaptureResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::EntityEdit>(&buf)?;

                    Ok(UpdateWebcaptureResponse::UpdatedEntity(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateWebcaptureResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(UpdateWebcaptureResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateWebcaptureResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateWebcaptureResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateWebcaptureResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn create_work(&self, param_editgroup_id: String, param_entity: models::WorkEntity, context: &Context) -> Box<Future<Item = CreateWorkResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/editgroup/{editgroup_id}/work",
            self.base_path,
            editgroup_id = utf8_percent_encode(&param_editgroup_id.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let body = serde_json::to_string(&param_entity).expect("impossible to fail to serialize");

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Post, &url);
        let mut custom_headers = hyper::header::Headers::new();

        let request = request.body(&body);

        custom_headers.set(ContentType(mimetypes::requests::CREATE_WORK.clone()));
        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<CreateWorkResponse, ApiError> {
            match response.status.to_u16() {
                201 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::EntityEdit>(&buf)?;

                    Ok(CreateWorkResponse::CreatedEntity(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateWorkResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(CreateWorkResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateWorkResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateWorkResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateWorkResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn create_work_auto_batch(&self, param_auto_batch: models::WorkAutoBatch, context: &Context) -> Box<Future<Item = CreateWorkAutoBatchResponse, Error = ApiError> + Send> {
        let url = format!("{}/v0/editgroup/auto/work/batch", self.base_path);

        let body = serde_json::to_string(&param_auto_batch).expect("impossible to fail to serialize");

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Post, &url);
        let mut custom_headers = hyper::header::Headers::new();

        let request = request.body(&body);

        custom_headers.set(ContentType(mimetypes::requests::CREATE_WORK_AUTO_BATCH.clone()));
        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<CreateWorkAutoBatchResponse, ApiError> {
            match response.status.to_u16() {
                201 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::Editgroup>(&buf)?;

                    Ok(CreateWorkAutoBatchResponse::CreatedEditgroup(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateWorkAutoBatchResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(CreateWorkAutoBatchResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateWorkAutoBatchResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateWorkAutoBatchResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(CreateWorkAutoBatchResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn delete_work(&self, param_editgroup_id: String, param_ident: String, context: &Context) -> Box<Future<Item = DeleteWorkResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/editgroup/{editgroup_id}/work/{ident}",
            self.base_path,
            editgroup_id = utf8_percent_encode(&param_editgroup_id.to_string(), PATH_SEGMENT_ENCODE_SET),
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Delete, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<DeleteWorkResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::EntityEdit>(&buf)?;

                    Ok(DeleteWorkResponse::DeletedEntity(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteWorkResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(DeleteWorkResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteWorkResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteWorkResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteWorkResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn delete_work_edit(&self, param_editgroup_id: String, param_edit_id: String, context: &Context) -> Box<Future<Item = DeleteWorkEditResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/editgroup/{editgroup_id}/work/edit/{edit_id}",
            self.base_path,
            editgroup_id = utf8_percent_encode(&param_editgroup_id.to_string(), PATH_SEGMENT_ENCODE_SET),
            edit_id = utf8_percent_encode(&param_edit_id.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Delete, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<DeleteWorkEditResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::Success>(&buf)?;

                    Ok(DeleteWorkEditResponse::DeletedEdit(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteWorkEditResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(DeleteWorkEditResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteWorkEditResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteWorkEditResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(DeleteWorkEditResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_work(&self, param_ident: String, param_expand: Option<String>, param_hide: Option<String>, context: &Context) -> Box<Future<Item = GetWorkResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_expand = param_expand.map_or_else(String::new, |query| format!("expand={expand}&", expand = query.to_string()));
        let query_hide = param_hide.map_or_else(String::new, |query| format!("hide={hide}&", hide = query.to_string()));

        let url = format!(
            "{}/v0/work/{ident}?{expand}{hide}",
            self.base_path,
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET),
            expand = utf8_percent_encode(&query_expand, QUERY_ENCODE_SET),
            hide = utf8_percent_encode(&query_hide, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetWorkResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::WorkEntity>(&buf)?;

                    Ok(GetWorkResponse::FoundEntity(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetWorkResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetWorkResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetWorkResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_work_edit(&self, param_edit_id: String, context: &Context) -> Box<Future<Item = GetWorkEditResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/work/edit/{edit_id}",
            self.base_path,
            edit_id = utf8_percent_encode(&param_edit_id.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetWorkEditResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::EntityEdit>(&buf)?;

                    Ok(GetWorkEditResponse::FoundEdit(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetWorkEditResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetWorkEditResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetWorkEditResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_work_history(&self, param_ident: String, param_limit: Option<i64>, context: &Context) -> Box<Future<Item = GetWorkHistoryResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_limit = param_limit.map_or_else(String::new, |query| format!("limit={limit}&", limit = query.to_string()));

        let url = format!(
            "{}/v0/work/{ident}/history?{limit}",
            self.base_path,
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET),
            limit = utf8_percent_encode(&query_limit, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetWorkHistoryResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<Vec<models::EntityHistoryEntry>>(&buf)?;

                    Ok(GetWorkHistoryResponse::FoundEntityHistory(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetWorkHistoryResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetWorkHistoryResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetWorkHistoryResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_work_redirects(&self, param_ident: String, context: &Context) -> Box<Future<Item = GetWorkRedirectsResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/work/{ident}/redirects",
            self.base_path,
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetWorkRedirectsResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<Vec<String>>(&buf)?;

                    Ok(GetWorkRedirectsResponse::FoundEntityRedirects(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetWorkRedirectsResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetWorkRedirectsResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetWorkRedirectsResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_work_releases(&self, param_ident: String, param_hide: Option<String>, context: &Context) -> Box<Future<Item = GetWorkReleasesResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_hide = param_hide.map_or_else(String::new, |query| format!("hide={hide}&", hide = query.to_string()));

        let url = format!(
            "{}/v0/work/{ident}/releases?{hide}",
            self.base_path,
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET),
            hide = utf8_percent_encode(&query_hide, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetWorkReleasesResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<Vec<models::ReleaseEntity>>(&buf)?;

                    Ok(GetWorkReleasesResponse::Found(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetWorkReleasesResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetWorkReleasesResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetWorkReleasesResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn get_work_revision(
        &self,
        param_rev_id: String,
        param_expand: Option<String>,
        param_hide: Option<String>,
        context: &Context,
    ) -> Box<Future<Item = GetWorkRevisionResponse, Error = ApiError> + Send> {
        // Query parameters
        let query_expand = param_expand.map_or_else(String::new, |query| format!("expand={expand}&", expand = query.to_string()));
        let query_hide = param_hide.map_or_else(String::new, |query| format!("hide={hide}&", hide = query.to_string()));

        let url = format!(
            "{}/v0/work/rev/{rev_id}?{expand}{hide}",
            self.base_path,
            rev_id = utf8_percent_encode(&param_rev_id.to_string(), PATH_SEGMENT_ENCODE_SET),
            expand = utf8_percent_encode(&query_expand, QUERY_ENCODE_SET),
            hide = utf8_percent_encode(&query_hide, QUERY_ENCODE_SET)
        );

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Get, &url);
        let mut custom_headers = hyper::header::Headers::new();

        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<GetWorkRevisionResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::WorkEntity>(&buf)?;

                    Ok(GetWorkRevisionResponse::FoundEntityRevision(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetWorkRevisionResponse::BadRequest(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetWorkRevisionResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(GetWorkRevisionResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }

    fn update_work(&self, param_editgroup_id: String, param_ident: String, param_entity: models::WorkEntity, context: &Context) -> Box<Future<Item = UpdateWorkResponse, Error = ApiError> + Send> {
        let url = format!(
            "{}/v0/editgroup/{editgroup_id}/work/{ident}",
            self.base_path,
            editgroup_id = utf8_percent_encode(&param_editgroup_id.to_string(), PATH_SEGMENT_ENCODE_SET),
            ident = utf8_percent_encode(&param_ident.to_string(), PATH_SEGMENT_ENCODE_SET)
        );

        let body = serde_json::to_string(&param_entity).expect("impossible to fail to serialize");

        let hyper_client = (self.hyper_client)();
        let request = hyper_client.request(hyper::method::Method::Put, &url);
        let mut custom_headers = hyper::header::Headers::new();

        let request = request.body(&body);

        custom_headers.set(ContentType(mimetypes::requests::UPDATE_WORK.clone()));
        context.x_span_id.as_ref().map(|header| custom_headers.set(XSpanId(header.clone())));

        let request = request.headers(custom_headers);

        // Helper function to provide a code block to use `?` in (to be replaced by the `catch` block when it exists).
        fn parse_response(mut response: hyper::client::response::Response) -> Result<UpdateWorkResponse, ApiError> {
            match response.status.to_u16() {
                200 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::EntityEdit>(&buf)?;

                    Ok(UpdateWorkResponse::UpdatedEntity(body))
                }
                400 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateWorkResponse::BadRequest(body))
                }
                401 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;
                    header! { (ResponseWwwAuthenticate, "WWW_Authenticate") => [String] }
                    let response_www_authenticate = response
                        .headers
                        .get::<ResponseWwwAuthenticate>()
                        .ok_or_else(|| "Required response header WWW_Authenticate for response 401 was not found.")?;

                    Ok(UpdateWorkResponse::NotAuthorized {
                        body: body,
                        www_authenticate: response_www_authenticate.0.clone(),
                    })
                }
                403 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateWorkResponse::Forbidden(body))
                }
                404 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateWorkResponse::NotFound(body))
                }
                500 => {
                    let mut buf = String::new();
                    response.read_to_string(&mut buf).map_err(|e| ApiError(format!("Response was not valid UTF8: {}", e)))?;
                    let body = serde_json::from_str::<models::ErrorResponse>(&buf)?;

                    Ok(UpdateWorkResponse::GenericError(body))
                }
                code => {
                    let mut buf = [0; 100];
                    let debug_body = match response.read(&mut buf) {
                        Ok(len) => match str::from_utf8(&buf[..len]) {
                            Ok(body) => Cow::from(body),
                            Err(_) => Cow::from(format!("<Body was not UTF8: {:?}>", &buf[..len].to_vec())),
                        },
                        Err(e) => Cow::from(format!("<Failed to read body: {}>", e)),
                    };
                    Err(ApiError(format!("Unexpected response code {}:\n{:?}\n\n{}", code, response.headers, debug_body)))
                }
            }
        }

        let result = request.send().map_err(|e| ApiError(format!("No response received: {}", e))).and_then(parse_response);
        Box::new(futures::done(result))
    }
}

#[derive(Debug)]
pub enum ClientInitError {
    InvalidScheme,
    InvalidUrl(hyper::error::ParseError),
    MissingHost,
    SslError(openssl::error::ErrorStack),
}

impl From<hyper::error::ParseError> for ClientInitError {
    fn from(err: hyper::error::ParseError) -> ClientInitError {
        ClientInitError::InvalidUrl(err)
    }
}

impl From<openssl::error::ErrorStack> for ClientInitError {
    fn from(err: openssl::error::ErrorStack) -> ClientInitError {
        ClientInitError::SslError(err)
    }
}

impl fmt::Display for ClientInitError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (self as &fmt::Debug).fmt(f)
    }
}

impl error::Error for ClientInitError {
    fn description(&self) -> &str {
        "Failed to produce a hyper client."
    }
}
