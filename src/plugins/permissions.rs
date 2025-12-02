use serde::{Deserialize, Serialize};
use zip::ZipArchive;
use std::{fs::File, iter::zip};

use crate::config::TermailConfig;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
enum Permission {
	#[serde(rename = "incoming_email.read")]
	IncomingEmailRead,
	#[serde(rename = "outgoing_email.modify")]
	OutgoingEmailModify,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
enum Hook {
	#[serde(rename = "email_received")]
	EmailReceived
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
	name: String,
	descripion: String,
	// When can a plugin run?
	hooks: Vec<Hook>,
	// What can a plugin do?
	permissions: Vec<Permission>
}

fn load_plugin(path: String, cfg: TermailConfig) {
	let zipped = File::open(path).unwrap();

	let archive = ZipArchive::new(zipped).unwrap();
	archive.by_path("")
	
}

fn read_manifest() {
	let deserialized = serde_json::from_str(s);
}