use clustervms::{BasicCameraInfo, Camera, CameraId, CameraMap};
use clustervms::config::ConfigManager;
use log::error;
use rand::{thread_rng, Rng};
use rand::distributions::{Alphanumeric};
use rocket::serde::json::{json, Json, Value};
use rocket::State;
use std::error;
use std::ops::Deref;
use std::path::Path;
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;



#[get("/")]
async fn list_cameras(cameras_state: &State<RwLock<CameraMap>>) -> Json<Vec<BasicCameraInfo>> {
	let cameras = cameras_state.read().await;
	Json(
		(*cameras).clone().values()
		.cloned()
		.map(|cam| BasicCameraInfo::from(cam))
		.collect()
	)
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
	let id = generate_camera_id(&cameras);
	cameras.insert(id, camera.clone());
	write_config_file(cameras.deref()).await;
	Some(Json(camera))
}

pub fn generate_camera_id(existing_ids: &CameraMap) -> CameraId {
	loop {
		let id: String = thread_rng().sample_iter(Alphanumeric).take(8).map(char::from).collect();
		// If the generated ID is not taken, return it.
		if !existing_ids.contains_key(id.as_str()) {
			return id;
	}
	}
}



const cameras_file_name : &str = "/tmp/clustervms/clustervms.yaml";

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



pub fn stage(config_mgr: &ConfigManager) -> rocket::fairing::AdHoc {
	let cameras = config_mgr.get_config().cameras.clone();
	
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
