use std::path::Path;

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, params};

use crate::domain::{
    Category, CategoryGroup, CategoryId, Centimeters, Drawer, DrawerId, Furniture, FurnitureId,
    FurnitureKind, GroupId, Item, ItemId, Placement, Room, RoomId, Snapshot,
};
use crate::storage::db;

pub struct SaveItem {
    pub id: Option<ItemId>,
    pub name: String,
    pub category_id: CategoryId,
    pub size: Centimeters,
    pub placement: Placement,
}

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        Ok(Self {
            conn: db::open(path)?,
        })
    }

    pub fn load(&self) -> Result<Snapshot> {
        Ok(Snapshot {
            rooms: self.rooms()?,
            furniture: self.furniture()?,
            drawers: self.drawers()?,
            groups: self.groups()?,
            categories: self.categories()?,
            items: self.items()?,
        })
    }

    pub fn create_room(&mut self, name: &str) -> Result<RoomId> {
        let name = clean_name(name)?;
        self.ensure_room_name(&name, None)?;
        self.conn
            .execute("INSERT INTO room (name) VALUES (?1)", params![name])?;
        Ok(RoomId(self.conn.last_insert_rowid()))
    }

    pub fn rename_room(&mut self, id: RoomId, name: &str) -> Result<()> {
        let name = clean_name(name)?;
        self.ensure_room_name(&name, Some(id))?;
        let changed = self.conn.execute(
            "UPDATE room SET name = ?1 WHERE id = ?2",
            params![name, id.0],
        )?;
        if changed == 0 {
            bail!("Room not found");
        }
        Ok(())
    }

    pub fn delete_room(&mut self, id: RoomId) -> Result<()> {
        self.conn
            .execute("DELETE FROM room WHERE id = ?1", params![id.0])?;
        Ok(())
    }

    pub fn create_cabinet(&mut self, room_id: RoomId, name: &str) -> Result<FurnitureId> {
        let name = clean_name(name)?;
        self.ensure_room(room_id)?;
        self.ensure_furniture_name(room_id, &name, None)?;
        let sort_order = self.next_sort_order(room_id)?;
        self.conn.execute(
            "INSERT INTO furniture (room_id, name, kind, width_cm, height_cm, depth_cm, sort_order)
             VALUES (?1, ?2, 'cabinet', NULL, NULL, NULL, ?3)",
            params![room_id.0, name, sort_order],
        )?;
        Ok(FurnitureId(self.conn.last_insert_rowid()))
    }

    pub fn create_shelf(
        &mut self,
        room_id: RoomId,
        name: &str,
        size: Centimeters,
    ) -> Result<FurnitureId> {
        let name = clean_name(name)?;
        let size = require_size(size)?;
        self.ensure_room(room_id)?;
        self.ensure_furniture_name(room_id, &name, None)?;
        let sort_order = self.next_sort_order(room_id)?;
        self.conn.execute(
            "INSERT INTO furniture (room_id, name, kind, width_cm, height_cm, depth_cm, sort_order)
             VALUES (?1, ?2, 'shelf', ?3, ?4, ?5, ?6)",
            params![
                room_id.0,
                name,
                size.width,
                size.height,
                size.depth,
                sort_order
            ],
        )?;
        Ok(FurnitureId(self.conn.last_insert_rowid()))
    }

    pub fn rename_furniture(&mut self, id: FurnitureId, name: &str) -> Result<()> {
        let name = clean_name(name)?;
        let room_id = self.furniture_room(id)?;
        self.ensure_furniture_name(room_id, &name, Some(id))?;
        self.conn.execute(
            "UPDATE furniture SET name = ?1 WHERE id = ?2",
            params![name, id.0],
        )?;
        Ok(())
    }

    pub fn update_shelf_size(&mut self, id: FurnitureId, size: Centimeters) -> Result<()> {
        let size = require_size(size)?;
        self.ensure_kind(id, "shelf")?;
        self.conn.execute(
            "UPDATE furniture SET width_cm = ?1, height_cm = ?2, depth_cm = ?3 WHERE id = ?4",
            params![size.width, size.height, size.depth, id.0],
        )?;
        Ok(())
    }

    pub fn delete_furniture(&mut self, id: FurnitureId) -> Result<()> {
        self.conn
            .execute("DELETE FROM furniture WHERE id = ?1", params![id.0])?;
        Ok(())
    }

    pub fn create_drawer(
        &mut self,
        cabinet_id: FurnitureId,
        name: &str,
        size: Centimeters,
    ) -> Result<DrawerId> {
        let name = clean_name(name)?;
        let size = require_size(size)?;
        self.ensure_kind(cabinet_id, "cabinet")?;
        self.ensure_drawer_name(cabinet_id, &name, None)?;
        let position = self.next_position(cabinet_id)?;
        self.conn.execute(
            "INSERT INTO drawer (cabinet_id, name, width_cm, height_cm, depth_cm, position)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                cabinet_id.0,
                name,
                size.width,
                size.height,
                size.depth,
                position
            ],
        )?;
        Ok(DrawerId(self.conn.last_insert_rowid()))
    }

    pub fn update_drawer(&mut self, id: DrawerId, name: &str, size: Centimeters) -> Result<()> {
        let name = clean_name(name)?;
        let size = require_size(size)?;
        let cabinet_id = self.drawer_cabinet(id)?;
        self.ensure_drawer_name(cabinet_id, &name, Some(id))?;
        self.conn.execute(
            "UPDATE drawer SET name = ?1, width_cm = ?2, height_cm = ?3, depth_cm = ?4 WHERE id = ?5",
            params![name, size.width, size.height, size.depth, id.0],
        )?;
        Ok(())
    }

    pub fn move_drawer(&mut self, id: DrawerId, direction: i32) -> Result<()> {
        if direction != -1 && direction != 1 {
            bail!("Drawer move direction must be -1 or 1");
        }
        let cabinet_id = self.drawer_cabinet(id)?;
        let mut drawers = self.drawers_of(cabinet_id)?;
        drawers.sort_by_key(|drawer| drawer.1);
        let Some(index) = drawers.iter().position(|(drawer_id, _)| *drawer_id == id) else {
            bail!("Drawer not found");
        };
        let other = index as isize + direction as isize;
        if other < 0 || other >= drawers.len() as isize {
            return Ok(());
        }
        let (other_id, other_pos) = drawers[other as usize];
        let (_, pos) = drawers[index];
        let tx = self.conn.transaction()?;
        tx.execute(
            "UPDATE drawer SET position = -1 WHERE id = ?1",
            params![id.0],
        )?;
        tx.execute(
            "UPDATE drawer SET position = ?1 WHERE id = ?2",
            params![pos, other_id.0],
        )?;
        tx.execute(
            "UPDATE drawer SET position = ?1 WHERE id = ?2",
            params![other_pos, id.0],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn delete_drawer(&mut self, id: DrawerId) -> Result<()> {
        self.conn
            .execute("DELETE FROM drawer WHERE id = ?1", params![id.0])?;
        Ok(())
    }

    pub fn create_group(&mut self, name: &str) -> Result<GroupId> {
        let name = clean_name(name)?;
        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM category_group WHERE name = ?1)",
            params![name],
            |row| row.get(0),
        )?;
        if exists {
            bail!("A group named \"{name}\" already exists");
        }
        self.conn.execute(
            "INSERT INTO category_group (name) VALUES (?1)",
            params![name],
        )?;
        Ok(GroupId(self.conn.last_insert_rowid()))
    }

    pub fn delete_group(&mut self, id: GroupId) -> Result<()> {
        let used: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM category WHERE group_id = ?1)",
            params![id.0],
            |row| row.get(0),
        )?;
        if used {
            bail!("This group still has categories");
        }
        let changed = self
            .conn
            .execute("DELETE FROM category_group WHERE id = ?1", params![id.0])?;
        if changed == 0 {
            bail!("Group not found");
        }
        Ok(())
    }

    pub fn create_category(&mut self, group_id: GroupId, name: &str) -> Result<CategoryId> {
        let name = clean_name(name)?;
        self.ensure_group(group_id)?;
        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM category WHERE group_id = ?1 AND name = ?2)",
            params![group_id.0, name],
            |row| row.get(0),
        )?;
        if exists {
            bail!("That category already exists in this group");
        }
        self.conn.execute(
            "INSERT INTO category (group_id, name) VALUES (?1, ?2)",
            params![group_id.0, name],
        )?;
        Ok(CategoryId(self.conn.last_insert_rowid()))
    }

    pub fn delete_category(&mut self, id: CategoryId) -> Result<()> {
        let used: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM item WHERE category_id = ?1)",
            params![id.0],
            |row| row.get(0),
        )?;
        if used {
            bail!("This category still has items");
        }
        let changed = self
            .conn
            .execute("DELETE FROM category WHERE id = ?1", params![id.0])?;
        if changed == 0 {
            bail!("Category not found");
        }
        Ok(())
    }

    pub fn save_item(&mut self, item: SaveItem) -> Result<ItemId> {
        let name = clean_name(&item.name)?;
        let size = require_size(item.size)?;
        self.ensure_category(item.category_id)?;
        self.ensure_placement(item.placement)?;
        let (drawer_id, shelf_id) = placement_ids(item.placement);
        if let Some(id) = item.id {
            let changed = self.conn.execute(
                "UPDATE item
                 SET name = ?1, category_id = ?2, width_cm = ?3, height_cm = ?4, depth_cm = ?5,
                     drawer_id = ?6, shelf_id = ?7
                 WHERE id = ?8",
                params![
                    name,
                    item.category_id.0,
                    size.width,
                    size.height,
                    size.depth,
                    drawer_id,
                    shelf_id,
                    id.0
                ],
            )?;
            if changed == 0 {
                bail!("Item not found");
            }
            Ok(id)
        } else {
            self.conn.execute(
                "INSERT INTO item (name, category_id, width_cm, height_cm, depth_cm, drawer_id, shelf_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    name,
                    item.category_id.0,
                    size.width,
                    size.height,
                    size.depth,
                    drawer_id,
                    shelf_id
                ],
            )?;
            Ok(ItemId(self.conn.last_insert_rowid()))
        }
    }

    pub fn delete_item(&mut self, id: ItemId) -> Result<()> {
        self.conn
            .execute("DELETE FROM item WHERE id = ?1", params![id.0])?;
        Ok(())
    }

    pub fn apply_placements(&mut self, placements: &[(ItemId, Placement)]) -> Result<()> {
        let tx = self.conn.transaction()?;
        for (id, placement) in placements {
            ensure_placement_tx(&tx, *placement)?;
            let (drawer_id, shelf_id) = placement_ids(*placement);
            let changed = tx.execute(
                "UPDATE item SET drawer_id = ?1, shelf_id = ?2 WHERE id = ?3",
                params![drawer_id, shelf_id, id.0],
            )?;
            if changed == 0 {
                bail!("Item {} not found", id.0);
            }
        }
        tx.commit()?;
        Ok(())
    }

    fn rooms(&self) -> Result<Vec<Room>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name FROM room ORDER BY name, id")?;
        let rows = stmt.query_map([], |row| {
            Ok(Room {
                id: RoomId(row.get(0)?),
                name: row.get(1)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("could not read rooms")
    }

    fn furniture(&self) -> Result<Vec<Furniture>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, room_id, name, kind, width_cm, height_cm, depth_cm, sort_order
             FROM furniture ORDER BY sort_order, name, id",
        )?;
        let mut rows = stmt.query([])?;
        let mut furniture = Vec::new();
        while let Some(row) = rows.next()? {
            let kind_name: String = row.get(3)?;
            let width: Option<f64> = row.get(4)?;
            let height: Option<f64> = row.get(5)?;
            let depth: Option<f64> = row.get(6)?;
            let kind = match kind_name.as_str() {
                "cabinet" => FurnitureKind::Cabinet,
                "shelf" => FurnitureKind::Shelf(Centimeters {
                    width: width.unwrap_or(0.0),
                    height: height.unwrap_or(0.0),
                    depth: depth.unwrap_or(0.0),
                }),
                other => bail!("unknown furniture kind {other}"),
            };
            furniture.push(Furniture {
                id: FurnitureId(row.get(0)?),
                room_id: RoomId(row.get(1)?),
                name: row.get(2)?,
                kind,
                sort_order: row.get(7)?,
            });
        }
        Ok(furniture)
    }

    fn drawers(&self) -> Result<Vec<Drawer>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, cabinet_id, name, width_cm, height_cm, depth_cm, position
             FROM drawer ORDER BY position, id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Drawer {
                id: DrawerId(row.get(0)?),
                cabinet_id: FurnitureId(row.get(1)?),
                name: row.get(2)?,
                size: Centimeters {
                    width: row.get(3)?,
                    height: row.get(4)?,
                    depth: row.get(5)?,
                },
                position: row.get(6)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("could not read drawers")
    }

    fn groups(&self) -> Result<Vec<CategoryGroup>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name FROM category_group ORDER BY name, id")?;
        let rows = stmt.query_map([], |row| {
            Ok(CategoryGroup {
                id: GroupId(row.get(0)?),
                name: row.get(1)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("could not read groups")
    }

    fn categories(&self) -> Result<Vec<Category>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, group_id, name FROM category ORDER BY name, id")?;
        let rows = stmt.query_map([], |row| {
            Ok(Category {
                id: CategoryId(row.get(0)?),
                group_id: GroupId(row.get(1)?),
                name: row.get(2)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("could not read categories")
    }

    fn items(&self) -> Result<Vec<Item>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, category_id, width_cm, height_cm, depth_cm, drawer_id, shelf_id
             FROM item ORDER BY name, id",
        )?;
        let mut rows = stmt.query([])?;
        let mut items = Vec::new();
        while let Some(row) = rows.next()? {
            let id = ItemId(row.get(0)?);
            let drawer_id: Option<i64> = row.get(6)?;
            let shelf_id: Option<i64> = row.get(7)?;
            let placement = match (drawer_id, shelf_id) {
                (Some(drawer), None) => Placement::Drawer(DrawerId(drawer)),
                (None, Some(shelf)) => Placement::Shelf(FurnitureId(shelf)),
                (None, None) => Placement::Unplaced,
                (Some(_), Some(_)) => bail!("item {} is in a drawer and on a shelf", id.0),
            };
            items.push(Item {
                id,
                name: row.get(1)?,
                category_id: CategoryId(row.get(2)?),
                size: Centimeters {
                    width: row.get(3)?,
                    height: row.get(4)?,
                    depth: row.get(5)?,
                },
                placement,
            });
        }
        Ok(items)
    }

    fn ensure_room(&self, id: RoomId) -> Result<()> {
        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM room WHERE id = ?1)",
            params![id.0],
            |row| row.get(0),
        )?;
        if !exists {
            bail!("Room not found");
        }
        Ok(())
    }

    fn ensure_group(&self, id: GroupId) -> Result<()> {
        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM category_group WHERE id = ?1)",
            params![id.0],
            |row| row.get(0),
        )?;
        if !exists {
            bail!("Group not found");
        }
        Ok(())
    }

    fn ensure_category(&self, id: CategoryId) -> Result<()> {
        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM category WHERE id = ?1)",
            params![id.0],
            |row| row.get(0),
        )?;
        if !exists {
            bail!("Category not found");
        }
        Ok(())
    }

    fn ensure_kind(&self, id: FurnitureId, expected: &str) -> Result<()> {
        let kind: String = self
            .conn
            .query_row(
                "SELECT kind FROM furniture WHERE id = ?1",
                params![id.0],
                |row| row.get(0),
            )
            .context("Furniture not found")?;
        if kind != expected {
            bail!(
                "{}",
                if expected == "cabinet" {
                    "Drawers belong in a cabinet"
                } else {
                    "That furniture is not a shelf"
                }
            );
        }
        Ok(())
    }

    fn ensure_room_name(&self, name: &str, except: Option<RoomId>) -> Result<()> {
        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM room WHERE name = ?1 AND id != ?2)",
            params![name, except.map(|id| id.0).unwrap_or(-1)],
            |row| row.get(0),
        )?;
        if exists {
            bail!("A room named \"{name}\" already exists");
        }
        Ok(())
    }

    fn ensure_furniture_name(
        &self,
        room_id: RoomId,
        name: &str,
        except: Option<FurnitureId>,
    ) -> Result<()> {
        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM furniture WHERE room_id = ?1 AND name = ?2 AND id != ?3)",
            params![room_id.0, name, except.map(|id| id.0).unwrap_or(-1)],
            |row| row.get(0),
        )?;
        if exists {
            bail!("This room already has furniture named \"{name}\"");
        }
        Ok(())
    }

    fn ensure_drawer_name(
        &self,
        cabinet_id: FurnitureId,
        name: &str,
        except: Option<DrawerId>,
    ) -> Result<()> {
        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM drawer WHERE cabinet_id = ?1 AND name = ?2 AND id != ?3)",
            params![cabinet_id.0, name, except.map(|id| id.0).unwrap_or(-1)],
            |row| row.get(0),
        )?;
        if exists {
            bail!("This cabinet already has a drawer named \"{name}\"");
        }
        Ok(())
    }

    fn ensure_placement(&self, placement: Placement) -> Result<()> {
        ensure_placement_tx(&self.conn, placement)
    }

    fn furniture_room(&self, id: FurnitureId) -> Result<RoomId> {
        let room: i64 = self
            .conn
            .query_row(
                "SELECT room_id FROM furniture WHERE id = ?1",
                params![id.0],
                |row| row.get(0),
            )
            .context("Furniture not found")?;
        Ok(RoomId(room))
    }

    fn drawer_cabinet(&self, id: DrawerId) -> Result<FurnitureId> {
        let cabinet: i64 = self
            .conn
            .query_row(
                "SELECT cabinet_id FROM drawer WHERE id = ?1",
                params![id.0],
                |row| row.get(0),
            )
            .context("Drawer not found")?;
        Ok(FurnitureId(cabinet))
    }

    fn next_sort_order(&self, room_id: RoomId) -> Result<i64> {
        let max: Option<i64> = self.conn.query_row(
            "SELECT MAX(sort_order) FROM furniture WHERE room_id = ?1",
            params![room_id.0],
            |row| row.get(0),
        )?;
        Ok(max.unwrap_or(-1) + 1)
    }

    fn next_position(&self, cabinet_id: FurnitureId) -> Result<i64> {
        let max: Option<i64> = self.conn.query_row(
            "SELECT MAX(position) FROM drawer WHERE cabinet_id = ?1",
            params![cabinet_id.0],
            |row| row.get(0),
        )?;
        Ok(max.unwrap_or(-1) + 1)
    }

    fn drawers_of(&self, cabinet_id: FurnitureId) -> Result<Vec<(DrawerId, i64)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, position FROM drawer WHERE cabinet_id = ?1 ORDER BY position")?;
        let rows = stmt.query_map(params![cabinet_id.0], |row| {
            Ok((DrawerId(row.get(0)?), row.get(1)?))
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("could not read drawer order")
    }
}

