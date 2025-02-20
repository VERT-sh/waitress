#[macro_export]
macro_rules! error_variants {
    ($name:ident { $($variant:ident($status:ident)),* $(,)? }) => {
        impl ::actix_web::ResponseError for $name {
            fn error_response(&self) -> ::actix_web::HttpResponse<::actix_web::body::BoxBody> {
                let status = match self {
                    $(
                        $name::$variant { .. } => ::actix_web::http::StatusCode::$status,
                    )*
                };

                ::actix_web::HttpResponse::build(status).json(crate::web::response::ApiResponse::Error::<()>(self.to_string()))
            }
        }
    };
}
