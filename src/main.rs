use axum::{
    routing::{get, post},
    extract::{
        Extension,
        Path,
        Json,
    },
    http::{HeaderMap},
    Router,
};

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(root))
        .route("/:org/:repo/objects/batch", post(objects_batch));

    let addr = "0.0.0.0:3000";
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    println!("Listening on {}", addr);
    axum::serve(listener, app).await.unwrap();
}

async fn root() -> &'static str {
    "Hubless is runnng"
}

async fn objects_batch(
    headers: HeaderMap,
    Path((org, repo)): Path<(String, String)>,
    Json(body): Json<serde_json::Value>
) {
    println!("Got batch request for {}/{}", org, repo);
    println!("Body: {}", body);
    println!("Headers: {:?}", headers);
}