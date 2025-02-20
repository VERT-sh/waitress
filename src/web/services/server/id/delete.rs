use actix_web::{delete, web::Data, HttpMessage, HttpRequest, Responder};

use crate::{
    db::{server::ServerDeletionError, Database},
    web::response::ApiResponse,
};

#[delete("/delete")]
pub async fn delete(
    req: HttpRequest,
    data: Data<Database>,
) -> Result<impl Responder, ServerDeletionError> {
    let server = req
        .extensions_mut()
        .remove::<crate::db::server::Server>()
        .ok_or_else(|| ServerDeletionError::ServerNotFound)?;

    server.delete(&data.pool).await?;

    Ok(ApiResponse::Success(()))
}
