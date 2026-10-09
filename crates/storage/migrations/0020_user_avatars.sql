-- Profile pictures. One small, validated image per user, kept apart from the
-- users table so listings never load the image bytes.
CREATE TABLE IF NOT EXISTS user_avatars (
  user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
  content_type TEXT NOT NULL CHECK(content_type IN ('image/png', 'image/jpeg', 'image/webp')),
  data BLOB NOT NULL,
  updated_at TEXT NOT NULL
);
