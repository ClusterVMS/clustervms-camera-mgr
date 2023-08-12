use clustervms::{BasicCameraInfo, Camera, CameraId, CameraMap, Stream, StreamId};
use clustervms::config::ConfigManager;
use log::error;
use rand::{thread_rng, Rng};
use rand::distributions::{Alphanumeric};
use rocket::serde::json::{json, Json, Value};
use rocket::State;
use tokio::sync::RwLock;
use url::Url;



#[get("/")]
async fn list_cameras(config_mgr_state: &State<RwLock<ConfigManager>>) -> Json<Vec<BasicCameraInfo>> {
	let config_mgr = config_mgr_state.read().await;
	let cameras = &config_mgr.get_config().cameras;

	Json(
		(*cameras).clone().values()
		.cloned()
		.map(|cam| BasicCameraInfo::from(cam))
		.collect()
	)
}

#[get("/?format=full")]
async fn list_cameras_full(config_mgr_state: &State<RwLock<ConfigManager>>) -> Json<Vec<Camera>> {
	let config_mgr = config_mgr_state.read().await;
	let cameras = &config_mgr.get_config().cameras;
	Json(cameras.clone().values().cloned().collect())
}

#[get("/<id>")]
async fn get_camera(id: CameraId, config_mgr_state: &State<RwLock<ConfigManager>>) -> Option<Json<Camera>> {
	let config_mgr = config_mgr_state.read().await;
	let cameras = &config_mgr.get_config().cameras;
	cameras.get(&id).map(|camera| {
		Json(camera.clone())
	})
}

// Creates a new Camera based on the info the user sent
// The ID in the user-supplied object is ignored (recommended to leave blank), and a new ID is generated.
#[post("/", data="<camera_json>")]
async fn new_camera(camera_json: Json<Camera>, config_mgr_state: &State<RwLock<ConfigManager>>) -> Option<Json<Camera>> {
	let mut config_mgr = config_mgr_state.write().await;
	let mut camera = camera_json.into_inner();
	let id = generate_camera_id(&config_mgr.get_config().cameras);
	camera.id = id.clone();

	let base_url = config_mgr.get_config().base_url.clone();

	// Set up recast stream URLs if not specified
	for (stream_id, mut stream) in &mut camera.streams {
		if stream.recast_url.is_none() {
			stream.recast_url = Url::parse(format!("{base_url}/v0/cameras/{id}/streams/{stream_id}/sdp").as_str()).ok();
		}
	}

	config_mgr.get_config_mut().cameras.insert(id.clone(), camera.clone());
	write_config_file(&config_mgr).await;

	Some(Json(camera))
}

#[put("/<id>", data="<camera_json>")]
async fn edit_camera(id: CameraId, camera_json: Json<Camera>, config_mgr_state: &State<RwLock<ConfigManager>>) -> Option<Json<Camera>> {
	let mut config_mgr = config_mgr_state.write().await;
	let camera = camera_json.into_inner();

	if id != camera.id {
		// Camera ID in the body does not match the one in the URL.
		// This likely means either a poorly-written client, or a deliberate attempt to attack the system.
		warn!("Camera ID in URL did not match ID in camera object; rejecting request.");
		return None;
	}

	config_mgr.get_config_mut().cameras.insert(id.clone(), camera.clone());
	write_config_file(&config_mgr).await;

	Some(Json(camera))
}

#[get("/<camera_id>/streams/<stream_id>")]
async fn get_stream(camera_id: CameraId, stream_id: StreamId, config_mgr_state: &State<RwLock<ConfigManager>>) -> Option<Json<Stream>> {
	let config_mgr = config_mgr_state.read().await;
	let cameras = &config_mgr.get_config().cameras;
	cameras.get(&camera_id).and_then(|camera| {
		camera.streams.get(&stream_id).map(|stream| {
			Json(stream.clone())
		})
	})
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

async fn write_config_file_inner(config_mgr: &ConfigManager) -> anyhow::Result<()> {
	config_mgr.write_config()?;

	Ok(())
}

async fn write_config_file(config_mgr: &ConfigManager) {
	match write_config_file_inner(config_mgr).await {
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



pub fn stage(config_mgr: ConfigManager) -> rocket::fairing::AdHoc {
	// Using tokio::sync::RwLock rather than std::sync::Mutex or std::sync::RwLock so that:
	//     A.) Multiple readers can read our config at the same time without blocking each other
	//     B.) A new reader cannot acquire the lock while a writer is waiting for it.
	let config_mgr_lock = tokio::sync::RwLock::new(config_mgr);

	rocket::fairing::AdHoc::on_ignite("JSON", |rocket| async {
		rocket
			.manage(config_mgr_lock)
			.register("/", catchers![not_found])
			.mount("/v0/cameras", routes![list_cameras, list_cameras_full, get_camera, edit_camera, new_camera, get_stream])
	})
}
