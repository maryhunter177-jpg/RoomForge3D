mod assets;
mod ingest;
mod process_tree;
mod storage;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::{DefaultBodyLimit, Request, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, FromRow, PgPool};
use std::{
    env,
    error::Error,
    time::{SystemTime, UNIX_EPOCH},
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    storage: Option<storage::Storage>,
    ingest_slots: std::sync::Arc<tokio::sync::Semaphore>,
    db: PgPool,
    secret: String,
    dummy_hash: String,
    rate: std::sync::Arc<tokio::sync::Mutex<(std::time::Instant, u32)>>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Credentials {
    email: String,
    password: String,
}
#[derive(Serialize, FromRow)]
struct User {
    id: Uuid,
    email: String,
    role: String,
}
#[derive(FromRow)]
struct LoginUser {
    id: Uuid,
    email: String,
    role: String,
    password_hash: String,
}
#[derive(Serialize, Deserialize)]
struct Claims {
    sub: Uuid,
    exp: u64,
}
#[derive(Serialize)]
struct AuthResponse {
    token: String,
    user: User,
}
#[derive(Debug)]
struct ApiError(StatusCode, &'static str);
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(serde_json::json!({"error": self.1}))).into_response()
    }
}
fn internal(error: impl std::fmt::Display) -> ApiError {
    tracing::error!(%error, "request failed");
    ApiError(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Erro interno do servidor",
    )
}
fn validate(input: &Credentials) -> Result<String, ApiError> {
    let email = input.email.trim().to_lowercase();
    let parts: Vec<_> = email.split('@').collect();
    if email.len() > 254
        || parts.len() != 2
        || parts[0].is_empty()
        || !parts[1].contains('.')
        || parts[1].starts_with('.')
        || parts[1].ends_with('.')
        || email.chars().any(char::is_whitespace)
    {
        return Err(ApiError(StatusCode::BAD_REQUEST, "Email inválido"));
    }
    if input.password.chars().count() < 12 || input.password.len() > 128 {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "A senha deve ter ao menos 12 caracteres e no máximo 128 bytes",
        ));
    }
    Ok(email)
}
fn hash_password(password: &str) -> Result<String, ApiError> {
    Argon2::default()
        .hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng))
        .map(|hash| hash.to_string())
        .map_err(internal)
}
fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash).is_ok_and(|parsed| {
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok()
    })
}
fn issue_token(user: &User, secret: &str) -> Result<String, ApiError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(internal)?
        .as_secs();
    encode(
        &Header::new(Algorithm::HS256),
        &Claims {
            sub: user.id,
            exp: now + 3600,
        },
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(internal)
}
fn verify_token(token: &str, secret: &str) -> Result<Uuid, ApiError> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .map(|data| data.claims.sub)
    .map_err(|_| ApiError(StatusCode::UNAUTHORIZED, "Sessão inválida ou expirada"))
}
async fn rate_limit(State(state): State<AppState>, request: Request, next: Next) -> Response {
    if !request.uri().path().starts_with("/api/v1/auth/") {
        return next.run(request).await;
    }
    let mut window = state.rate.lock().await;
    if window.0.elapsed().as_secs() >= 60 {
        *window = (std::time::Instant::now(), 0);
    }
    if window.1 >= 60 {
        return ApiError(
            StatusCode::TOO_MANY_REQUESTS,
            "Muitas tentativas. Aguarde um minuto.",
        )
        .into_response();
    }
    window.1 += 1;
    drop(window);
    next.run(request).await
}
async fn register(
    State(state): State<AppState>,
    Json(input): Json<Credentials>,
) -> Result<(StatusCode, Json<AuthResponse>), ApiError> {
    let email = validate(&input)?;
    let password_hash = tokio::task::spawn_blocking(move || hash_password(&input.password))
        .await
        .map_err(internal)??;
    let result = sqlx::query_as::<_, User>(
        "INSERT INTO users(email,password_hash,role) VALUES($1,$2,'user') RETURNING id,email,role",
    )
    .bind(email)
    .bind(password_hash)
    .fetch_one(&state.db)
    .await;
    let user = result.map_err(|error| {
        if error
            .as_database_error()
            .is_some_and(|db| db.is_unique_violation())
        {
            ApiError(StatusCode::CONFLICT, "Email já cadastrado")
        } else {
            internal(error)
        }
    })?;
    let token = issue_token(&user, &state.secret)?;
    Ok((StatusCode::CREATED, Json(AuthResponse { token, user })))
}
async fn login(
    State(state): State<AppState>,
    Json(input): Json<Credentials>,
) -> Result<Json<AuthResponse>, ApiError> {
    let email = validate(&input)?;
    let found = sqlx::query_as::<_, LoginUser>(
        "SELECT id,email,role,password_hash FROM users WHERE email=$1",
    )
    .bind(email)
    .fetch_optional(&state.db)
    .await
    .map_err(internal)?;
    // Verify the startup dummy hash for unknown accounts to reduce timing disclosure.
    let hash = match &found {
        Some(user) => user.password_hash.clone(),
        None => state.dummy_hash.clone(),
    };
    let valid = tokio::task::spawn_blocking(move || verify_password(&input.password, &hash))
        .await
        .map_err(internal)?;
    let found = found.filter(|_| valid).ok_or(ApiError(
        StatusCode::UNAUTHORIZED,
        "Email ou senha incorretos",
    ))?;
    let user = User {
        id: found.id,
        email: found.email,
        role: found.role,
    };
    let token = issue_token(&user, &state.secret)?;
    Ok(Json(AuthResponse { token, user }))
}
async fn me(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<User>, ApiError> {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(ApiError(
            StatusCode::UNAUTHORIZED,
            "Autenticação necessária",
        ))?;
    let id = verify_token(token, &state.secret)?;
    let user = sqlx::query_as::<_, User>("SELECT id,email,role FROM users WHERE id=$1")
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .map_err(internal)?
        .ok_or(ApiError(StatusCode::UNAUTHORIZED, "Sessão inválida"))?;
    Ok(Json(user))
}
async fn health(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    sqlx::query("SELECT 1")
        .execute(&state.db)
        .await
        .map_err(|_| ApiError(StatusCode::SERVICE_UNAVAILABLE, "Banco indisponível"))?;
    Ok(Json(serde_json::json!({"status":"ok"})))
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "roomforge_api=info,tower_http=info".into()),
        )
        .init();
    let secret = env::var("JWT_SECRET")?;
    if secret.len() < 32 {
        return Err("JWT_SECRET must contain at least 32 bytes".into());
    }
    let origin = env::var("CORS_ORIGIN")
        .unwrap_or_else(|_| "http://localhost:5173".into())
        .parse::<HeaderValue>()?;
    if origin == "*" {
        return Err("CORS_ORIGIN must be an explicit origin".into());
    }
    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(&env::var("DATABASE_URL")?)
        .await?;
    sqlx::migrate!("../../migrations").run(&db).await?;
    if let Ok(email) = env::var("ADMIN_EMAIL") {
        let email = email.trim().to_lowercase();
        sqlx::query("UPDATE users SET role='admin' WHERE email=$1")
            .bind(email)
            .execute(&db)
            .await?;
    }
    sqlx::query("UPDATE assets SET processing_status='failed',processing_error='Processamento interrompido; envie novamente' WHERE processing_status='processing'").execute(&db).await?;
    let cors = CorsLayer::new()
        .allow_origin(origin)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::PATCH,
        ])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]);
    let dummy_hash = hash_password("dummy-password-not-an-account")
        .map_err(|_| "Failed to initialize password hashing")?;
    let storage = storage::Storage::from_env()?;
    let state = AppState {
        storage,
        ingest_slots: std::sync::Arc::new(tokio::sync::Semaphore::new(2)),
        db,
        secret,
        dummy_hash,
        rate: std::sync::Arc::new(tokio::sync::Mutex::new((std::time::Instant::now(), 0))),
    };
    let app = Router::new()
        .merge(assets::routes(state.clone()))
        .route("/health", get(health))
        .route("/api/v1/auth/register", post(register))
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/me", get(me))
        .route_layer(middleware::from_fn_with_state(state.clone(), rate_limit))
        .layer(DefaultBodyLimit::max(4096))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);
    let host = env::var("API_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port = env::var("API_PORT")
        .unwrap_or_else(|_| "8080".into())
        .parse::<u16>()?;
    let listener = tokio::net::TcpListener::bind(format!("{host}:{port}")).await?;
    tracing::info!(%host, port, "API ready");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            if let Err(error) = tokio::signal::ctrl_c().await {
                tracing::error!(%error, "shutdown signal failed");
            }
        })
        .await?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_and_normalizes_credentials() {
        let input = Credentials {
            email: " USER@Example.com ".into(),
            password: "a-long-password".into(),
        };
        assert_eq!(validate(&input).ok().as_deref(), Some("user@example.com"));
        assert!(validate(&Credentials {
            email: "invalid".into(),
            password: "a-long-password".into()
        })
        .is_err());
        assert!(validate(&Credentials {
            email: "user@example.com".into(),
            password: "short".into()
        })
        .is_err());
    }
    #[test]
    fn passwords_are_salted_and_reject_incorrect_values() {
        let first = hash_password("secure-password").ok();
        let second = hash_password("secure-password").ok();
        assert!(first.is_some());
        assert_ne!(first, second);
        if let Some(hash) = first {
            assert!(verify_password("secure-password", &hash));
            assert!(!verify_password("wrong-password", &hash));
        }
        assert!(!verify_password("password", "corrupted"));
    }
    #[test]
    fn tokens_reject_wrong_signatures_and_expired_claims() {
        let user = User {
            id: Uuid::new_v4(),
            email: "user@example.com".into(),
            role: "user".into(),
        };
        let secret = "test-secret-with-at-least-32-bytes";
        let token = issue_token(&user, secret).ok();
        assert!(token.is_some());
        if let Some(token) = token {
            assert_eq!(verify_token(&token, secret).ok(), Some(user.id));
            assert!(verify_token(&token, "another-secret").is_err());
        }
        let expired = encode(
            &Header::new(Algorithm::HS256),
            &Claims {
                sub: user.id,
                exp: 1,
            },
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .ok();
        if let Some(token) = expired {
            assert!(verify_token(&token, secret).is_err());
        }
    }
    #[tokio::test]
    async fn routes_reject_missing_auth_invalid_input_and_rate_overflow() {
        use tower::ServiceExt;
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://test:test@localhost/test")
            .expect("valid test connection URL");

        let state = AppState {
            storage: None,
            ingest_slots: std::sync::Arc::new(tokio::sync::Semaphore::new(2)),
            db,
            secret: "test-secret-with-at-least-32-bytes".into(),
            dummy_hash: String::new(),
            rate: std::sync::Arc::new(tokio::sync::Mutex::new((std::time::Instant::now(), 0))),
        };
        let app = Router::new()
            .route("/api/v1/auth/me", get(me))
            .route("/api/v1/auth/register", post(register))
            .route_layer(middleware::from_fn_with_state(state.clone(), rate_limit))
            .with_state(state.clone());
        let request = axum::http::Request::builder()
            .uri("/api/v1/auth/me")
            .body(axum::body::Body::empty())
            .expect("valid test request");
        assert_eq!(
            app.clone()
                .oneshot(request)
                .await
                .expect("router response")
                .status(),
            StatusCode::UNAUTHORIZED
        );
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/v1/auth/register")
            .header(header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(
                r#"{"email":"invalid","password":"short"}"#,
            ))
            .expect("valid test request");
        assert_eq!(
            app.clone()
                .oneshot(request)
                .await
                .expect("router response")
                .status(),
            StatusCode::BAD_REQUEST
        );
        state.rate.lock().await.1 = 60;
        let request = axum::http::Request::builder()
            .uri("/api/v1/auth/me")
            .body(axum::body::Body::empty())
            .expect("valid test request");
        assert_eq!(
            app.oneshot(request)
                .await
                .expect("router response")
                .status(),
            StatusCode::TOO_MANY_REQUESTS
        );
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn registration_login_and_me_roundtrip(pool: PgPool) {
        let state = AppState {
            storage: None,
            ingest_slots: std::sync::Arc::new(tokio::sync::Semaphore::new(2)),
            db: pool,
            secret: "test-secret-with-at-least-32-bytes".into(),
            dummy_hash: hash_password("dummy-password-not-an-account").expect("dummy hash"),
            rate: std::sync::Arc::new(tokio::sync::Mutex::new((std::time::Instant::now(), 0))),
        };
        let credentials = || Credentials {
            email: " Person@example.com ".into(),
            password: "very-secure-password".into(),
        };
        let result = register(State(state.clone()), Json(credentials())).await;
        assert!(result.is_ok());
        let (status, Json(registered)) = result.expect("registration succeeds");
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(registered.user.role, "user");
        assert_eq!(registered.user.email, "person@example.com");
        let duplicate = register(State(state.clone()), Json(credentials())).await;
        assert!(matches!(duplicate, Err(ApiError(StatusCode::CONFLICT, _))));
        let login_result = login(State(state.clone()), Json(credentials())).await;
        assert!(login_result.is_ok());
        let Json(logged_in) = login_result.expect("login succeeds");
        assert_eq!(logged_in.user.id, registered.user.id);
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            format!("Bearer {}", logged_in.token)
                .parse()
                .expect("valid bearer header"),
        );
        let current = me(State(state.clone()), headers).await;
        assert!(current.is_ok());
        assert_eq!(
            current.expect("authenticated profile").0.id,
            registered.user.id
        );
        let bad_password = Credentials {
            email: "person@example.com".into(),
            password: "wrong-but-long-password".into(),
        };
        assert!(matches!(
            login(State(state.clone()), Json(bad_password)).await,
            Err(ApiError(StatusCode::UNAUTHORIZED, _))
        ));
        let missing = Credentials {
            email: "missing@example.com".into(),
            password: "wrong-but-long-password".into(),
        };
        assert!(matches!(
            login(State(state), Json(missing)).await,
            Err(ApiError(StatusCode::UNAUTHORIZED, _))
        ));
    }
}
