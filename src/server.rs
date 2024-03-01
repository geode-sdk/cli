use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct ApiResponse<T> {
	pub error: Option<String>,
	pub payload: Option<T>,
}
