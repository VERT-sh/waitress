use std::fmt::{self, Display, Formatter};

use actix_web::{HttpResponse, Responder};
use serde::Serialize;

#[derive(Serialize, Debug)]
#[serde(tag = "type", content = "data")]
pub enum ApiResponse<T> {
    #[serde(rename = "success")]
    Success(T),
    #[serde(rename = "error")]
    Error(String),
}

impl<T: Serialize> Responder for ApiResponse<T> {
    type Body = actix_web::body::BoxBody;
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        match self {
            ApiResponse::Success(_) => HttpResponse::Ok().json(self),
            ApiResponse::Error(_) => HttpResponse::BadRequest().json(self),
        }
    }
}

impl Display for ApiResponse<String> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        // serialize the enum to a string
        let s = serde_json::to_string(self).map_err(|_| fmt::Error)?;
        write!(f, "{}", s)
    }
}
