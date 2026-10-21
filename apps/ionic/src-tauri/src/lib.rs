//! Desktop window for the shared Ionic interface.
//!
//! Deliberately exposes no commands, filesystem, process, network proxy, or VPN
//! plugin. The frontend therefore retains its unavailable VPN bridge state.

pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("could not start the Aegis desktop window");
}
