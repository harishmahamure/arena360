CREATE TABLE units (
  id TEXT PRIMARY KEY
    CHECK (
      length(id) = 36
      AND substr(id, 9, 1) = '-'
      AND substr(id, 14, 1) = '-'
      AND substr(id, 19, 1) = '-'
      AND substr(id, 24, 1) = '-'
      AND id = lower(id)
      AND id NOT GLOB '*[^0-9a-f-]*'
    ),
  name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 100),
  abbreviation TEXT NOT NULL CHECK (length(trim(abbreviation)) BETWEEN 1 AND 20),
  unit_type TEXT NOT NULL CHECK (
    unit_type IN (
      'piece', 'box', 'carton', 'pack', 'bottle', 'can',
      'kilogram', 'gram', 'liter', 'milliliter', 'other'
    )
  ),
  description TEXT,
  is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
  created_at TEXT NOT NULL CHECK (
    length(created_at) = 27
    AND created_at GLOB
      '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9].[0-9][0-9][0-9][0-9][0-9][0-9]Z'
  ),
  updated_at TEXT NOT NULL CHECK (
    length(updated_at) = 27
    AND updated_at GLOB
      '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9].[0-9][0-9][0-9][0-9][0-9][0-9]Z'
  ),
  deleted_at TEXT CHECK (
    deleted_at IS NULL
    OR (
      length(deleted_at) = 27
      AND deleted_at GLOB
        '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9].[0-9][0-9][0-9][0-9][0-9][0-9]Z'
    )
  )
) STRICT;

CREATE UNIQUE INDEX units_active_type_unique
  ON units(unit_type) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX units_active_name_unique
  ON units(lower(name)) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX units_active_abbreviation_unique
  ON units(lower(abbreviation)) WHERE deleted_at IS NULL;

CREATE TABLE setting_overrides (
  id TEXT PRIMARY KEY
    CHECK (
      length(id) = 36
      AND substr(id, 9, 1) = '-'
      AND substr(id, 14, 1) = '-'
      AND substr(id, 19, 1) = '-'
      AND substr(id, 24, 1) = '-'
      AND id = lower(id)
      AND id NOT GLOB '*[^0-9a-f-]*'
    ),
  location_id TEXT
    CHECK (
      location_id IS NULL
      OR (
        length(location_id) = 36
        AND substr(location_id, 9, 1) = '-'
        AND substr(location_id, 14, 1) = '-'
        AND substr(location_id, 19, 1) = '-'
        AND substr(location_id, 24, 1) = '-'
        AND location_id = lower(location_id)
        AND location_id NOT GLOB '*[^0-9a-f-]*'
      )
    ),
  key TEXT NOT NULL CHECK (length(trim(key)) BETWEEN 1 AND 120),
  value TEXT NOT NULL CHECK (json_valid(value) = 1),
  revision INTEGER NOT NULL DEFAULT 1 CHECK (revision > 0),
  created_at TEXT NOT NULL CHECK (
    length(created_at) = 27
    AND created_at GLOB
      '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9].[0-9][0-9][0-9][0-9][0-9][0-9]Z'
  ),
  updated_at TEXT NOT NULL CHECK (
    length(updated_at) = 27
    AND updated_at GLOB
      '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9].[0-9][0-9][0-9][0-9][0-9][0-9]Z'
  )
) STRICT;

CREATE UNIQUE INDEX setting_overrides_scope_key_unique
  ON setting_overrides(COALESCE(location_id, ''), key);

CREATE TABLE access_roles (
  id TEXT PRIMARY KEY
    CHECK (
      length(id) = 36
      AND substr(id, 9, 1) = '-'
      AND substr(id, 14, 1) = '-'
      AND substr(id, 19, 1) = '-'
      AND substr(id, 24, 1) = '-'
      AND id = lower(id)
      AND id NOT GLOB '*[^0-9a-f-]*'
    ),
  system_key TEXT UNIQUE CHECK (
    system_key IS NULL OR length(trim(system_key)) BETWEEN 1 AND 80
  ),
  name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 80),
  description TEXT NOT NULL DEFAULT '',
  permissions TEXT NOT NULL CHECK (
    json_valid(permissions) = 1 AND json_type(permissions) = 'array'
  ),
  is_template INTEGER NOT NULL DEFAULT 0 CHECK (is_template IN (0, 1)),
  revision INTEGER NOT NULL DEFAULT 1 CHECK (revision > 0),
  created_at TEXT NOT NULL CHECK (
    length(created_at) = 27
    AND created_at GLOB
      '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9].[0-9][0-9][0-9][0-9][0-9][0-9]Z'
  ),
  updated_at TEXT NOT NULL CHECK (
    length(updated_at) = 27
    AND updated_at GLOB
      '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9].[0-9][0-9][0-9][0-9][0-9][0-9]Z'
  )
) STRICT;

CREATE UNIQUE INDEX access_roles_name_unique
  ON access_roles(lower(name), is_template);
