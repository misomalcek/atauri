mod state;
mod routes;
mod ws;

use axum::Router;
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);

    let app_state = state::AppState::new();

    let api_routes = routes::api_router();

    let app = Router::new()
        .nest("/api", api_routes)
        .route("/ws/stream", axum::routing::get(ws::ws_handler))
        .with_state(app_state)
        .layer(CorsLayer::permissive())
        // Serve the Svelte dist/ folder for everything else
        .fallback_service(ServeDir::new("dist").append_index_html_on_directories(true));

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("Tauri Code Web running at http://localhost:{port}");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
