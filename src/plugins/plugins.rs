use wasmtime::component::{Component, Linker, HasSelf};
use wasmtime::{Config, Engine, Store};
use crate::error::Error;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

mod bindings {
    wasmtime::component::bindgen!({
        path: "wit/main.wit",
        world: "plugin",
    });
}

use bindings::tm::plugin_system::termail_host;
use bindings::Plugin;

/// Manifest structure for plugin.toml
#[derive(Debug, serde::Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub website: String,
    #[serde(default)]
    pub dispatchers: Vec<String>,
}

/// Plugin Manager - owns all loaded plugins
pub struct PluginManager {
    plugins: HashMap<String, LoadedPlugin>,
    engine: Engine,
	linker: Linker<PluginState>,
}

/// A loaded plugin with its runtime state
/// 
/// This is a termail-specific struct that is used to store the plugin's state.
pub struct LoadedPlugin {
    pub name: String,
    pub description: String,
    pub dispatchers: Vec<String>,
    _store: Store<PluginState>,
    _instance: Plugin,
}

/// Global Host State shared across all plugins
#[derive(Clone)]
pub struct TermailHostState {
    pub active_invocations: Arc<Mutex<HashMap<String, String>>>, // invocation_id -> event
}

impl TermailHostState {
    pub fn new() -> Self {
        Self {
            active_invocations: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

/// Plugin Store Data - each plugin instance gets its own
/// 
/// This is specific to wasmtime and is used to store the plugin's state.
struct PluginState {
    invocation_count: u32,
    host_state: TermailHostState,
}

/// Implement the `termail-host` interface trait
impl termail_host::Host for PluginState {
    fn invoke(&mut self, invocation_id: String, event: String) -> String {
        self.invocation_count += 1;
        invoke(&self.host_state, invocation_id, event)
    }
}

/// Host Logic
pub fn invoke(host_state: &TermailHostState, invocation_id: String, event: String) -> String {
    let invocations = host_state.active_invocations.lock().unwrap();
    if let Some(expected_event) = invocations.get(&invocation_id) {
        if expected_event == &event {
            println!("[Host] Valid 'invoke' call for event: {}", event);
            return format!("Processed: {}", event);
        }
    }
    println!("[Host] Invalid invocation_id or event mismatch");
    "Error: Invalid invocation".to_string()
}

impl PluginManager {
    pub fn new() -> Result<Self, Error> {
        let mut config = Config::new();
        config.wasm_component_model(true);
        let engine = Engine::new(&config)
            .map_err(|e| Error::Plugin(format!("Failed to create wasmtime engine: {}", e)))?;
        
		let linker = Linker::new(&engine);
        Ok(Self {
            plugins: HashMap::new(),
            engine,
			linker,
        })
    }

    /// Load plugins from directories, filtered by enabled list
    pub fn load_plugins(
        &mut self,
        host_state: &mut TermailHostState,
        enabled_plugins: &[String],
    ) -> Result<u32, Error> {
        // Check .config/plugins first, then ./plugins
        let search_dirs = vec![
            PathBuf::from(".config/termail/plugins"),
            PathBuf::from("./plugins"),
        ];

		let mut loaded_plugins = 0;
        for dir in search_dirs {
			if !dir.exists() {
                continue;
            }

            // Scan for plugin directories
            for entry in std::fs::read_dir(&dir)
                .map_err(|e| Error::Plugin(format!("Failed to read dir {:?}: {}", dir, e)))? 
            {
                let entry = entry.map_err(|e| Error::Plugin(format!("Failed to read entry: {}", e)))?;
                let path = entry.path();

				if !path.is_dir() {
					continue;
				}
				let manifest_path = path.join("manifest.toml");
				
				// A plugin is defined by its manifest.toml file. If it doesn't exsit,
				// then it is not a plugin.
				if !manifest_path.exists() {
					continue;
				}

				match self.load_manifest(&manifest_path) {
					Ok(manifest) => {
						if enabled_plugins.contains(&manifest.name.to_lowercase()) {
							println!("Loading plugin: {}", manifest.name);
							self.load_plugin(host_state, &path, manifest)?;
							loaded_plugins += 1;
						} else {
							println!("Plugin {} is not enabled, skipping", manifest.name);
						}
					}
					Err(e) => {
						println!("Failed to load manifest: {:?}", manifest_path);
						return Err(Error::Plugin(format!(
							"Failed to load manifest ({:?}), with error: {}", manifest_path, e)));
					}
				}
            }
        }

        Ok(loaded_plugins)
    }

    /// Load a single plugin manifest
    fn load_manifest(&self, manifest_path: &Path) -> Result<PluginManifest, Error> {
        let content = std::fs::read_to_string(manifest_path)
            .map_err(|e| Error::Plugin(format!("Failed to read manifest: {}", e)))?;
        
        toml::from_str(&content)
            .map_err(|e| Error::Plugin(format!("Failed to parse manifest: {}", e)))
    }

    /// Load a single plugin from directory
    fn load_plugin(
        &mut self,
        host_state: &mut TermailHostState,
        plugin_dir: &Path,
        manifest: PluginManifest,
    ) -> Result<(), Error> {
        // Look for .wasm file
        let wasm_path = plugin_dir.join("plugin.wasm");
		println!("WASM path: {:?}", wasm_path);
        if !wasm_path.exists() {
            return Err(Error::Plugin(format!(
                "Plugin {} missing plugin.wasm",
                manifest.name
            )));
        }

		let component = Component::from_file(&self.engine, &wasm_path)
            .map_err(|e| Error::Plugin(format!("Failed to load WASM: {}", e)))?;

		Plugin::add_to_linker::<PluginState, HasSelf<PluginState>>(&mut self.linker, |state: &mut PluginState| state)
			.map_err(|e| Error::Plugin(format!("Failed to add to linker: {}", e)))?;

		let mut store = Store::new(&self.engine, PluginState { invocation_count: 0, host_state: host_state.clone() });
		let instance = Plugin::instantiate(&mut store, &component, &self.linker)
			.map_err(|e| Error::Plugin(format!("Failed to instantiate: {}", e)))?;

        let loaded_plugin = LoadedPlugin {
            name: manifest.name.clone(),
            description: manifest.description,
            dispatchers: manifest.dispatchers,
            _store: store,
            _instance: instance,
        };

        self.plugins.insert(manifest.name, loaded_plugin);
        Ok(())
    }
}