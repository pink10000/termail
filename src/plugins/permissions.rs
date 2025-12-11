use gix::{bstr::BStr};
use std::path::Path;
use serde::{Deserialize, Serialize};

use crate::config::TermailConfig;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
enum Permission {
	#[serde(rename = "incoming_email.read")]
	IncomingEmailRead,
	#[serde(rename = "outgoing_email.modify")]
	OutgoingEmailModify,
	CategorizeEmail
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

pub fn read_manifest() {
	//let deserialized = serde_json::from_str(s);
	// read the manifest use serde
}