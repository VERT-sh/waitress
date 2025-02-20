pub mod auth;
pub mod owns_server;

#[macro_export]
macro_rules! middleware_error {
    ($status:expr, $($arg:tt)*) => {
        {
            use ::actix_web::error::*;
            $status(crate::web::response::ApiResponse::Error(format!($($arg)*)))
        }
    };
    ($($arg:tt)*) => {
        middleware_error!(::actix_web::error::ErrorInternalServerError, $($arg)*)
    };
}
