use crate::*;

use axum::http::{HeaderValue, header};
use notify::Watcher;
use tower_http::{services::ServeDir, set_header::SetResponseHeaderLayer};

pub fn run_project(port: u16) -> error::Result<()> {
	let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;

	runtime.block_on(async {
		let _watcher = watch_project()?;
		serve_project(port).await
	})
}

async fn serve_project(port: u16) -> error::Result<()> {
	let app = axum::Router::new()
		.fallback_service(ServeDir::new(DEFAULT_BUILD_PATH))
		.layer(SetResponseHeaderLayer::overriding(header::CACHE_CONTROL, HeaderValue::from_static("no-store")));

	let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;

	axum::serve(listener, app).await?;

	Ok(())
}

fn watch_project() -> error::Result<notify::RecommendedWatcher> {
	project::build_project(path::Path::new(DEFAULT_BUILD_PATH))?;

	let root = env::current_dir()?;
	let ignored = [root.join(DEFAULT_BUILD_PATH), root.join("target"), root.join(".git")];

	let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
		let event = match event {
			Ok(event) => event,
			Err(error) => {
				log::error!("Failed to watch project files: {error}");
				return;
			}
		};

		if !matches!(event.kind, notify::EventKind::Create(_) | notify::EventKind::Modify(_) | notify::EventKind::Remove(_)) {
			return;
		}

		if !event.paths.iter().any(|path| !ignored.iter().any(|ignored| path.starts_with(ignored))) {
			return;
		}

		log::info!("Files changed. Rebuilding project...");

		if let Err(error) = project::build_project(path::Path::new(DEFAULT_BUILD_PATH)) {
			log::error!("Build failed: {error:?}");
		}
	})?;

	watcher.watch(path::Path::new("."), notify::RecursiveMode::Recursive)?;

	Ok(watcher)
}
