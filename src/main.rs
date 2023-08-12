#![feature(proc_macro_hygiene, decl_macro)]
#![feature(option_result_contains)]

#[macro_use] extern crate rocket;
#[macro_use] extern crate serde_derive;

use clap::{Command, Arg, ArgAction};
use clustervms::config;
use rocket::{Request, Response};
use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::Header;

mod rest_api;



// Since the UI is served by another server, we may need to setup CORS to allow the UI to make requests to this server.
pub struct CORS;

#[rocket::async_trait]
impl Fairing for CORS {
	fn info(&self) -> Info {
		Info {
			name: "Add CORS headers to responses",
			kind: Kind::Response
		}
	}

	async fn on_response<'r>(&self, _request: &'r Request<'_>, response: &mut Response<'r>) {
		response.set_header(Header::new("Access-Control-Allow-Origin", "*"));
		response.set_header(Header::new("Access-Control-Allow-Methods", "POST, GET, PATCH, OPTIONS"));
		response.set_header(Header::new("Access-Control-Allow-Headers", "*"));
		response.set_header(Header::new("Access-Control-Allow-Credentials", "true"));
	}
}


#[rocket::main]
async fn main() -> anyhow::Result<()> {
	let matches = Command::new("clustervms-camera-mgr")
		.version("0.0.4")
		.author("Alicrow")
		.about("Camera manager for ClusterVMS.")
		.arg(
			Arg::new("config")
				.action(ArgAction::Append)	// Allow argument to be specified multiple times
				.short('c')
				.long("config")
				.help("TOML file with ClusterVMS config")
		)
		.get_matches();

	let mut config_manager = config::ConfigManager::new();
	let config_filename_matches = matches.get_many::<String>("config");
	match config_filename_matches {
		Some(filenames) => {
			config_manager.read_config(filenames.map(|v| v.as_str()).collect())?;
		},
		None => {
			// Use default file path
			config_manager.read_default_config_files()?;
		}
	};

	rocket::build()
		.attach(rest_api::stage(config_manager.clone()))
		.attach(CORS)
		.launch()
		.await?;

	anyhow::Ok(())
}
