CREATE TABLE room (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE
);

CREATE TABLE category_group (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE
);

CREATE TABLE category (
    id INTEGER PRIMARY KEY,
    group_id INTEGER NOT NULL REFERENCES category_group(id),
    name TEXT NOT NULL,
    UNIQUE(group_id, name)
);

CREATE TABLE furniture (
    id INTEGER PRIMARY KEY,
    room_id INTEGER NOT NULL REFERENCES room(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    kind TEXT NOT NULL CHECK(kind IN ('cabinet', 'shelf')),
    width_cm REAL,
    height_cm REAL,
    depth_cm REAL,
    sort_order INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE drawer (
    id INTEGER PRIMARY KEY,
    cabinet_id INTEGER NOT NULL REFERENCES furniture(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    width_cm REAL NOT NULL,
    height_cm REAL NOT NULL,
    depth_cm REAL NOT NULL,
    position INTEGER NOT NULL,
    UNIQUE(cabinet_id, position),
    UNIQUE(cabinet_id, name)
);

CREATE TABLE item (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    category_id INTEGER NOT NULL REFERENCES category(id),
    width_cm REAL NOT NULL,
    height_cm REAL NOT NULL,
    depth_cm REAL NOT NULL,
    drawer_id INTEGER REFERENCES drawer(id) ON DELETE SET NULL,
    shelf_id INTEGER REFERENCES furniture(id) ON DELETE SET NULL,
    CHECK (drawer_id IS NULL OR shelf_id IS NULL)
);

CREATE INDEX idx_furniture_room ON furniture(room_id);
CREATE INDEX idx_drawer_cabinet ON drawer(cabinet_id);
CREATE INDEX idx_category_group ON category(group_id);
CREATE INDEX idx_item_category ON item(category_id);
CREATE INDEX idx_item_drawer ON item(drawer_id);
CREATE INDEX idx_item_shelf ON item(shelf_id);
