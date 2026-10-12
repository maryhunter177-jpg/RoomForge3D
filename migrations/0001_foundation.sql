CREATE TABLE users (
 id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
 email text UNIQUE NOT NULL,
 password_hash text NOT NULL,
 role text NOT NULL DEFAULT 'user' CHECK (role IN ('admin', 'user')),
 created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE categories (
 id serial PRIMARY KEY, slug text UNIQUE NOT NULL, name text NOT NULL, segment text NOT NULL
);
CREATE TABLE assets (
 id uuid PRIMARY KEY DEFAULT gen_random_uuid(), name text NOT NULL,
 category_id int REFERENCES categories(id), tags text[] NOT NULL DEFAULT '{}',
 glb_url text NOT NULL, glb_lod1_url text, glb_lod2_url text, thumb_url text,
 width_m real, depth_m real, height_m real, triangles int, size_bytes bigint,
 placement text NOT NULL DEFAULT 'floor' CHECK (placement IN ('floor','wall','ceiling','surface')),
 default_scale real NOT NULL DEFAULT 1, published boolean NOT NULL DEFAULT false,
 created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE projects (
 id uuid PRIMARY KEY DEFAULT gen_random_uuid(), user_id uuid NOT NULL REFERENCES users(id),
 name text NOT NULL, data jsonb NOT NULL, thumb_url text, updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX projects_user_id_idx ON projects(user_id);
CREATE INDEX assets_published_category_idx ON assets(published, category_id);
CREATE TABLE materials (
 id serial PRIMARY KEY, kind text NOT NULL CHECK (kind IN ('floor','wall')),
 name text NOT NULL, albedo_url text, normal_url text, roughness_url text, tile_m real NOT NULL DEFAULT 1
);
