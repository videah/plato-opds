//! Helper functions for interacting with the Plato e-reader software.

use serde_json::json;

/// Show a notification on the device with the given `message`.
pub fn show_notification(message: &str) {
    let event = json!({
        "type": "notify",
        "message": message,
    });
    println!("{event}");
}

/// Set the device's Wi-Fi state to `enable`.
pub fn set_wifi(enable: bool) {
    let event = json!({
        "type": "setWifi",
        "enable": enable,
    });
    println!("{event}");
}

/// Add a document to the device's library. The `doc` parameter should be a JSON object with the
/// document's metadata.
///
/// The format can be found in the [Plato codebase](https://github.com/baskerville/plato/blob/master/crates/core/src/metadata.rs).
pub fn add_document(doc: serde_json::Value) {
    let event = json!({
        "type": "addDocument",
        "info": doc,
    });
    println!("{event}");
}
