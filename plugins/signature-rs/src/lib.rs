use wit_bindgen::generate;

generate!({
    path: "../../wit/main.wit",
    world: "plugin",
});

struct SignaturePlugin;

/// This is the main entry point for the plugin. You MUST implement the `Guest` trait.
/// However, the name for the plugin can be anything you want.
impl Guest for SignaturePlugin {
    // As described in the `main.wit` file, this function will be called
    // when a specific event is triggered.
    fn on_notify(invocation_id: String, event: String) -> String {
        let host_response =
            tm::plugin_system::host_api::call_host(&invocation_id, "Hello from signature-rs!");
        let response_text = match host_response {
            Ok(resp) => format!(" (Host said: {})", resp),
            Err(e) => format!(" (Host error: {})", e),
        };
        let signature = format!("\n\n--\nSent from signature-rs{}", response_text);
        format!("{}{}", event, signature)
    }
}

// This is the macro that will be called when the plugin is exported.
// It's used to export the plugin to `termail` so that it can be loaded.
export!(SignaturePlugin);
