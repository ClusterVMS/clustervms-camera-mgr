use std::collections::HashMap;



pub type CameraId = u64;
pub type StreamId = u64;
pub type CameraList = Vec<Camera>;
pub type CameraMap = HashMap<CameraId, Camera>;

#[derive(Clone)]
#[derive(Debug)]
#[derive(Serialize, Deserialize)]
pub struct Camera {
	pub id: CameraId,
	pub name: String,
	pub streams: Vec<Stream>,
}

#[derive(Clone)]
#[derive(Debug)]
#[derive(Serialize, Deserialize)]
pub struct Stream {
	pub id: StreamId,
	pub source_url: String,
	pub recast_url: Option<String>,
}