fn ensure_placement_tx(conn: &Connection, placement: Placement) -> Result<()> {
    match placement {
        Placement::Unplaced => Ok(()),
        Placement::Drawer(id) => {
            let exists: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM drawer WHERE id = ?1)",
                params![id.0],
                |row| row.get(0),
            )?;
            if !exists {
                bail!("Drawer not found");
            }
            Ok(())
        }
        Placement::Shelf(id) => {
            let kind: String = conn
                .query_row(
                    "SELECT kind FROM furniture WHERE id = ?1",
                    params![id.0],
                    |row| row.get(0),
                )
                .context("Shelf not found")?;
            if kind != "shelf" {
                bail!("Items sit on a shelf or in a drawer");
            }
            Ok(())
        }
    }
}

fn placement_ids(placement: Placement) -> (Option<i64>, Option<i64>) {
    match placement {
        Placement::Unplaced => (None, None),
        Placement::Drawer(id) => (Some(id.0), None),
        Placement::Shelf(id) => (None, Some(id.0)),
    }
}

fn clean_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        bail!("Name is required");
    }
    Ok(name.to_string())
}

fn require_size(size: Centimeters) -> Result<Centimeters> {
    for (label, value) in [
        ("Width", size.width),
        ("Height", size.height),
        ("Depth", size.depth),
    ] {
        if !value.is_finite() || value <= 0.0 {
            bail!("{label} must be a positive number of centimeters");
        }
    }
    Ok(size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Placement;

    fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("test.db")).unwrap();
        (dir, store)
    }

    fn size() -> Centimeters {
        Centimeters {
            width: 30.0,
            height: 15.0,
            depth: 40.0,
        }
    }

    #[test]
    fn delete_drawer_shelf_and_room_leave_items_unplaced() {
        let (_dir, mut store) = store();
        let room = store.create_room("Kitchen").unwrap();
        let cabinet = store.create_cabinet(room, "Pans").unwrap();
        let drawer = store.create_drawer(cabinet, "Top", size()).unwrap();
        let shelf = store.create_shelf(room, "Open shelf", size()).unwrap();
        let group = store.create_group("Kitchenware").unwrap();
        let category = store.create_category(group, "Pots").unwrap();
        let in_drawer = store
            .save_item(SaveItem {
                id: None,
                name: "Saucepan".into(),
                category_id: category,
                size: size(),
                placement: Placement::Drawer(drawer),
            })
            .unwrap();
        let on_shelf = store
            .save_item(SaveItem {
                id: None,
                name: "Bowl".into(),
                category_id: category,
                size: size(),
                placement: Placement::Shelf(shelf),
            })
            .unwrap();

        store.delete_drawer(drawer).unwrap();
        let snap = store.load().unwrap();
        assert_eq!(snap.item(in_drawer).unwrap().placement, Placement::Unplaced);

        store.delete_furniture(shelf).unwrap();
        let snap = store.load().unwrap();
        assert_eq!(snap.item(on_shelf).unwrap().placement, Placement::Unplaced);

        let other = store.create_cabinet(room, "Other").unwrap();
        let other_drawer = store.create_drawer(other, "Bottom", size()).unwrap();
        store
            .save_item(SaveItem {
                id: Some(in_drawer),
                name: "Saucepan".into(),
                category_id: category,
                size: size(),
                placement: Placement::Drawer(other_drawer),
            })
            .unwrap();
        store.delete_room(room).unwrap();
        let snap = store.load().unwrap();
        assert!(snap.rooms.is_empty());
        assert!(snap.furniture.is_empty());
        assert!(snap.drawers.is_empty());
        assert_eq!(snap.item(in_drawer).unwrap().placement, Placement::Unplaced);
        assert_eq!(snap.item(on_shelf).unwrap().placement, Placement::Unplaced);
    }

    #[test]
    fn category_delete_is_refused_while_an_item_uses_it() {
        let (_dir, mut store) = store();
        let group = store.create_group("Textiles").unwrap();
        let category = store.create_category(group, "Shirts").unwrap();
        store
            .save_item(SaveItem {
                id: None,
                name: "Shirt".into(),
                category_id: category,
                size: size(),
                placement: Placement::Unplaced,
            })
            .unwrap();
        assert!(store.delete_category(category).is_err());
        assert!(store.delete_group(group).is_err());
    }

    #[test]
    fn a_drawer_cannot_be_added_to_a_shelf_and_a_cabinet_has_no_dimensions() {
        let (_dir, mut store) = store();
        let room = store.create_room("Hall").unwrap();
        let shelf = store.create_shelf(room, "Shelf", size()).unwrap();
        assert!(store.create_drawer(shelf, "Nope", size()).is_err());
        let cabinet = store.create_cabinet(room, "Cabinet").unwrap();
        let snap = store.load().unwrap();
        assert!(matches!(
            snap.furniture(cabinet).unwrap().kind,
            FurnitureKind::Cabinet
        ));
        let group = store.create_group("Misc").unwrap();
        let bits = store.create_category(group, "Bits").unwrap();
        assert!(
            store
                .save_item(SaveItem {
                    id: None,
                    name: "Odd".into(),
                    category_id: bits,
                    size: size(),
                    placement: Placement::Shelf(cabinet),
                })
                .is_err()
        );
    }

    #[test]
    fn placements_apply_in_one_transaction() {
        let (_dir, mut store) = store();
        let room = store.create_room("Hall").unwrap();
        let cabinet = store.create_cabinet(room, "Dresser").unwrap();
        let first = store.create_drawer(cabinet, "Top", size()).unwrap();
        let second = store.create_drawer(cabinet, "Bottom", size()).unwrap();
        store.move_drawer(second, -1).unwrap();
        let snap = store.load().unwrap();
        let drawers = snap.drawers_in(cabinet);
        assert_eq!(drawers[0].id, second);
        assert_eq!(drawers[1].id, first);

        let group = store.create_group("Textiles").unwrap();
        let category = store.create_category(group, "Shirts").unwrap();
        let item = store
            .save_item(SaveItem {
                id: None,
                name: "Shirt".into(),
                category_id: category,
                size: size(),
                placement: Placement::Unplaced,
            })
            .unwrap();
        store
            .apply_placements(&[(item, Placement::Drawer(first))])
            .unwrap();
        assert_eq!(
            store.load().unwrap().item(item).unwrap().placement,
            Placement::Drawer(first)
        );
    }
}
