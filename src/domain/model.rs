#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RoomId(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FurnitureId(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DrawerId(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GroupId(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CategoryId(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ItemId(pub i64);

/// Width, height, and depth in centimeters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Centimeters {
    pub width: f64,
    pub height: f64,
    pub depth: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Room {
    pub id: RoomId,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FurnitureKind {
    Cabinet,
    Shelf(Centimeters),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Furniture {
    pub id: FurnitureId,
    pub room_id: RoomId,
    pub name: String,
    pub kind: FurnitureKind,
    pub sort_order: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Drawer {
    pub id: DrawerId,
    pub cabinet_id: FurnitureId,
    pub name: String,
    pub size: Centimeters,
    pub position: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CategoryGroup {
    pub id: GroupId,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Category {
    pub id: CategoryId,
    pub group_id: GroupId,
    pub name: String,
}

/// An item sits in one drawer, on one shelf, or nowhere yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Placement {
    Unplaced,
    Drawer(DrawerId),
    Shelf(FurnitureId),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub id: ItemId,
    pub name: String,
    pub category_id: CategoryId,
    pub size: Centimeters,
    pub placement: Placement,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    pub rooms: Vec<Room>,
    pub furniture: Vec<Furniture>,
    pub drawers: Vec<Drawer>,
    pub groups: Vec<CategoryGroup>,
    pub categories: Vec<Category>,
    pub items: Vec<Item>,
}

impl Snapshot {
    pub fn room(&self, id: RoomId) -> Option<&Room> {
        self.rooms.iter().find(|room| room.id == id)
    }

    pub fn furniture(&self, id: FurnitureId) -> Option<&Furniture> {
        self.furniture.iter().find(|furniture| furniture.id == id)
    }

    pub fn drawer(&self, id: DrawerId) -> Option<&Drawer> {
        self.drawers.iter().find(|drawer| drawer.id == id)
    }

    pub fn group(&self, id: GroupId) -> Option<&CategoryGroup> {
        self.groups.iter().find(|group| group.id == id)
    }

    pub fn category(&self, id: CategoryId) -> Option<&Category> {
        self.categories.iter().find(|category| category.id == id)
    }

    pub fn item(&self, id: ItemId) -> Option<&Item> {
        self.items.iter().find(|item| item.id == id)
    }

    pub fn group_of(&self, category_id: CategoryId) -> Option<GroupId> {
        self.category(category_id).map(|category| category.group_id)
    }

    pub fn furniture_in(&self, room_id: RoomId) -> Vec<&Furniture> {
        let mut found: Vec<_> = self
            .furniture
            .iter()
            .filter(|furniture| furniture.room_id == room_id)
            .collect();
        found.sort_by(|a, b| {
            a.sort_order
                .cmp(&b.sort_order)
                .then(a.name.cmp(&b.name))
                .then(a.id.cmp(&b.id))
        });
        found
    }

    pub fn drawers_in(&self, cabinet_id: FurnitureId) -> Vec<&Drawer> {
        let mut found: Vec<_> = self
            .drawers
            .iter()
            .filter(|drawer| drawer.cabinet_id == cabinet_id)
            .collect();
        found.sort_by(|a, b| a.position.cmp(&b.position).then(a.id.cmp(&b.id)));
        found
    }

    pub fn items_in_drawer(&self, drawer_id: DrawerId) -> Vec<&Item> {
        self.items
            .iter()
            .filter(|item| item.placement == Placement::Drawer(drawer_id))
            .collect()
    }

    pub fn items_on_shelf(&self, shelf_id: FurnitureId) -> Vec<&Item> {
        self.items
            .iter()
            .filter(|item| item.placement == Placement::Shelf(shelf_id))
            .collect()
    }

    pub fn describe(&self, placement: Placement) -> String {
        match placement {
            Placement::Unplaced => "Unplaced".to_string(),
            Placement::Drawer(id) => {
                let Some(drawer) = self.drawer(id) else {
                    return "Missing drawer".to_string();
                };
                let Some(cabinet) = self.furniture(drawer.cabinet_id) else {
                    return drawer.name.clone();
                };
                let Some(room) = self.room(cabinet.room_id) else {
                    return format!("{} / {}", cabinet.name, drawer.name);
                };
                format!("{} / {} / {}", room.name, cabinet.name, drawer.name)
            }
            Placement::Shelf(id) => {
                let Some(shelf) = self.furniture(id) else {
                    return "Missing shelf".to_string();
                };
                let Some(room) = self.room(shelf.room_id) else {
                    return shelf.name.clone();
                };
                format!("{} / {}", room.name, shelf.name)
            }
        }
    }
}
