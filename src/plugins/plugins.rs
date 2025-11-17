use wasmtime::component::{Component, Linker, HasSelf};
use wasmtime::{Config, Engine, Store};
use crate::error::Error;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::backends::BackendType;

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
    pub backends: Vec<BackendType>,
    #[serde(default)]
    pub hooks: Vec<PluginHook>,
}

#[derive(Debug, serde::Deserialize, Clone, Eq, Hash, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PluginHook {
    #[serde(rename = "before_send")]
    BeforeSend,
    #[serde(rename = "after_send")]
    AfterSend,
    #[serde(rename = "before_receive")]
    BeforeReceive,
    #[serde(rename = "after_receive")]
    AfterReceive,
}

/// Plugin Manager - owns all loaded plugins
pub struct PluginManager {
    plugins: HashMap<PluginHook, Vec<LoadedPlugin>>,
    engine: Engine,
	linker: Linker<PluginState>,
    host_state: TermailHostState,
}

impl std::fmt::Debug for PluginManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PluginManager {{ plugins: {:?}, engine: {:?} }}", self.plugins.keys(), self.engine)
    }
}

/// A loaded plugin with its runtime state
/// 
/// This is a termail-specific struct that is used to store the plugin's state.
pub struct LoadedPlugin {
    // Not sure if we actually need the name of the plugin for anything. Maybe for
    // logging/debugging purposes in the future?
    pub name: String,
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
        
        // Optimize Cranelift for speed even in debug builds
        config.cranelift_opt_level(wasmtime::OptLevel::Speed);
        
        let engine = Engine::new(&config)
            .map_err(|e| Error::Plugin(format!("Failed to create wasmtime engine: {}", e)))?;
        
		let linker = Linker::new(&engine);
        Ok(Self {
            plugins: HashMap::new(),
            engine,
			linker,
			host_state: TermailHostState::new(),
        })
    }

    /// Load plugins from directories
    /// 
    /// If no plugin directory is found, nothing is loaded.
    pub fn load_plugins(
        &mut self,
        enabled_plugins: &[String],
    ) -> Result<u32, Error> {
        // Check .config/termail/plugins first, fall back to ./plugins
        let plugin_dir = PathBuf::from(".config/termail/plugins");
        let plugin_dir = if plugin_dir.exists() {
            plugin_dir
        } else {
            PathBuf::from("./plugins")
        };

        if !plugin_dir.exists() {
            println!("No plugin directory found, skipping plugin loading");
            return Ok(0);
        }

        let mut loaded_plugins = 0;
        
        for entry in std::fs::read_dir(&plugin_dir)
            .map_err(|e| Error::Plugin(
                format!("Failed to read plugin dir {:?}: {}", plugin_dir, e))
            )?
            .filter_map(|entry| entry.ok())
        {
            let plugin_dir = entry.path();
            let manifest_path = plugin_dir.join("manifest.toml");
            if !manifest_path.exists() {
                continue;
            }
            let manifest = self.load_manifest(&manifest_path)
                .map_err(|e| Error::Plugin(format!("Failed to load manifest for plugin {:?}: {}", manifest_path, e)))?;

            let Some(manifest) = manifest else {
                continue;
            };

            if enabled_plugins.contains(&manifest.name.to_lowercase()) {
                println!("Loading plugin: {}", manifest.name);
                self.load_plugin(&plugin_dir, manifest)?;
                loaded_plugins += 1;
            } else {
                println!("Plugin {} is not enabled, skipping", manifest.name);
            }
        }

        Ok(loaded_plugins)
    }

    /// Load a single plugin manifest
    /// 
    /// If a plugin has no backends it can operate on, it should not be loaded.
    fn load_manifest(&self, manifest_path: &Path) -> Result<Option<PluginManifest>, Error> {
        let content = std::fs::read_to_string(manifest_path)
            .map_err(|e| Error::Plugin(format!("Failed to read manifest: {}", e)))?;
        
        let manifest: Result<PluginManifest, Error> = toml::from_str(&content)
            .map_err(|e| Error::Plugin(format!("Failed to parse manifest: {}", e)));
    
        match manifest {
            Ok(m) => {
                if m.backends.is_empty() {
                    println!("Warning! Plugin {} has an empty \"backends\" field, and will NOT be loaded.", m.name);
                    return Ok(None)
                }
                return Ok(Some(m))
            }
            Err(e) => Err(e)
        }
    }


    /// Load a single plugin from directory
    fn load_plugin(
        &mut self,
        plugin_dir: &Path,
        manifest: PluginManifest,
    ) -> Result<(), Error> {
        // Prefer pre-compiled .cwasm for much faster loading, fall back to .wasm
        let cwasm_path = plugin_dir.join("plugin.cwasm");
        let wasm_path = plugin_dir.join("plugin.wasm");
        
        let component = if cwasm_path.exists() {
            println!("Loading pre-compiled plugin: {:?}", cwasm_path);
            unsafe { Component::deserialize_file(&self.engine, &cwasm_path) }
                .map_err(|e| Error::Plugin(format!("Failed to load pre-compiled WASM: {}", e)))?
        } else if wasm_path.exists() {
            println!("Loading plugin (JIT compilation): {:?}", wasm_path);
            Component::from_file(&self.engine, &wasm_path)
                .map_err(|e| Error::Plugin(format!("Failed to load WASM: {}", e)))?
        } else {
            return Err(Error::Plugin(format!(
                "Plugin {} missing plugin.wasm or plugin.cwasm",
                manifest.name
            )));
        };

		Plugin::add_to_linker::<PluginState, HasSelf<PluginState>>(&mut self.linker, |state: &mut PluginState| state)
			.map_err(|e| Error::Plugin(format!("Failed to add to linker: {}", e)))?;

		let mut store = Store::new(&self.engine, PluginState { 
            invocation_count: 0, 
            host_state: self.host_state.clone()
        });

        for hook in manifest.hooks {
            self.plugins.entry(hook).or_insert(Vec::new()).push(LoadedPlugin {
                name: manifest.name.clone(),
                _instance: Plugin::instantiate(
                    &mut store,
                    &component,
                    &self.linker,
                )
                .map_err(|e| Error::Plugin(format!("Failed to instantiate: {}", e)))?,
            });
        }
        Ok(())

    }

    // pub fn dispatch_event(&self, hook: PluginHook, backend: BackendType, event: String) -> Result<String, Error> {
        
    // }

}
