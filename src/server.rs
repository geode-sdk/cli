use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct ApiResponse<T> {
	pub error: String,
	pub payload: T,
}

#[derive(Deserialize, Debug)]
pub struct PaginatedData<T> {
	pub data: Vec<T>,
	pub count: i32,
}
