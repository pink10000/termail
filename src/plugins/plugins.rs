use crate::error::Error;
use crate::plugins::events::Hook;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

use crate::backends::BackendType;

pub mod bindings {
    wasmtime::component::bindgen!({
        path: "wit/main.wit",
        world: "plugin",
    });
}

use bindings::Plugin;
use bindings::tm::plugin_system::host_api;
use bindings::tm::plugin_system::event_api;

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
    pub hooks: Vec<Hook>,
}

/// Plugin Manager - owns all loaded plugins
pub struct PluginManager {
    plugins: HashMap<Hook, Vec<LoadedPlugin>>,
    engine: Engine,
    linker: Linker<PluginState>,
    host_state: TermailHostState,
}

impl std::fmt::Debug for PluginManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PluginManager {{ plugins: {:?}, engine: {:?} }}",
            self.plugins.keys(),
            self.engine
        )
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
    /// Maps invocation_id to the WIT event that's currently being processed
    /// This allows plugins to query the host about the current event context
    /// We probably do not need to wrap this in an `Arc` and `Mutex`
    /// since it is only used within the same thread.
    pub active_invocations: Arc<Mutex<HashMap<String, event_api::Event>>>,
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
    // Per-plugin WASI context (tracks fds, env vars, etc.) that backs guest syscalls.
    wasi_ctx: WasiCtx,
    // Resource table shared with wasi_ctx; required by wasmtime's preview2 runtime.
    wasi_table: ResourceTable,
    host_state: TermailHostState,
}

/// Implement the host API for plugins to call the host as defined in the `main.wit` file.
impl host_api::Host for PluginState {
    fn call_host(&mut self, invocation_id: String, request: String) -> Result<String, String> {
        let invocations = self.host_state.active_invocations.lock().unwrap();
        if let Some(_event) = invocations.get(&invocation_id) {
            Ok(format!("Host processed: {}", request))
        } else {
            Err(format!("Invalid invocation ID: {}", invocation_id))
        }
    }
}

/// Implement WasiView to provide WASI support to plugins.
/// 
/// Wasmtime calls these accessors whenever guest code performs a WASI syscall.
impl WasiView for PluginState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi_ctx,
            table: &mut self.wasi_table,
        }
    }
}

impl PluginManager {
    pub fn new() -> Result<Self, Error> {
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.cranelift_opt_level(wasmtime::OptLevel::Speed);

        let engine = Engine::new(&config)
            .map_err(|e| Error::Plugin(format!("Failed to create wasmtime engine: {}", e)))?;

        let mut linker = Linker::new(&engine);

        // Add WASI support to the linker (preview2, sync version wrapped in spawn_blocking)
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker)
            .map_err(|e| Error::Plugin(format!("Failed to add WASI to linker: {}", e)))?;

