use std::path::PathBuf;
use std::sync::Arc;

use knowerage_mcp::registry::{auto_full_reconcile_enabled, Registry};
use knowerage_mcp::security::RegistryLock;

fn main() {
    env_logger::init();

    let workspace_root = std::env::var("KNOWERAGE_WORKSPACE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().expect("Cannot determine working directory"));

    log::info!(
        "Knowerage MCP server starting, workspace: {}",
        workspace_root.display()
    );

    let registry_lock = Arc::new(RegistryLock::new());
    let registry = Registry::with_lock(workspace_root.clone(), Arc::clone(&registry_lock));

    let _watcher = if auto_full_reconcile_enabled() {
        match registry.start_watcher() {
            Ok(w) => {
                log::info!(
                    "KNOWERAGE_AUTO_FULL_RECONCILE enabled: watching knowerage/ for changes"
                );
                Some(w)
            }
            Err(e) => {
                log::warn!("File watcher failed to start: {e}");
                None
            }
        }
    } else {
        log::info!("KNOWERAGE_AUTO_FULL_RECONCILE disabled: file watcher not started");
        None
    };

    let server = knowerage_mcp::mcp::McpServer::new_with_lock(workspace_root, registry_lock);
    if let Err(e) = server.run_stdio() {
        log::error!("MCP server error: {e}");
        std::process::exit(1);
    }
}
