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
    store: Store<PluginState>,
    instance: Plugin,
}

/// Global Host State shared across all plugins
#[derive(Clone)]
pub struct TermailHostState {
    pub active_invocations: Arc<Mutex<HashMap<String, String>>>, 
    // invocation_id -> event
    // we probably do not need to wrap this in an `Arc` and `Mutex` 
    // since it is only used within the same thread.
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
    _host_state: TermailHostState,
}

/// Implement the `termail-host` interface trait
impl termail_host::Host for PluginState {
    fn invoke(&mut self, _invocation_id: String, _event: String) -> String {
        self.invocation_count += 1;
        // invoke(&self.host_state, invocation_id, event)
        "OK".to_string()
    }
}

/// Host function called by plugins to update event data
/// 
/// When a plugin processes an event, it calls this function with:
/// - invocation_id: The ID provided in on_notify
/// - event: The modified event data
/// 
/// This updates the stored event data so the host can retrieve it after on_notify returns
pub fn invoke(host_state: &TermailHostState, invocation_id: String, event: String) -> String {
    let mut invocations = host_state.active_invocations.lock().unwrap();
    
    if invocations.contains_key(&invocation_id) {
        // Update the event data with the plugin's modified version
        invocations.insert(invocation_id.clone(), event.clone());
        println!("[Host] Plugin updated event via invoke: {}", invocation_id);
        format!("OK: Event updated")
    } else {
        eprintln!("[Host] Invalid invocation_id: {}", invocation_id);
        "Error: Invalid invocation_id".to_string()
    }
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
        
        for hook in manifest.hooks {
            let mut store = Store::new(&self.engine, PluginState { 
                invocation_count: 0, 
                _host_state: self.host_state.clone()
            });

            let instance = Plugin::instantiate(&mut store, &component, &self.linker)
                .map_err(|e| Error::Plugin(format!("Failed to instantiate: {}", e)))?;

            let loaded_plugin = LoadedPlugin {
                name: manifest.name.clone(),
                store,
                instance,
            };

            self.plugins
                .entry(hook)
                .or_insert_with(Vec::new)
                .push(loaded_plugin);
        }

        Ok(())
    }

    /// Dispatch an event to the appropriate plugins
    /// 
    /// Plugins are called in sequence, each receiving the output of the previous plugin.
    /// 
    /// # Flow
    /// 1. Host stores initial event data with a unique invocation_id
    /// 2. Host calls `plugin.on_notify(invocation_id, event)`
    /// 3. Plugin processes event and calls `host.invoke(invocation_id, modified_event)` 
    /// 4. Host's `invoke()` updates the stored event data
    /// 5. Host retrieves the modified event and passes it to the next plugin
    /// 6. Final modified event is returned
    pub fn dispatch_event(&mut self, hook: PluginHook, _backend: BackendType, event: String) -> Result<String, Error> {
        // Get plugins for this hook
        let plugins = match self.plugins.get_mut(&hook) {
            Some(plugins) if !plugins.is_empty() => plugins,
            _ => return Ok(event), // No plugins for this hook
        };

        let mut current_event = event;

        // Call each plugin in sequence
        for plugin in plugins {
            let invocation_id = uuid::Uuid::new_v4().to_string();

            // Step 1: Store the current event data in the invocation registry
            self.host_state
                .active_invocations
                .lock()
                .unwrap()
                .insert(invocation_id.clone(), current_event.clone());

            // Step 2: Call the plugin's on-notify function
            // The plugin will call invoke(invocation_id, modified_event) to update the data
            let out = plugin.instance
                .call_on_notify(&mut plugin.store, &invocation_id, &current_event)
                .map_err(|e| Error::Plugin(format!("Plugin {} failed: {}", plugin.name, e)))?;

            // // Step 5: Retrieve the (potentially modified) event data
            // // The plugin should have called invoke() which updated this
            // let invocations = self.host_state.active_invocations.lock().unwrap();
            // current_event = invocations
            //     .get(&invocation_id)
            //     .ok_or_else(|| Error::Plugin(format!("Plugin {} lost invocation data", plugin.name)))?
            //     .clone();
            // drop(invocations);

            // Clean up the invocation
            // self.host_state
            //     .active_invocations
            //     .lock()
            //     .unwrap()
            //     .remove(&invocation_id);

            current_event = out;
        }

        // Step 6: Return the final modified event
        Ok(current_event)
    }

}