        let mut host_api = linker
            .instance("tm:plugin-system/host-api")
            .map_err(|e| Error::Plugin(format!("Failed to get host API instance: {}", e)))?;
        host_api
            .func_wrap(
                "call-host",
                |caller: wasmtime::StoreContextMut<PluginState>,
                 (id, req): (String, String)|
                 -> wasmtime::Result<(Result<String, String>,)> {
                    let host_state = caller.data().host_state.clone();
                    let invocations = host_state.active_invocations.lock().unwrap();
                    if let Some(_event) = invocations.get(&id) {
                        Ok((Ok(format!("Host processed: {}", req)),))
                    } else {
                        Ok((Err(format!("Invalid invocation ID: {}", id)),))
                    }
                },
            )
            .map_err(|e| Error::Plugin(format!("Failed to define call-host: {}", e)))?;

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
    pub fn load_plugins(&mut self, enabled_plugins: &[String]) -> Result<u32, Error> {
        // Check .config/termail/plugins first, fall back to ./plugins
        let plugin_dir = PathBuf::from(".config/termail/plugins");
        let plugin_dir = if plugin_dir.exists() {
            plugin_dir
        } else {
            PathBuf::from("./plugins")
        };

        if !plugin_dir.exists() {
            return Ok(0);
        }

        let mut loaded_plugins_count = 0;

        for entry in std::fs::read_dir(&plugin_dir)
            .map_err(|e| {
                Error::Plugin(format!("Failed to read plugin dir {:?}: {}", plugin_dir, e))
            })?
            .filter_map(|entry| entry.ok())
        {
            let plugin_dir = entry.path();

            let manifest_path = match plugin_dir.join("manifest.toml").exists() {
                true => plugin_dir.join("manifest.toml"),
                false => continue,
            };

            let Some(manifest) = self.load_manifest(&manifest_path).map_err(|e| {
                Error::Plugin(format!(
                    "Failed to load manifest for plugin {:?}: {}",
                    manifest_path, e
                ))
            })? else {
                continue;
            };

            if enabled_plugins.contains(&manifest.name.to_lowercase()) {
                self.load_plugin(&plugin_dir, manifest)?;
                loaded_plugins_count += 1;
            } else {
                println!("Plugin {} is not enabled, skipping", manifest.name);
            }
        }

        Ok(loaded_plugins_count)
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
                    return Ok(None);
                }
                return Ok(Some(m));
            }
            Err(e) => Err(e),
        }
    }

    /// Load a single plugin from directory
    /// 
    /// cwasm is faster, but it uses unsafe
    fn load_plugin(&mut self, plugin_dir: &Path, manifest: PluginManifest) -> Result<(), Error> {
        // Prefer pre-compiled .cwasm for much faster loading, fall back to .wasm
        let cwasm_path = plugin_dir.join("plugin.cwasm");
        let wasm_path = plugin_dir.join("plugin.wasm");

        let component = if cwasm_path.exists() {
            unsafe { Component::deserialize_file(&self.engine, &cwasm_path) }
                .map_err(|e| Error::Plugin(format!("Failed to load pre-compiled WASM: {}", e)))?
        } else if wasm_path.exists() {
            Component::from_file(&self.engine, &wasm_path)
                .map_err(|e| Error::Plugin(format!("Failed to load WASM: {}", e)))?
        } else {
            return Err(Error::Plugin(format!(
                "Plugin {} missing \"plugin.wasm\" or \"plugin.cwasm\"",
                manifest.name
            )));
        };

        for hook in manifest.hooks {
            let mut wasi_builder = WasiCtxBuilder::new();
            // If we need stdin/env, inherit_* helpers can expose them here.
            let wasi_ctx = wasi_builder.build();

            let mut store = Store::new(
                &self.engine,
                PluginState {
                    wasi_ctx,
                    wasi_table: ResourceTable::new(),
                    host_state: self.host_state.clone(),
                },
            );

            // TODO: A maintainable/readable error message that tells users potential fixes. For example, 
            // sometimes the user may have forgotten to recompile the plugin (this has happened to me).
            let instance = Plugin::instantiate(&mut store, &component, &self.linker)
                .map_err(|e| Error::Plugin(format!("Failed to instantiate plugin \"{}\" with error: {}", manifest.name, e)))?;

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
    /// Returns the final content string after all plugins have processed the event.
    pub async fn dispatch(&mut self, event: event_api::Event) -> Result<event_api::Response, Error> {
        // Get the hook for this event to find which plugins to call
        let hook = event.hook();
        
        // Get the plugins registered for this hook
       /* let plugins = match self.plugins.get_mut(&hook) {
            Some(plugins) if !plugins.is_empty() => plugins,
            _ => {
                // No plugins registered for this hook, return the content as-is
                return Ok(event.content().to_string());
            }
        };*/
        let mut plugins: Vec<LoadedPlugin> = Vec::new();
        let mut response = event_api::Response::AfterReceiveResponse(());
        for plugin in plugins.iter_mut() {
            let invocation_id = uuid::Uuid::new_v4().to_string();

            self.host_state
                .active_invocations
                .lock()
                .unwrap()
                .insert(invocation_id.clone(), event.clone());

            // Call the plugin's on-notify function and get the modified event back
            // Use block_in_place to allow sync WASI calls without crossing thread boundaries
            response = tokio::task::block_in_place(|| {
                plugin
                    .instance
                    .call_notify(&mut plugin.store, &event, &invocation_id)
            }).map_err(|e| Error::Plugin(format!("Plugin {} failed: {}", plugin.name, e)))?;

            // Remove from active_invocations after processing
            self.host_state
                .active_invocations
                .lock()
                .unwrap()
                .remove(&invocation_id);

            println!("[Host] Plugin {} processed event", plugin.name);
        }

        // Return the final content string
        Ok(response)
    }
}
