use wit_bindgen::generate;

generate!({
    path: "../../wit/main.wit",
    world: "plugin",
});

// use tm::plugin_system::host_api;
use tm::plugin_system::event_api;

struct SignaturePlugin;

/// This is the main entry point for the plugin. You MUST implement the `Guest` trait.
/// However, the name for the plugin can be anything you want.
impl Guest for SignaturePlugin {
    // As described in the `main.wit` file, this function will be called
    // when a specific event is triggered.
    fn on_notify(_invocation_id: String, event: event_api::Event) -> event_api::Event {
        let return_event = match event {
            event_api::Event::BeforeSend(content) => {
                event_api::Event::BeforeSend(content + "\n\n--\nSent from signature-rs!")
            }
            // If the event is not a BeforeSend event, return it as-is
            // Of course, termail will never trigger an unsubscribed 
            // event on a plugin, so this is just to exhaust the match.
            _ => event,
        };
        
        // let host_response = host_api::call_host(&invocation_id, "Hello from signature-rs!");
        // let response_text = match host_response {
        //     Ok(resp) => format!(" (Host said: {})", resp),
        //     Err(e) => format!(" (Host error: {})", e),
        // };
        // let signature = format!("\n\n--\nSent from signature-rs!");
        
        return_event
    }
}

// This is the macro that will be called when the plugin is exported.
// It's used to export the plugin to `termail` so that it can be loaded.
export!(SignaturePlugin);
