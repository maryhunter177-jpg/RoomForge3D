use super::{internal, me, ApiError, AppState};
use axum::{
    extract::Request,
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    middleware::{self, Next},
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use std::{env, path::PathBuf};
use uuid::Uuid;

#[derive(Serialize, FromRow)]
pub struct Asset {
    pub id: Uuid,
    pub name: String,
    pub category_id: Option<i32>,
    pub tags: Vec<String>,
    pub glb_url: String,
    pub glb_lod1_url: Option<String>,
    pub glb_lod2_url: Option<String>,
    pub thumb_url: Option<String>,
    pub width_m: Option<f32>,
    pub depth_m: Option<f32>,
    pub height_m: Option<f32>,
    pub triangles: Option<i32>,
    pub size_bytes: Option<i64>,
    pub placement: String,
    pub default_scale: f32,
    pub published: bool,
    pub created_at: DateTime<Utc>,
    pub processing_status: String,
    pub processing_error: Option<String>,
    #[serde(skip_serializing)]
    pub storage_key: Option<String>,
    #[serde(skip_serializing)]
    pub lod1_key: Option<String>,
    #[serde(skip_serializing)]
    pub lod2_key: Option<String>,
    #[serde(skip_serializing)]
    pub thumb_key: Option<String>,
}
#[derive(Deserialize, Default)]
pub struct Filters {
    category: Option<i32>,
    q: Option<String>,
    segment: Option<String>,
}
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct AssetPatch {
    name: Option<String>,
    #[serde(default, deserialize_with = "nullable_category")]
    category_id: Option<Option<i32>>,
    tags: Option<Vec<String>>,
    placement: Option<String>,
    default_scale: Option<f32>,
    width_m: Option<f32>,
    depth_m: Option<f32>,
    height_m: Option<f32>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Publish {
    published: bool,
}
#[derive(Serialize, Deserialize, FromRow)]
pub struct Category {
    id: i32,
    slug: String,
    name: String,
    segment: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CategoryInput {
    slug: String,
    name: String,
    segment: String,
}
#[derive(Serialize, FromRow)]
pub struct Material {
    id: i32,
    kind: String,
    name: String,
    albedo_url: Option<String>,
    normal_url: Option<String>,
    roughness_url: Option<String>,
    tile_m: f32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialInput {
    kind: String,
    name: String,
    albedo_url: Option<String>,
    normal_url: Option<String>,
    roughness_url: Option<String>,
    tile_m: f32,
}
#[derive(Deserialize)]
pub struct MaterialFilter {
    kind: Option<String>,
}
fn nullable_category<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Option<i32>>, D::Error> {
    Ok(Some(Option::<i32>::deserialize(deserializer)?))
}
fn bad(message: &'static str) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, message)
}
fn nonempty(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}
fn segment(value: &str) -> bool {
    matches!(
        value,
        "escritorio" | "quarto" | "oficina" | "consultorio" | "clinica" | "estudio"
    )
}
fn validate_patch(value: &AssetPatch) -> Result<(), ApiError> {
    if value.name.as_ref().is_some_and(|name| !nonempty(name, 120))
        || value.category_id.flatten().is_some_and(|id| id <= 0)
        || value
            .tags
            .as_ref()
            .is_some_and(|tags| tags.len() > 30 || tags.iter().any(|tag| !nonempty(tag, 50)))
        || value
            .placement
            .as_ref()
            .is_some_and(|p| !matches!(p.as_str(), "floor" | "wall" | "ceiling" | "surface"))
        || [
            value.default_scale,
            value.width_m,
            value.depth_m,
            value.height_m,
        ]
        .iter()
        .flatten()
        .any(|v| !v.is_finite() || *v <= 0.0 || *v > 1000.0)
    {
        return Err(bad("Metadados inválidos"));
    }
    Ok(())
}
pub async fn require_admin(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    match me(State(state), request.headers().clone()).await {
        Ok(Json(user)) if user.role == "admin" => next.run(request).await,
        Ok(_) => {
            ApiError(StatusCode::FORBIDDEN, "Acesso restrito ao administrador").into_response()
        }
        Err(error) => error.into_response(),
    }
}
pub fn routes(state: AppState) -> Router<AppState> {
    let admin = Router::new()
        .route("/api/v1/admin/assets", get(admin_assets))
        .route(
            "/api/v1/admin/assets/upload",
            post(upload).layer(axum::extract::DefaultBodyLimit::max(50 * 1024 * 1024)),
        )
        .route(
            "/api/v1/admin/assets/{id}",
            get(admin_asset).patch(patch_asset).delete(delete_asset),
        )
        .route("/api/v1/admin/assets/{id}/publish", post(publish))
        .route(
            "/api/v1/admin/categories",
            get(categories).post(create_category),
        )
        .route(
            "/api/v1/admin/categories/{id}",
            get(get_category)
                .patch(update_category)
                .put(update_category)
                .delete(delete_category),
        )
        .route(
            "/api/v1/admin/materials",
            get(materials).post(create_material),
        )
        .route(
            "/api/v1/admin/materials/{id}",
            get(get_material)
                .patch(update_material)
                .put(update_material)
                .delete(delete_material),
        )
        .route_layer(middleware::from_fn_with_state(state, require_admin));
    Router::new()
        .route("/api/v1/assets", get(assets))
        .route("/api/v1/assets/{id}", get(asset))
        .route("/api/v1/categories", get(categories))
        .route("/api/v1/materials", get(materials))
        .merge(admin)
}
async fn signed(state: &AppState, mut asset: Asset) -> Result<Asset, ApiError> {
    if let Some(storage) = &state.storage {
        if let Some(key) = &asset.storage_key {
            asset.glb_url = storage.url(key).await?;
        }
        if let Some(key) = &asset.lod1_key {
            asset.glb_lod1_url = Some(storage.url(key).await?);
        }
        if let Some(key) = &asset.lod2_key {
            asset.glb_lod2_url = Some(storage.url(key).await?);
        }
        if let Some(key) = &asset.thumb_key {
            asset.thumb_url = Some(storage.url(key).await?);
        }
    }
    Ok(asset)
}
async fn list(
    state: &AppState,
    filters: Filters,
    admin: bool,
) -> Result<Json<Vec<Asset>>, ApiError> {
    if filters.q.as_ref().is_some_and(|q| q.len() > 120)
        || filters.segment.as_ref().is_some_and(|s| !segment(s))
    {
        return Err(bad("Filtro inválido"));
    }
    let rows=sqlx::query_as::<_,Asset>("SELECT a.* FROM assets a LEFT JOIN categories c ON c.id=a.category_id WHERE ($1 OR a.published) AND ($2::int IS NULL OR a.category_id=$2) AND ($3::text IS NULL OR a.name ILIKE '%' || $3 || '%') AND ($4::text IS NULL OR c.segment=$4) ORDER BY a.created_at DESC LIMIT 500").bind(admin).bind(filters.category).bind(filters.q).bind(filters.segment).fetch_all(&state.db).await.map_err(internal)?;
    let mut result = Vec::with_capacity(rows.len());
    for asset in rows {
        result.push(signed(state, asset).await?);
    }
    Ok(Json(result))
}
async fn assets(
    State(state): State<AppState>,
    Query(filters): Query<Filters>,
) -> Result<Json<Vec<Asset>>, ApiError> {
    list(&state, filters, false).await
}
async fn admin_assets(
    State(state): State<AppState>,
    Query(filters): Query<Filters>,
) -> Result<Json<Vec<Asset>>, ApiError> {
    list(&state, filters, true).await
}
async fn find(state: &AppState, id: Uuid, admin: bool) -> Result<Asset, ApiError> {
    let asset =
        sqlx::query_as::<_, Asset>("SELECT * FROM assets WHERE id=$1 AND ($2 OR published)")
            .bind(id)
            .bind(admin)
            .fetch_optional(&state.db)
            .await
            .map_err(internal)?
            .ok_or(ApiError(StatusCode::NOT_FOUND, "Modelo não encontrado"))?;
    signed(state, asset).await
}
async fn asset(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Asset>, ApiError> {
    Ok(Json(find(&state, id, false).await?))
}
async fn admin_asset(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Asset>, ApiError> {
    Ok(Json(find(&state, id, true).await?))
}
async fn patch_asset(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(patch): Json<AssetPatch>,
) -> Result<Json<Asset>, ApiError> {
    validate_patch(&patch)?;
    check_category(&state, patch.category_id.flatten()).await?;
    let asset=sqlx::query_as::<_,Asset>("UPDATE assets SET name=coalesce($2,name),category_id=CASE WHEN $10 THEN $3 ELSE category_id END,tags=coalesce($4,tags),placement=coalesce($5,placement),default_scale=coalesce($6,default_scale),width_m=coalesce($7,width_m),depth_m=coalesce($8,depth_m),height_m=coalesce($9,height_m) WHERE id=$1 RETURNING *").bind(id).bind(patch.name).bind(patch.category_id.flatten()).bind(patch.tags).bind(patch.placement).bind(patch.default_scale).bind(patch.width_m).bind(patch.depth_m).bind(patch.height_m).bind(patch.category_id.is_some()).fetch_optional(&state.db).await.map_err(internal)?.ok_or(ApiError(StatusCode::NOT_FOUND,"Modelo não encontrado"))?;
    Ok(Json(signed(&state, asset).await?))
}
async fn publish(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<Publish>,
) -> Result<Json<Asset>, ApiError> {
    let asset=sqlx::query_as::<_,Asset>("UPDATE assets SET published=$2 WHERE id=$1 AND (NOT $2 OR processing_status='ready') RETURNING *").bind(id).bind(input.published).fetch_optional(&state.db).await.map_err(internal)?.ok_or(ApiError(StatusCode::CONFLICT,"Modelo inexistente ou otimização ainda não concluída"))?;
    Ok(Json(signed(&state, asset).await?))
}
async fn delete_asset(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let asset = find(&state, id, true).await?;
    if asset.processing_status == "processing" {
        return Err(ApiError(
            StatusCode::CONFLICT,
            "Aguarde o processamento antes de excluir",
        ));
    }
    if let Some(storage) = &state.storage {
        for key in [
            &asset.storage_key,
            &asset.lod1_key,
            &asset.lod2_key,
            &asset.thumb_key,
        ]
        .into_iter()
        .flatten()
        {
            storage.delete(key).await?;
        }
    }
    sqlx::query("DELETE FROM assets WHERE id=$1")
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}
async fn check_category(state: &AppState, id: Option<i32>) -> Result<(), ApiError> {
    if let Some(id) = id {
        let exists =
            sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM categories WHERE id=$1)")
                .bind(id)
                .fetch_one(&state.db)
                .await
                .map_err(internal)?;
        if !exists {
            return Err(bad("Categoria inexistente"));
        }
    }
    Ok(())
}
async fn upload(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<Asset>), ApiError> {
    if state.storage.is_none() {
        return Err(ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "Armazenamento não configurado",
        ));
    }
    let mut file = None;
    let mut thumbnail = None;
    let mut patch = AssetPatch::default();
    let mut seen = std::collections::HashSet::new();
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|_| bad("Upload multipart inválido"))?
    {
        let name = field
            .name()
            .ok_or_else(|| bad("Campo sem nome"))?
            .to_string();
        if !seen.insert(name.clone()) {
            return Err(bad("Campo duplicado"));
        }
        let limit = if name == "file" {
            50 * 1024 * 1024
        } else if name == "thumbnail" {
            2 * 1024 * 1024
        } else {
            8192
        };
        let mut bytes = Vec::new();
        while let Some(chunk) = field
            .chunk()
            .await
            .map_err(|_| bad("Upload excedeu o limite"))?
        {
            if bytes.len() + chunk.len() > limit {
                return Err(ApiError(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "Arquivo grande demais",
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        match name.as_str() {
            "file" => file = Some(bytes),
            "thumbnail" => {
                if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
                    return Err(bad("Thumbnail deve ser PNG"));
                }
                thumbnail = Some(bytes)
            }
            "name" => {
                patch.name = Some(String::from_utf8(bytes).map_err(|_| bad("Nome inválido"))?)
            }
            "category_id" => {
                patch.category_id = Some(Some(
                    String::from_utf8(bytes)
                        .map_err(|_| bad("Categoria inválida"))?
                        .parse()
                        .map_err(|_| bad("Categoria inválida"))?,
                ))
            }
            "tags" => {
                patch.tags = Some(
                    serde_json::from_slice(&bytes)
                        .map_err(|_| bad("Tags devem ser uma lista JSON"))?,
                )
            }
            "placement" => {
                patch.placement =
                    Some(String::from_utf8(bytes).map_err(|_| bad("Posicionamento inválido"))?)
            }
            _ => return Err(bad("Campo de upload desconhecido")),
        }
    }
    validate_patch(&patch)?;
    check_category(&state, patch.category_id.flatten()).await?;
    let name = patch.name.ok_or_else(|| bad("Informe o nome do modelo"))?;
    let bytes = file.ok_or_else(|| bad("Envie o arquivo GLB"))?;
    let max_bytes = env::var("ASSET_MAX_BYTES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20 * 1024 * 1024)
        .min(50 * 1024 * 1024);
    let max_triangles = env::var("ASSET_MAX_TRIANGLES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(150000);
    let metadata = super::ingest::inspect(&bytes, max_bytes, max_triangles)?;
    let asset=sqlx::query_as::<_,Asset>("INSERT INTO assets(name,category_id,tags,glb_url,width_m,depth_m,height_m,triangles,size_bytes,placement,processing_status) VALUES($1,$2,$3,'',$4,$5,$6,$7,$8,$9,'processing') RETURNING *").bind(name).bind(patch.category_id.flatten()).bind(patch.tags.unwrap_or_default()).bind(metadata.width).bind(metadata.depth).bind(metadata.height).bind(i32::try_from(metadata.triangles).map_err(internal)?).bind(i64::try_from(bytes.len()).map_err(internal)?).bind(patch.placement.unwrap_or_else(||"floor".into())).fetch_one(&state.db).await.map_err(internal)?;
    let id = asset.id;
    tokio::spawn(async move {
        if let Err(error) = process(&state, id, bytes, thumbnail).await {
            tracing::error!(?error,%id,"asset processing failed");
            if let Err(db_error) = sqlx::query(
                "UPDATE assets SET processing_status='failed',processing_error=$2 WHERE id=$1",
            )
            .bind(id)
            .bind(error.1)
            .execute(&state.db)
            .await
            {
                tracing::error!(%db_error,"failed to record pipeline error");
            }
        }
    });
    Ok((StatusCode::ACCEPTED, Json(asset)))
}
async fn process(
    state: &AppState,
    id: Uuid,
    bytes: Vec<u8>,
    thumbnail: Option<Vec<u8>>,
) -> Result<(), ApiError> {
    let _permit = state.ingest_slots.acquire().await.map_err(internal)?;
    let base = env::var("INGEST_TEMP_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("roomforge-ingest"));
    let dir = base.join(id.to_string());
    tokio::fs::create_dir_all(&dir).await.map_err(internal)?;
    let result = process_files(state, id, &dir, bytes, thumbnail).await;
    if let Err(error) = tokio::fs::remove_dir_all(&dir).await {
        tracing::warn!(%error,"temporary ingest cleanup failed");
    }
    result
}
async fn process_files(
    state: &AppState,
    id: Uuid,
    dir: &std::path::Path,
    bytes: Vec<u8>,
    thumbnail: Option<Vec<u8>>,
) -> Result<(), ApiError> {
    let input = dir.join("source.glb");
    let output = dir.join("optimized");
    tokio::fs::write(&input, bytes).await.map_err(internal)?;
    let script = env::var("OPTIMIZER_SCRIPT").unwrap_or_else(|_| "tools/optimize-model.mjs".into());
    let node = env::var("NODE_BINARY").unwrap_or_else(|_| "node".into());
    let mut command = tokio::process::Command::new(node);
    command
        .arg(script)
        .arg(&input)
        .arg(&output)
        .kill_on_drop(true);
    command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.as_std_mut().process_group(0);
    }
    #[cfg(windows)]
    command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
    let child = command.spawn().map_err(internal)?;
    let pid = child
        .id()
        .ok_or_else(|| internal("Optimizer process has no ID"))?;
    let _process_tree = super::process_tree::ProcessTree::attach(pid).map_err(internal)?;
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(300),
        child.wait_with_output(),
    )
    .await
    .map_err(|_| {
        ApiError(
            StatusCode::GATEWAY_TIMEOUT,
            "Otimização excedeu cinco minutos",
        )
    })?
    .map_err(internal)?;
    if !result.status.success() {
        tracing::error!(stderr=%String::from_utf8_lossy(&result.stderr),"optimizer failed");
        return Err(ApiError(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Falha na otimização do GLB",
        ));
    }
    let storage = state.storage.as_ref().ok_or(ApiError(
        StatusCode::SERVICE_UNAVAILABLE,
        "Armazenamento não configurado",
    ))?;
    let mut keys = Vec::new();
    for name in ["model.glb", "lod1.glb", "lod2.glb"] {
        let data = tokio::fs::read(output.join(name)).await.map_err(internal)?;
        if !data.starts_with(b"glTF") {
            return Err(bad("Otimização gerou GLB inválido"));
        }
        let hash = format!("{:x}", Sha256::digest(&data));
        let key = format!("assets/{id}/{hash}-{name}");
        storage.put(&key, &data, "model/gltf-binary").await?;
        keys.push(key);
    }
    let thumb_key = if let Some(data) = thumbnail {
        let key = format!("assets/{id}/{:x}.png", Sha256::digest(&data));
        storage.put(&key, &data, "image/png").await?;
        Some(key)
    } else {
        None
    };
    sqlx::query("UPDATE assets SET storage_key=$2,lod1_key=$3,lod2_key=$4,thumb_key=$5,processing_status='ready',processing_error=NULL WHERE id=$1").bind(id).bind(&keys[0]).bind(&keys[1]).bind(&keys[2]).bind(thumb_key).execute(&state.db).await.map_err(internal)?;
    Ok(())
}
fn validate_category(input: &CategoryInput) -> Result<(), ApiError> {
    if !nonempty(&input.name, 80)
        || input.slug.is_empty()
        || input.slug.len() > 80
        || !input
            .slug
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        || !segment(&input.segment)
    {
        return Err(bad("Categoria inválida"));
    }
    Ok(())
}
fn db_write(error: sqlx::Error) -> ApiError {
    if error
        .as_database_error()
        .is_some_and(|e| e.is_unique_violation() || e.is_foreign_key_violation())
    {
        ApiError(StatusCode::CONFLICT, "Registro duplicado ou em uso")
    } else {
        internal(error)
    }
}
async fn categories(State(state): State<AppState>) -> Result<Json<Vec<Category>>, ApiError> {
    Ok(Json(
        sqlx::query_as("SELECT * FROM categories ORDER BY name")
            .fetch_all(&state.db)
            .await
            .map_err(internal)?,
    ))
}
async fn get_category(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<Category>, ApiError> {
    Ok(Json(
        sqlx::query_as("SELECT * FROM categories WHERE id=$1")
            .bind(id)
            .fetch_optional(&state.db)
            .await
            .map_err(internal)?
            .ok_or(ApiError(StatusCode::NOT_FOUND, "Categoria não encontrada"))?,
    ))
}
async fn create_category(
    State(state): State<AppState>,
    Json(input): Json<CategoryInput>,
) -> Result<(StatusCode, Json<Category>), ApiError> {
    validate_category(&input)?;
    Ok((
        StatusCode::CREATED,
        Json(
            sqlx::query_as(
                "INSERT INTO categories(slug,name,segment) VALUES($1,$2,$3) RETURNING *",
            )
            .bind(input.slug)
            .bind(input.name)
            .bind(input.segment)
            .fetch_one(&state.db)
            .await
            .map_err(db_write)?,
        ),
    ))
}
async fn update_category(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(input): Json<CategoryInput>,
) -> Result<Json<Category>, ApiError> {
    validate_category(&input)?;
    Ok(Json(
        sqlx::query_as("UPDATE categories SET slug=$2,name=$3,segment=$4 WHERE id=$1 RETURNING *")
            .bind(id)
            .bind(input.slug)
            .bind(input.name)
            .bind(input.segment)
            .fetch_optional(&state.db)
            .await
            .map_err(db_write)?
            .ok_or(ApiError(StatusCode::NOT_FOUND, "Categoria não encontrada"))?,
    ))
}
async fn delete_category(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<StatusCode, ApiError> {
    let count = sqlx::query("DELETE FROM categories WHERE id=$1")
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(db_write)?
        .rows_affected();
    if count == 0 {
        return Err(ApiError(StatusCode::NOT_FOUND, "Categoria não encontrada"));
    }
    Ok(StatusCode::NO_CONTENT)
}
fn validate_material(input: &MaterialInput) -> Result<(), ApiError> {
    if !nonempty(&input.name, 120)
        || !matches!(input.kind.as_str(), "floor" | "wall")
        || !input.tile_m.is_finite()
        || input.tile_m <= 0.0
        || input.tile_m > 100.0
        || [&input.albedo_url, &input.normal_url, &input.roughness_url]
            .into_iter()
            .flatten()
            .any(|url| {
                url.len() > 2048 || !(url.starts_with("https://") || url.starts_with("http://"))
            })
    {
        return Err(bad("Material inválido"));
    }
    Ok(())
}
async fn materials(
    State(state): State<AppState>,
    Query(input): Query<MaterialFilter>,
) -> Result<Json<Vec<Material>>, ApiError> {
    if input
        .kind
        .as_ref()
        .is_some_and(|kind| !matches!(kind.as_str(), "floor" | "wall"))
    {
        return Err(bad("Tipo de material inválido"));
    }
    Ok(Json(
        sqlx::query_as("SELECT * FROM materials WHERE ($1::text IS NULL OR kind=$1) ORDER BY name")
            .bind(input.kind)
            .fetch_all(&state.db)
            .await
            .map_err(internal)?,
    ))
}
async fn get_material(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<Material>, ApiError> {
    Ok(Json(
        sqlx::query_as("SELECT * FROM materials WHERE id=$1")
            .bind(id)
            .fetch_optional(&state.db)
            .await
            .map_err(internal)?
            .ok_or(ApiError(StatusCode::NOT_FOUND, "Material não encontrado"))?,
    ))
}
async fn create_material(
    State(state): State<AppState>,
    Json(input): Json<MaterialInput>,
) -> Result<(StatusCode, Json<Material>), ApiError> {
    validate_material(&input)?;
    Ok((StatusCode::CREATED,Json(sqlx::query_as("INSERT INTO materials(kind,name,albedo_url,normal_url,roughness_url,tile_m) VALUES($1,$2,$3,$4,$5,$6) RETURNING *").bind(input.kind).bind(input.name).bind(input.albedo_url).bind(input.normal_url).bind(input.roughness_url).bind(input.tile_m).fetch_one(&state.db).await.map_err(db_write)?)))
}
async fn update_material(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(input): Json<MaterialInput>,
) -> Result<Json<Material>, ApiError> {
    validate_material(&input)?;
    Ok(Json(sqlx::query_as("UPDATE materials SET kind=$2,name=$3,albedo_url=$4,normal_url=$5,roughness_url=$6,tile_m=$7 WHERE id=$1 RETURNING *").bind(id).bind(input.kind).bind(input.name).bind(input.albedo_url).bind(input.normal_url).bind(input.roughness_url).bind(input.tile_m).fetch_optional(&state.db).await.map_err(db_write)?.ok_or(ApiError(StatusCode::NOT_FOUND,"Material não encontrado"))?))
}
async fn delete_material(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<StatusCode, ApiError> {
    let count = sqlx::query("DELETE FROM materials WHERE id=$1")
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(db_write)?
        .rows_affected();
    if count == 0 {
        return Err(ApiError(StatusCode::NOT_FOUND, "Material não encontrado"));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;
    #[sqlx::test(migrations = "../../migrations")]
    async fn admin_gate_uses_current_role_and_public_hides_drafts(pool: sqlx::PgPool) {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO users(id,email,password_hash,role) VALUES($1,'admin-test@example.com','unused','user')").bind(id).execute(&pool).await.expect("user");
        let state = AppState {
            db: pool.clone(),
            secret: "test-secret-with-at-least-32-bytes".into(),
            dummy_hash: String::new(),
            rate: std::sync::Arc::new(tokio::sync::Mutex::new((std::time::Instant::now(), 0))),
            storage: None,
            ingest_slots: std::sync::Arc::new(tokio::sync::Semaphore::new(2)),
        };
        let token = super::super::issue_token(
            &super::super::User {
                id,
                email: "admin-test@example.com".into(),
                role: "user".into(),
            },
            &state.secret,
        )
        .expect("token");
        let app = routes(state.clone()).with_state(state.clone());
        let request = || {
            Request::builder()
                .uri("/api/v1/admin/assets")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("request")
        };
        assert_eq!(
            app.clone()
                .oneshot(request())
                .await
                .expect("response")
                .status(),
            StatusCode::FORBIDDEN
        );
        sqlx::query("UPDATE users SET role='admin' WHERE id=$1")
            .bind(id)
            .execute(&pool)
            .await
            .expect("promote");
        assert_eq!(
            app.clone()
                .oneshot(request())
                .await
                .expect("response")
                .status(),
            StatusCode::OK
        );
        let draft=sqlx::query_scalar::<_,Uuid>("INSERT INTO assets(name,glb_url,processing_status) VALUES('Draft','http://example.test/model.glb','processing') RETURNING id").fetch_one(&pool).await.expect("draft");
        assert!(matches!(
            find(&state, draft, false).await,
            Err(ApiError(StatusCode::NOT_FOUND, _))
        ));
        assert!(matches!(
            publish(
                State(state.clone()),
                Path(draft),
                Json(Publish { published: true })
            )
            .await,
            Err(ApiError(StatusCode::CONFLICT, _))
        ));
        sqlx::query("UPDATE assets SET processing_status='ready' WHERE id=$1")
            .bind(draft)
            .execute(&pool)
            .await
            .expect("ready");
        let _ = publish(
            State(state.clone()),
            Path(draft),
            Json(Publish { published: true }),
        )
        .await
        .expect("publish");
        assert!(
            find(&state, draft, false)
                .await
                .expect("public asset")
                .published
        );
        let category = create_category(
            State(state.clone()),
            Json(CategoryInput {
                slug: "desk-test".into(),
                name: "Mesas".into(),
                segment: "escritorio".into(),
            }),
        )
        .await
        .expect("category")
        .1
         .0;
        let _ = patch_asset(
            State(state.clone()),
            Path(draft),
            Json(AssetPatch {
                category_id: Some(Some(category.id)),
                ..Default::default()
            }),
        )
        .await
        .expect("assign");
        assert!(matches!(
            delete_category(State(state.clone()), Path(category.id)).await,
            Err(ApiError(StatusCode::CONFLICT, _))
        ));
        delete_asset(State(state.clone()), Path(draft))
            .await
            .expect("delete asset");
        delete_category(State(state), Path(category.id))
            .await
            .expect("delete category");
    }
    #[test]
    fn rejects_invalid_metadata_and_materials() {
        assert!(validate_patch(&AssetPatch {
            default_scale: Some(f32::NAN),
            ..Default::default()
        })
        .is_err());
        assert!(validate_patch(&AssetPatch {
            placement: Some("invalid".into()),
            ..Default::default()
        })
        .is_err());
        assert!(validate_category(&CategoryInput {
            slug: "Bad Slug".into(),
            name: "Valid".into(),
            segment: "quarto".into()
        })
        .is_err());
        assert!(validate_material(&MaterialInput {
            kind: "floor".into(),
            name: "Madeira".into(),
            albedo_url: Some("javascript:bad".into()),
            normal_url: None,
            roughness_url: None,
            tile_m: 1.0
        })
        .is_err());
    }
}
