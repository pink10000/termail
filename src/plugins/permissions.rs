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

async fn install_plugin(repo_url: String, commit: String, cfg: TermailConfig) -> Result<(),()> {
	let url = gix::url::parse(BStr::new(&repo_url)).unwrap();

	// Handle someone pressing ctrl+c? don't do any memory ops in here
	// literally just leave it like this and don't add anything it's from
	// the documentation 
	// https://github.com/GitoxideLabs/gitoxide/blob/main/gix/examples/clone.rs
	unsafe {
		gix::interrupt::init_handler(1, || {}).unwrap();
	}
	
	// Create a directory for the plugin that has the same path as the git repo
	// It's probably safe to do! I join with "/" on the path first to avoid 
	// a directory traversal vulnerability :3
	let path = Path::new(&cfg.plugin_dir).join(
		Path::new("/").join(url.path.to_string())
	);

	std::fs::create_dir_all(&path);

	let mut prepare_fetch = gix::prepare_clone(url, &path).unwrap();

	let (repo, outcome) = prepare_fetch.fetch_only(gix::progress::Discard, &gix::interrupt::IS_INTERRUPTED).await.unwrap();
	
	//let remote = repo.find_default_remote(gix::remote::Direction::Fetch);
	Ok(())
}

//update plugin

fn load_plugin(git_path: String, cfg: TermailConfig) {
	
}

fn read_manifest() {
	//let deserialized = serde_json::from_str(s);
}