use std::cmp;
use std::error;
use std::fs::File;
use std::io::BufReader;
use std::ops::Deref;
use std::path::Path;

use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;

use rocket::serde::json::{json, Json, Value};
use rocket::State;

use log::{warn, error};

use crate::common::Camera;
use crate::common::CameraId;
use crate::common::CameraList;
use crate::common::CameraMap;


#[get("/")]
async fn list_cameras(cameras_state: &State<RwLock<CameraMap>>) -> Json<CameraList> {
	let cameras = cameras_state.read().await;
	Json((*cameras).clone().values().cloned().collect())
}

#[get("/<id>")]
async fn get_camera(id: CameraId, cameras_state: &State<RwLock<CameraMap>>) -> Option<Json<Camera>> {
	let cameras = cameras_state.read().await;
	cameras.get(&id).map(|camera| {
		Json(camera.clone())
	})
}

#[post("/", data="<camera_json>")]
async fn new_camera(camera_json: Json<Camera>, cameras_state: &State<RwLock<CameraMap>>) -> Option<Json<Camera>> {
	let mut cameras = cameras_state.write().await;
	let mut camera = camera_json.into_inner();
	let id = next_camera_id(&cameras);
	camera.id = id;
	cameras.insert(id, camera.clone());
	write_config_file(cameras.deref()).await;
	Some(Json(camera))
}

fn next_camera_id(cameras: &CameraMap) -> CameraId {
	let mut highest_id : CameraId = 0;
	for id in cameras.keys() {
		highest_id = cmp::max(highest_id, *id);
	}
	return highest_id + 1;
}



const cameras_file_name : &str = "/tmp/clustervms/clustervms.yaml";

fn read_config_file() -> CameraMap {
	let file_result = File::open(Path::new(cameras_file_name));

	match file_result {
		Ok(file) => {
			let buf_reader = BufReader::new(file);
			let cameras_result = serde_yaml::from_reader(buf_reader);
			match cameras_result {
				Ok(cameras) => cameras,
				Err(err) => {
					error!("Failed to read camera config file; error was {}", err);
					return CameraMap::new();
				}
			}
		},
		Err(err) => {
			// First time running (before config file is created) we should encounter this, so not necessarily an error
			warn!("Failed to open camera config file for reading; error was {}", err);
			return CameraMap::new();
		}
	}
}

async fn write_config_file_inner(cameras: &CameraMap) -> Result<(), Box<dyn error::Error>> {
	let file = tokio::fs::File::create(Path::new(cameras_file_name)).await?;

	let yaml = serde_yaml::to_vec(&cameras)?;
	let mut writer = tokio::io::BufWriter::new(file);
	writer.write(&yaml).await?;
	writer.flush().await?;
	Ok(())
}

async fn write_config_file(cameras: &CameraMap) {
	match write_config_file_inner(cameras).await {
		Ok(_) => {
			info!("Wrote camera config file");
		},
		Err(err) => {
			error!("Failed to write camera config file; error was {}", err);
		}
	}
}

#[catch(404)]
fn not_found() -> Value {
	json!({
		"status": "error",
		"reason": "Resource was not found."
	})
}



pub fn stage() -> rocket::fairing::AdHoc {
	let cameras = read_config_file();
	
	// Using tokio::sync::RwLock rather than std::sync::Mutex or std::sync::RwLock so that:
	//     A.) Multiple readers can read our config at the same time without blocking each other
	//     B.) A new reader cannot acquire the lock while a writer is waiting for it.
	let cameras_lock = tokio::sync::RwLock::new(cameras);

	rocket::fairing::AdHoc::on_ignite("JSON", |rocket| async {
		rocket
			.manage(cameras_lock)
			.register("/", catchers![not_found])
			.mount("/v0/cameras", routes![list_cameras, get_camera, new_camera])
	})
}
