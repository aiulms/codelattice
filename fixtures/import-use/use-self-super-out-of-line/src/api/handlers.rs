use super::models::Response;
use crate::api::models::Response as ApiResponse;
use crate::root_fn;

pub fn handle() -> Response {
    let _ = root_fn();
    Response { code: 200 }
}
