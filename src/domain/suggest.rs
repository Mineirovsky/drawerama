use std::collections::{BTreeSet, HashMap};

use crate::domain::model::{
    CategoryId, Centimeters, DrawerId, FurnitureId, GroupId, Item, ItemId, Placement, RoomId,
    Snapshot,
};
use crate::domain::space::{HOST_MIN_FREE, SMALL_SHARE_MAX, volume_fits};

const EPS: f64 = 1e-6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assignment {
    pub item_id: ItemId,
    pub placement: Placement,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Move {
    pub item_id: ItemId,
    pub from: Placement,
    pub to: Placement,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Consolidation {
    pub moves: Vec<Move>,
    pub already_optimal: bool,
    /// Category items the plan could not give a container.
    pub not_assigned: Vec<ItemId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub primary: Vec<Assignment>,
    pub primary_unplaced: Vec<ItemId>,
    pub consolidation: Consolidation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Slot {
    Drawer(DrawerId),
    Shelf(FurnitureId),
}

#[derive(Clone, Copy)]
struct SlotInfo {
    slot: Slot,
    size: Centimeters,
    room_id: RoomId,
    cabinet_id: Option<FurnitureId>,
    furniture_id: FurnitureId,
    position: Option<i64>,
}

struct World<'a> {
    snapshot: &'a Snapshot,
    slots: Vec<SlotInfo>,
}

#[derive(Default, Clone)]
struct Occupancy {
    used: HashMap<Slot, f64>,
    categories: HashMap<Slot, BTreeSet<CategoryId>>,
}

struct Presence {
    category_slots: Vec<SlotInfo>,
    category_cabinets: BTreeSet<FurnitureId>,
    category_rooms: BTreeSet<RoomId>,
    group_furniture: BTreeSet<FurnitureId>,
    group_rooms: BTreeSet<RoomId>,
}

pub fn plan(snapshot: &Snapshot, batch: &[ItemId]) -> Suggestion {
    let world = World::new(snapshot);
    let batch = unique_batch(batch);
    let batch_set: BTreeSet<ItemId> = batch.iter().copied().collect();

    let mut primary_occ = Occupancy::default();
    for item in &snapshot.items {
        if item.placement != Placement::Unplaced && !batch_set.contains(&item.id) {
            primary_occ.add(item);
        }
    }

    let mut categories: Vec<CategoryId> = batch
        .iter()
        .filter_map(|id| snapshot.item(*id).map(|item| item.category_id))
        .collect();
    categories.sort();
    categories.dedup();
    categories.sort_by(|a, b| {
        let volume_a = batch_volume(snapshot, &batch, *a);
        let volume_b = batch_volume(snapshot, &batch, *b);
        volume_b
            .partial_cmp(&volume_a)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.cmp(b))
    });

    let mut primary = Vec::new();
    let mut primary_unplaced = Vec::new();
    for category_id in &categories {
        let items: Vec<&Item> = batch
            .iter()
            .filter_map(|id| snapshot.item(*id))
            .filter(|item| item.category_id == *category_id)
            .collect();
        let share_volume = in_play_volume(snapshot, &batch_set, *category_id);
        let has_existing = snapshot.items.iter().any(|item| {
            item.category_id == *category_id
                && item.placement != Placement::Unplaced
                && !batch_set.contains(&item.id)
        });
        let assigned = world.place_primary(
            &primary_occ,
            *category_id,
            &items,
            share_volume,
            has_existing,
        );
        let assigned_ids: BTreeSet<ItemId> = assigned.iter().map(|(id, _, _)| *id).collect();
        for (item_id, slot, note) in &assigned {
            if let Some(item) = snapshot.item(*item_id) {
                primary_occ.add_at(*slot, item.category_id, item.size.geometric_volume_cm3());
            }
            primary.push(Assignment {
                item_id: *item_id,
                placement: slot_placement(*slot),
                note: note.clone(),
            });
        }
        for item in &items {
            if !assigned_ids.contains(&item.id) {
                primary_unplaced.push(item.id);
            }
        }
    }

    let consolidation = world.consolidate(&categories);

    Suggestion {
        primary,
        primary_unplaced,
        consolidation,
    }
}

fn unique_batch(batch: &[ItemId]) -> Vec<ItemId> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for id in batch {
        if seen.insert(*id) {
            out.push(*id);
        }
    }
    out
}

fn batch_volume(snapshot: &Snapshot, batch: &[ItemId], category: CategoryId) -> f64 {
    batch
        .iter()
        .filter_map(|id| snapshot.item(*id))
        .filter(|item| item.category_id == category)
        .map(|item| item.size.geometric_volume_cm3())
        .sum()
}

fn in_play_volume(snapshot: &Snapshot, batch: &BTreeSet<ItemId>, category: CategoryId) -> f64 {
    snapshot
        .items
        .iter()
        .filter(|item| item.category_id == category)
        .filter(|item| batch.contains(&item.id) || item.placement != Placement::Unplaced)
        .map(|item| item.size.geometric_volume_cm3())
        .sum()
}

fn slot_placement(slot: Slot) -> Placement {
    match slot {
        Slot::Drawer(id) => Placement::Drawer(id),
        Slot::Shelf(id) => Placement::Shelf(id),
    }
}

impl Occupancy {
    fn add(&mut self, item: &Item) {
        let Some(slot) = slot_of(item.placement) else {
            return;
        };
        self.add_at(slot, item.category_id, item.size.geometric_volume_cm3());
    }

    fn add_at(&mut self, slot: Slot, category: CategoryId, volume: f64) {
        *self.used.entry(slot).or_insert(0.0) += volume;
        self.categories.entry(slot).or_default().insert(category);
    }

    fn used(&self, slot: Slot) -> f64 {
        self.used.get(&slot).copied().unwrap_or(0.0)
    }

    fn holds(&self, slot: Slot, category: CategoryId) -> bool {
        self.categories
            .get(&slot)
            .is_some_and(|categories| categories.contains(&category))
    }

    fn holds_other(&self, slot: Slot, category: CategoryId) -> bool {
        self.categories
            .get(&slot)
            .is_some_and(|categories| categories.iter().any(|id| *id != category))
    }
}

fn slot_of(placement: Placement) -> Option<Slot> {
    match placement {
        Placement::Unplaced => None,
        Placement::Drawer(id) => Some(Slot::Drawer(id)),
        Placement::Shelf(id) => Some(Slot::Shelf(id)),
    }
}

impl<'a> World<'a> {
    fn new(snapshot: &'a Snapshot) -> Self {
        let mut slots = Vec::new();
        for drawer in &snapshot.drawers {
            let Some(cabinet) = snapshot.furniture(drawer.cabinet_id) else {
                continue;
            };
            slots.push(SlotInfo {
                slot: Slot::Drawer(drawer.id),
                size: drawer.size,
                room_id: cabinet.room_id,
                cabinet_id: Some(cabinet.id),
                furniture_id: cabinet.id,
                position: Some(drawer.position),
            });
        }
        for furniture in &snapshot.furniture {
            if let crate::domain::model::FurnitureKind::Shelf(size) = furniture.kind {
                slots.push(SlotInfo {
                    slot: Slot::Shelf(furniture.id),
                    size,
                    room_id: furniture.room_id,
                    cabinet_id: None,
                    furniture_id: furniture.id,
                    position: None,
                });
            }
        }
        Self { snapshot, slots }
    }

    fn info(&self, slot: Slot) -> Option<SlotInfo> {
        self.slots.iter().copied().find(|info| info.slot == slot)
    }

    fn group_of(&self, category: CategoryId) -> Option<GroupId> {
        self.snapshot.group_of(category)
    }

    fn groups_in_cabinet(&self, occ: &Occupancy, cabinet: FurnitureId) -> BTreeSet<GroupId> {
        let mut groups = BTreeSet::new();
        for info in &self.slots {
            if info.cabinet_id != Some(cabinet) {
                continue;
            }
            if let Some(categories) = occ.categories.get(&info.slot) {
                for category in categories {
                    if let Some(group) = self.group_of(*category) {
                        groups.insert(group);
                    }
                }
            }
        }
        groups
    }

    fn groups_in_slot(&self, occ: &Occupancy, slot: Slot) -> BTreeSet<GroupId> {
        let mut groups = BTreeSet::new();
        if let Some(categories) = occ.categories.get(&slot) {
            for category in categories {
                if let Some(group) = self.group_of(*category) {
                    groups.insert(group);
                }
            }
        }
        groups
    }

    fn blocked(&self, occ: &Occupancy, info: SlotInfo, group: GroupId) -> bool {
        let groups = if let Some(cabinet) = info.cabinet_id {
            self.groups_in_cabinet(occ, cabinet)
        } else {
            self.groups_in_slot(occ, info.slot)
        };
        groups.iter().any(|existing| *existing != group)
    }

    fn presence(&self, occ: &Occupancy, category: CategoryId, group: GroupId) -> Presence {
        let mut presence = Presence {
            category_slots: Vec::new(),
            category_cabinets: BTreeSet::new(),
            category_rooms: BTreeSet::new(),
            group_furniture: BTreeSet::new(),
            group_rooms: BTreeSet::new(),
        };
        for info in &self.slots {
            let Some(categories) = occ.categories.get(&info.slot) else {
                continue;
            };
            let mut same_group = false;
            for id in categories {
                if *id == category {
                    presence.category_slots.push(*info);
                    presence.category_rooms.insert(info.room_id);
                    if let Some(cabinet) = info.cabinet_id {
                        presence.category_cabinets.insert(cabinet);
                    } else {
                        presence.category_cabinets.insert(info.furniture_id);
                    }
                }
                if self.group_of(*id) == Some(group) {
                    same_group = true;
                }
            }
            if same_group {
                presence.group_furniture.insert(info.furniture_id);
                presence.group_rooms.insert(info.room_id);
            }
        }
        presence
    }

    fn distance(info: SlotInfo, presence: &Presence) -> i64 {
        let Some(position) = info.position else {
            return 0;
        };
        let Some(cabinet) = info.cabinet_id else {
            return 0;
        };
        presence
            .category_slots
            .iter()
            .filter(|slot| slot.cabinet_id == Some(cabinet))
            .filter_map(|slot| slot.position)
            .map(|other| (other - position).abs())
            .min()
            .unwrap_or(0)
    }

    /// Lower tier is a better home. `None` means the container cannot be used
    /// as this category's own space.
    fn tier(
        &self,
        occ: &Occupancy,
        info: SlotInfo,
        category: CategoryId,
        group: GroupId,
        presence: &Presence,
    ) -> Option<(u8, i64)> {
        if self.blocked(occ, info, group) {
            return None;
        }
        if occ.holds(info.slot, category) {
            return Some((0, 0));
        }
        if occ.holds_other(info.slot, category) {
            return None;
        }
        if let Some(cabinet) = info.cabinet_id {
            if presence.category_cabinets.contains(&cabinet) {
                return Some((1, Self::distance(info, presence)));
            }
        }
        let holds_group = presence.group_furniture.contains(&info.furniture_id);
        let same_room = presence.group_rooms.contains(&info.room_id)
            || presence.category_rooms.contains(&info.room_id);
        if holds_group && same_room {
            return Some((2, 0));
        }
        if same_room {
            return Some((3, 0));
        }
        if holds_group {
            return Some((4, 0));
        }
        Some((5, 0))
    }

    fn items_fit(&self, occ: &Occupancy, info: SlotInfo, items: &[&Item]) -> bool {
        if items.is_empty() {
            return true;
        }
        let needed: f64 = items
            .iter()
            .map(|item| item.size.geometric_volume_cm3())
            .sum();
        if !volume_fits(info.size, occ.used(info.slot), needed) {
            return false;
        }
        items.iter().all(|item| info.size.fits(item.size))
    }

    fn dedicated<'b>(
        &self,
        occ: &Occupancy,
        category: CategoryId,
        group: GroupId,
        items: &[&'b Item],
    ) -> Vec<SlotInfo> {
        self.slots
            .iter()
            .copied()
            .filter(|info| {
                !self.blocked(occ, *info, group)
                    && !occ.holds_other(info.slot, category)
                    && self.items_fit(occ, *info, items)
            })
            .collect()
    }

    fn smallest_dedicated(&self, candidates: &[SlotInfo]) -> Option<SlotInfo> {
        candidates.iter().copied().min_by(|a, b| {
            volume_key(a.size.geometric_volume_cm3())
                .cmp(&volume_key(b.size.geometric_volume_cm3()))
                .then(a.slot.cmp(&b.slot))
        })
    }

    fn best_by_tier(
        &self,
        occ: &Occupancy,
        category: CategoryId,
        group: GroupId,
        presence: &Presence,
        items: &[&Item],
    ) -> Option<(SlotInfo, u8)> {
        let mut best: Option<(SlotInfo, u8, i64)> = None;
        for info in &self.slots {
            let Some((tier, distance)) = self.tier(occ, *info, category, group, presence) else {
                continue;
            };
            if !self.items_fit(occ, *info, items) {
                continue;
            }
            let replace = match best {
                None => true,
                Some((current, current_tier, current_distance)) => {
                    (
                        tier,
                        distance,
                        volume_key(info.size.geometric_volume_cm3()),
                        info.slot,
                    ) < (
                        current_tier,
                        current_distance,
                        volume_key(current.size.geometric_volume_cm3()),
                        current.slot,
                    )
                }
            };
            if replace {
                best = Some((*info, tier, distance));
            }
        }
        best.map(|(info, tier, _)| (info, tier))
    }

    fn best_host(
        &self,
        occ: &Occupancy,
        category: CategoryId,
        group: GroupId,
        items: &[&Item],
        share_volume: f64,
    ) -> Option<SlotInfo> {
        self.slots
            .iter()
            .copied()
            .filter(|info| self.host_ok(occ, *info, category, group, items, share_volume))
            .min_by(|a, b| {
                volume_key(a.size.geometric_volume_cm3())
                    .cmp(&volume_key(b.size.geometric_volume_cm3()))
                    .then(a.slot.cmp(&b.slot))
            })
    }

    fn host_ok(
        &self,
        occ: &Occupancy,
        info: SlotInfo,
        category: CategoryId,
        group: GroupId,
        items: &[&Item],
        share_volume: f64,
    ) -> bool {
        if self.blocked(occ, info, group) || !occ.holds_other(info.slot, category) {
            return false;
        }
        let usable = info.size.usable_volume_cm3();
        if usable <= EPS || share_volume / usable >= SMALL_SHARE_MAX {
            return false;
        }
        let free = (usable - occ.used(info.slot)) / usable;
        if free + 1e-9 < HOST_MIN_FREE {
            return false;
        }
        self.items_fit(occ, info, items)
    }

    /// Share only when the category would waste its own smallest container.
    fn maybe_host(
        &self,
        occ: &Occupancy,
        category: CategoryId,
        group: GroupId,
        items: &[&Item],
        share_volume: f64,
    ) -> Option<SlotInfo> {
        let host = self.best_host(occ, category, group, items, share_volume)?;
        let dedicated = self.dedicated(occ, category, group, items);
        if let Some(smallest) = self.smallest_dedicated(&dedicated) {
            let usable = smallest.size.usable_volume_cm3();
            if usable > EPS && share_volume / usable < SMALL_SHARE_MAX {
                Some(host)
            } else {
                None
            }
        } else {
            Some(host)
        }
    }

    fn place_primary(
        &self,
        occ: &Occupancy,
        category: CategoryId,
        items: &[&Item],
        share_volume: f64,
        has_existing: bool,
    ) -> Vec<(ItemId, Slot, String)> {
        let Some(group) = self.group_of(category) else {
            return Vec::new();
        };
        if items.is_empty() {
            return Vec::new();
        }
        let presence = self.presence(occ, category, group);
        if self.existing_fits_all(occ, category, items) {
            if let Some((info, _)) = self.best_by_tier(occ, category, group, &presence, items) {
                return assign_all(items, info.slot, "Joins other items of this category");
            }
        }
        if !has_existing {
            if let Some(host) = self.maybe_host(occ, category, group, items, share_volume) {
                return assign_all(
                    items,
                    host.slot,
                    "Shares this space with another category in the same group",
                );
            }
        }
        if let Some((info, tier)) = self.best_by_tier(occ, category, group, &presence, items) {
            let note = note_for_tier(tier, &presence);
            return assign_all(items, info.slot, note);
        }
        self.spill(occ, category, group, &presence, items)
    }

    fn existing_fits_all(&self, occ: &Occupancy, category: CategoryId, items: &[&Item]) -> bool {
        self.slots
            .iter()
            .any(|info| occ.holds(info.slot, category) && self.items_fit(occ, *info, items))
    }

    fn spill(
        &self,
        occ: &Occupancy,
        category: CategoryId,
        group: GroupId,
        presence: &Presence,
        items: &[&Item],
    ) -> Vec<(ItemId, Slot, String)> {
        let mut ordered: Vec<(SlotInfo, u8)> = self
            .slots
            .iter()
            .filter_map(|info| {
                self.tier(occ, *info, category, group, presence)
                    .map(|tier| (*info, tier.0))
            })
            .collect();
        ordered.sort_by(|a, b| {
            let distance_a = Self::distance(a.0, presence);
            let distance_b = Self::distance(b.0, presence);
            a.1.cmp(&b.1)
                .then(distance_a.cmp(&distance_b))
                .then(
                    volume_key(a.0.size.geometric_volume_cm3())
                        .cmp(&volume_key(b.0.size.geometric_volume_cm3())),
                )
                .then(a.0.slot.cmp(&b.0.slot))
        });
        let slots: Vec<Slot> = ordered.iter().map(|(info, _)| info.slot).collect();
        let notes: HashMap<Slot, String> = ordered
            .iter()
            .map(|(info, tier)| (info.slot, note_for_tier(*tier, presence).to_string()))
            .collect();
        first_fit(self, occ, items, &slots, false)
            .unwrap_or_default()
            .into_iter()
            .map(|(item_id, slot)| {
                let note = notes
                    .get(&slot)
                    .cloned()
                    .unwrap_or_else(|| "Placed in the next space that fits".to_string());
                (item_id, slot, note)
            })
            .collect()
    }

    fn consolidate(&self, categories: &[CategoryId]) -> Consolidation {
        let category_set: BTreeSet<CategoryId> = categories.iter().copied().collect();
        let mut occ = Occupancy::default();
        for item in &self.snapshot.items {
            if item.placement != Placement::Unplaced && !category_set.contains(&item.category_id) {
                occ.add(item);
            }
        }

        let mut ordered = categories.to_vec();
        ordered.sort_by(|a, b| {
            let volume_a = category_volume(self.snapshot, *a);
            let volume_b = category_volume(self.snapshot, *b);
            volume_b
                .partial_cmp(&volume_a)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.cmp(b))
        });

        let mut targets: HashMap<ItemId, (Placement, String)> = HashMap::new();
        for category in ordered {
            let mut items: Vec<&Item> = self
                .snapshot
                .items
                .iter()
                .filter(|item| item.category_id == category)
                .collect();
            items.sort_by(|a, b| {
                b.size
                    .geometric_volume_cm3()
                    .partial_cmp(&a.size.geometric_volume_cm3())
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(a.id.cmp(&b.id))
            });
            let anchor = anchor_room(self.snapshot, category);
            let assigned = self.place_consolidation(&occ, category, &items, anchor);
            for (item_id, slot, note) in &assigned {
                if let Some(item) = self.snapshot.item(*item_id) {
                    occ.add_at(*slot, item.category_id, item.size.geometric_volume_cm3());
                }
                targets.insert(*item_id, (slot_placement(*slot), note.clone()));
            }
        }

        let mut moves = Vec::new();
        let mut not_assigned = Vec::new();
        for item in &self.snapshot.items {
            if !category_set.contains(&item.category_id) {
                continue;
            }
            match targets.get(&item.id) {
                Some((placement, note)) if *placement != item.placement => {
                    moves.push(Move {
                        item_id: item.id,
                        from: item.placement,
                        to: *placement,
                        note: note.clone(),
                    });
                }
                Some(_) => {}
                None => not_assigned.push(item.id),
            }
        }
        moves.sort_by_key(|relocation| relocation.item_id);
        not_assigned.sort();
        let already_optimal = moves.is_empty() && not_assigned.is_empty();
        Consolidation {
            moves,
            already_optimal,
            not_assigned,
        }
    }

    fn place_consolidation(
        &self,
        occ: &Occupancy,
        category: CategoryId,
        items: &[&Item],
        anchor: Option<RoomId>,
    ) -> Vec<(ItemId, Slot, String)> {
        let Some(group) = self.group_of(category) else {
            return Vec::new();
        };
        if items.is_empty() {
            return Vec::new();
        }
        let share_volume: f64 = items
            .iter()
            .map(|item| item.size.geometric_volume_cm3())
            .sum();
        if let Some(host) = self.maybe_host(occ, category, group, items, share_volume) {
            return assign_all(
                items,
                host.slot,
                "Shares this space with another category in the same group",
            );
        }
        if let Some(room) = anchor {
            if let Some(assigned) = self.pack_owned(occ, category, group, items, Some(room)) {
                return assigned;
            }
        }
        if let Some(assigned) = self.pack_owned(occ, category, group, items, None) {
            return assigned;
        }
        let mut slots: Vec<SlotInfo> = self
            .slots
            .iter()
            .copied()
            .filter(|info| {
                !self.blocked(occ, *info, group) && !occ.holds_other(info.slot, category)
            })
            .collect();
        slots.sort_by(|a, b| {
            let anchor_a = anchor == Some(a.room_id);
            let anchor_b = anchor == Some(b.room_id);
            anchor_b
                .cmp(&anchor_a)
                .then(a.room_id.cmp(&b.room_id))
                .then(a.furniture_id.cmp(&b.furniture_id))
                .then(a.position.unwrap_or(0).cmp(&b.position.unwrap_or(0)))
                .then(a.slot.cmp(&b.slot))
        });
        let slot_ids: Vec<Slot> = slots.iter().map(|info| info.slot).collect();
        first_fit(self, occ, items, &slot_ids, false)
            .unwrap_or_default()
            .into_iter()
            .map(|(item_id, slot)| {
                (
                    item_id,
                    slot,
                    "Split where the category still fits".to_string(),
                )
            })
            .collect()
    }

    /// Own containers only: one drawer or shelf, then an adjacent drawer run,
    /// then more than one cabinet. `only_room` keeps the category in the room
    /// it already occupies when that room can hold it.
    fn pack_owned(
        &self,
        occ: &Occupancy,
        category: CategoryId,
        group: GroupId,
        items: &[&Item],
        only_room: Option<RoomId>,
    ) -> Option<Vec<(ItemId, Slot, String)>> {
        let in_room = |info: SlotInfo| only_room.is_none_or(|room| info.room_id == room);
        let dedicated: Vec<SlotInfo> = self
            .dedicated(occ, category, group, items)
            .into_iter()
            .filter(|info| in_room(*info))
            .collect();
        if let Some(slot) = self.smallest_dedicated(&dedicated) {
            return Some(assign_all(
                items,
                slot.slot,
                "Smallest space that fits this category",
            ));
        }
        if let Some(assigned) = self.best_run(occ, category, group, items, only_room) {
            return Some(assigned);
        }
        if let Some(room) = only_room {
            return self
                .pack_room(occ, category, group, items, room)
                .map(|(_, _, assigned)| with_room_note(self, assigned));
        }
        self.best_room(occ, category, group, items, None)
    }

    fn best_run(
        &self,
        occ: &Occupancy,
        category: CategoryId,
        group: GroupId,
        items: &[&Item],
        only_room: Option<RoomId>,
    ) -> Option<Vec<(ItemId, Slot, String)>> {
        let mut cabinets: BTreeSet<FurnitureId> = BTreeSet::new();
        for info in &self.slots {
            if only_room.is_some_and(|room| info.room_id != room) {
                continue;
            }
            if let Some(cabinet) = info.cabinet_id {
                cabinets.insert(cabinet);
            }
        }
        let mut best: Option<(i64, usize, FurnitureId, i64, Vec<(ItemId, Slot)>)> = None;
        for cabinet in cabinets {
            if self.cabinet_blocked(occ, cabinet, group) {
                continue;
            }
            let mut drawers: Vec<SlotInfo> = self
                .slots
                .iter()
                .copied()
                .filter(|info| {
                    info.cabinet_id == Some(cabinet)
                        && !occ.holds_other(info.slot, category)
                        && !self.blocked(occ, *info, group)
                })
                .collect();
            drawers.sort_by(|a, b| {
                a.position
                    .unwrap_or(0)
                    .cmp(&b.position.unwrap_or(0))
                    .then(a.slot.cmp(&b.slot))
            });
            for start in 0..drawers.len() {
                for end in (start + 1)..drawers.len() {
                    let first = drawers[start].position.unwrap_or(0);
                    let last = drawers[end].position.unwrap_or(0);
                    if last - first != (end - start) as i64 {
                        break;
                    }
                    let run = &drawers[start..=end];
                    let slots: Vec<Slot> = run.iter().map(|info| info.slot).collect();
                    let Some(assigned) = first_fit(self, occ, items, &slots, true) else {
                        continue;
                    };
                    let volume: f64 = run
                        .iter()
                        .map(|info| info.size.geometric_volume_cm3())
                        .sum();
                    let score = (volume_key(volume), run.len(), cabinet, first, assigned);
                    let better = match &best {
                        None => true,
                        Some(current) => {
                            (score.0, score.1, score.2, score.3)
                                < (current.0, current.1, current.2, current.3)
                        }
                    };
                    if better {
                        best = Some(score);
                    }
                }
            }
        }
        best.map(|(_, _, _, _, assigned)| {
            assigned
                .into_iter()
                .map(|(item_id, slot)| {
                    (
                        item_id,
                        slot,
                        "Adjacent drawers, because one drawer is not enough".to_string(),
                    )
                })
                .collect()
        })
    }

    fn cabinet_blocked(&self, occ: &Occupancy, cabinet: FurnitureId, group: GroupId) -> bool {
        self.groups_in_cabinet(occ, cabinet)
            .iter()
            .any(|existing| *existing != group)
    }

    fn best_room(
        &self,
        occ: &Occupancy,
        category: CategoryId,
        group: GroupId,
        items: &[&Item],
        anchor: Option<RoomId>,
    ) -> Option<Vec<(ItemId, Slot, String)>> {
        let mut rooms: BTreeSet<RoomId> = self.slots.iter().map(|info| info.room_id).collect();
        if let Some(anchor) = anchor {
            if rooms.contains(&anchor) {
                if let Some((_, _, assigned)) = self.pack_room(occ, category, group, items, anchor)
                {
                    return Some(with_room_note(self, assigned));
                }
                rooms.remove(&anchor);
            }
        }
        let mut best: Option<(usize, i64, RoomId, Vec<(ItemId, Slot)>)> = None;
        for room in rooms {
            let Some((units, volume, assigned)) = self.pack_room(occ, category, group, items, room)
            else {
                continue;
            };
            let score = (units, volume, room, assigned);
            let better = match &best {
                None => true,
                Some(current) => (score.0, score.1, score.2) < (current.0, current.1, current.2),
            };
            if better {
                best = Some(score);
            }
        }
        best.map(|(_, _, _, assigned)| with_room_note(self, assigned))
    }

    fn pack_room(
        &self,
        occ: &Occupancy,
        category: CategoryId,
        group: GroupId,
        items: &[&Item],
        room: RoomId,
    ) -> Option<(usize, i64, Vec<(ItemId, Slot)>)> {
        let units = self.units_in(occ, category, group, room);
        if units.is_empty() || units.len() > 12 {
            return self.pack_units_greedy(occ, items, &units);
        }
        let mut best: Option<(usize, i64, Vec<(ItemId, Slot)>)> = None;
        let n = units.len();
        for mask in 1u32..(1u32 << n) {
            let subset: Vec<&Unit> = units
                .iter()
                .enumerate()
                .filter(|(index, _)| mask & (1u32 << index) != 0)
                .map(|(_, unit)| unit)
                .collect();
            let Some(assigned) = pack_units(self, occ, items, &subset) else {
                continue;
            };
            let volume = used_volume(self, &assigned);
            let score = (subset.len(), volume, assigned);
            let better = match &best {
                None => true,
                Some(current) => (score.0, score.1) < (current.0, current.1),
            };
            if better {
                best = Some(score);
            }
        }
        best
    }

    fn pack_units_greedy(
        &self,
        occ: &Occupancy,
        items: &[&Item],
        units: &[Unit],
    ) -> Option<(usize, i64, Vec<(ItemId, Slot)>)> {
        let refs: Vec<&Unit> = units.iter().collect();
        let assigned = pack_units(self, occ, items, &refs)?;
        Some((units.len(), used_volume(self, &assigned), assigned))
    }

    fn units_in(
        &self,
        occ: &Occupancy,
        category: CategoryId,
        group: GroupId,
        room: RoomId,
    ) -> Vec<Unit> {
        let mut units = Vec::new();
        for furniture in self.snapshot.furniture_in(room) {
            match furniture.kind {
                crate::domain::model::FurnitureKind::Cabinet => {
                    if self.cabinet_blocked(occ, furniture.id, group) {
                        continue;
                    }
                    let drawers: Vec<Slot> = self
                        .snapshot
                        .drawers_in(furniture.id)
                        .into_iter()
                        .filter(|drawer| {
                            let slot = Slot::Drawer(drawer.id);
                            !occ.holds_other(slot, category)
                        })
                        .map(|drawer| Slot::Drawer(drawer.id))
                        .collect();
                    if !drawers.is_empty() {
                        units.push(Unit { slots: drawers });
                    }
                }
                crate::domain::model::FurnitureKind::Shelf(_) => {
                    let slot = Slot::Shelf(furniture.id);
                    let Some(info) = self.info(slot) else {
                        continue;
                    };
                    if self.blocked(occ, info, group) || occ.holds_other(slot, category) {
                        continue;
                    }
                    units.push(Unit { slots: vec![slot] });
                }
            }
        }
        units
    }
}

struct Unit {
    slots: Vec<Slot>,
}

fn pack_units(
    world: &World,
    occ: &Occupancy,
    items: &[&Item],
    units: &[&Unit],
) -> Option<Vec<(ItemId, Slot)>> {
    let mut slots = Vec::new();
    for unit in units {
        slots.extend(unit.slots.iter().copied());
    }
    first_fit(world, occ, items, &slots, true)
}

fn used_volume(world: &World, assigned: &[(ItemId, Slot)]) -> i64 {
    let mut slots = BTreeSet::new();
    for (_, slot) in assigned {
        slots.insert(*slot);
    }
    let volume: f64 = slots
        .into_iter()
        .filter_map(|slot| world.info(slot))
        .map(|info| info.size.geometric_volume_cm3())
        .sum();
    volume_key(volume)
}

fn with_room_note(world: &World, assigned: Vec<(ItemId, Slot)>) -> Vec<(ItemId, Slot, String)> {
    let cabinets: BTreeSet<_> = assigned
        .iter()
        .filter_map(|(_, slot)| world.info(*slot).and_then(|info| info.cabinet_id))
        .collect();
    let shelves = assigned
        .iter()
        .any(|(_, slot)| matches!(slot, Slot::Shelf(_)));
    let note = if cabinets.len() + usize::from(shelves) <= 1 {
        "Drawers in the same cabinet"
    } else {
        "Split across furniture in the same room"
    };
    assigned
        .into_iter()
        .map(|(item_id, slot)| (item_id, slot, note.to_string()))
        .collect()
}

fn category_volume(snapshot: &Snapshot, category: CategoryId) -> f64 {
    snapshot
        .items
        .iter()
        .filter(|item| item.category_id == category)
        .map(|item| item.size.geometric_volume_cm3())
        .sum()
}

fn anchor_room(snapshot: &Snapshot, category: CategoryId) -> Option<RoomId> {
    let mut counts: HashMap<RoomId, usize> = HashMap::new();
    for item in &snapshot.items {
        if item.category_id != category {
            continue;
        }
        let room = match item.placement {
            Placement::Drawer(id) => snapshot
                .drawer(id)
                .and_then(|drawer| snapshot.furniture(drawer.cabinet_id))
                .map(|cabinet| cabinet.room_id),
            Placement::Shelf(id) => snapshot.furniture(id).map(|shelf| shelf.room_id),
            Placement::Unplaced => None,
        };
        if let Some(room) = room {
            *counts.entry(room).or_insert(0) += 1;
        }
    }
    counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
        .map(|(room, _)| room)
}

fn assign_all(items: &[&Item], slot: Slot, note: &str) -> Vec<(ItemId, Slot, String)> {
    items
        .iter()
        .map(|item| (item.id, slot, note.to_string()))
        .collect()
}

fn note_for_tier(tier: u8, presence: &Presence) -> &'static str {
    let anchored = !presence.group_rooms.is_empty() || !presence.category_rooms.is_empty();
    match tier {
        0 => "Joins other items of this category",
        1 => "Adjacent drawer in the same cabinet",
        2 => "Same cabinet as this group",
        3 => "Empty space in the same room",
        4 => "Same group in another room",
        _ if anchored => "Another room",
        _ => "Smallest space that fits",
    }
}

fn volume_key(volume: f64) -> i64 {
    (volume * 1000.0).round() as i64
}

fn first_fit(
    world: &World,
    occ: &Occupancy,
    items: &[&Item],
    slots: &[Slot],
    require_all: bool,
) -> Option<Vec<(ItemId, Slot)>> {
    let mut used = occ.used.clone();
    let mut sorted: Vec<&Item> = items.to_vec();
    sorted.sort_by(|a, b| {
        b.size
            .geometric_volume_cm3()
            .partial_cmp(&a.size.geometric_volume_cm3())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.id.cmp(&b.id))
    });
    let mut placed = Vec::new();
    for item in sorted {
        let mut found = None;
        for slot in slots {
            let Some(info) = world.info(*slot) else {
                continue;
            };
            let already = used.get(slot).copied().unwrap_or(0.0);
            if volume_fits(info.size, already, item.size.geometric_volume_cm3())
                && info.size.fits(item.size)
            {
                found = Some(*slot);
                break;
            }
        }
        if let Some(slot) = found {
            *used.entry(slot).or_insert(0.0) += item.size.geometric_volume_cm3();
            placed.push((item.id, slot));
        } else if require_all {
            return None;
        }
    }
    if require_all && placed.len() != items.len() {
        None
    } else {
        Some(placed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::{Category, CategoryGroup, Drawer, Furniture, FurnitureKind, Room};

    struct House {
        snap: Snapshot,
        next: i64,
    }

    impl House {
        fn new() -> Self {
            Self {
                snap: Snapshot::default(),
                next: 1,
            }
        }

        fn id(&mut self) -> i64 {
            let id = self.next;
            self.next += 1;
            id
        }

        fn room(&mut self, name: &str) -> RoomId {
            let id = RoomId(self.id());
            self.snap.rooms.push(Room {
                id,
                name: name.to_string(),
            });
            id
        }

        fn cabinet(&mut self, room: RoomId, name: &str) -> FurnitureId {
            let id = FurnitureId(self.id());
            self.snap.furniture.push(Furniture {
                id,
                room_id: room,
                name: name.to_string(),
                kind: FurnitureKind::Cabinet,
                sort_order: self.snap.furniture.len() as i64,
            });
            id
        }

        fn drawer(&mut self, cabinet: FurnitureId, name: &str, size: Centimeters) -> DrawerId {
            let position = self
                .snap
                .drawers
                .iter()
                .filter(|drawer| drawer.cabinet_id == cabinet)
                .count() as i64;
            let id = DrawerId(self.id());
            self.snap.drawers.push(Drawer {
                id,
                cabinet_id: cabinet,
                name: name.to_string(),
                size,
                position,
            });
            id
        }

        fn group(&mut self, name: &str) -> GroupId {
            let id = GroupId(self.id());
            self.snap.groups.push(CategoryGroup {
                id,
                name: name.to_string(),
            });
            id
        }

        fn category(&mut self, group: GroupId, name: &str) -> CategoryId {
            let id = CategoryId(self.id());
            self.snap.categories.push(Category {
                id,
                group_id: group,
                name: name.to_string(),
            });
            id
        }

        fn item(
            &mut self,
            name: &str,
            category: CategoryId,
            size: Centimeters,
            placement: Placement,
        ) -> ItemId {
            let id = ItemId(self.id());
            self.snap.items.push(Item {
                id,
                name: name.to_string(),
                category_id: category,
                size,
                placement,
            });
            id
        }
    }

    fn cm(width: f64, height: f64, depth: f64) -> Centimeters {
        Centimeters {
            width,
            height,
            depth,
        }
    }

    fn cube(side: f64) -> Centimeters {
        cm(side, side, side)
    }

    fn placement_of(suggestion: &Suggestion, id: ItemId) -> Option<Placement> {
        suggestion
            .primary
            .iter()
            .find(|assignment| assignment.item_id == id)
            .map(|assignment| assignment.placement)
    }

    fn drawer_of(placement: Placement) -> DrawerId {
        match placement {
            Placement::Drawer(id) => id,
            other => panic!("expected a drawer, got {other:?}"),
        }
    }

    #[test]
    fn chemicals_block_a_textile_and_shirts_may_share_a_cabinet_with_blankets() {
        let mut house = House::new();
        let room = house.room("Hall");
        let chemicals = house.cabinet(room, "Cleaning");
        let textiles = house.cabinet(room, "Linen");
        let chemical_drawer = house.drawer(chemicals, "Bottom", cube(20.0));
        let shirt_drawer = house.drawer(textiles, "Shirts", cube(20.0));
        let blanket_drawer = house.drawer(textiles, "Blankets", cube(20.0));
        let other_room = house.room("Spare");
        let elsewhere = house.cabinet(other_room, "Spare cabinet");
        let smaller = house.drawer(elsewhere, "Small", cube(12.0));

        let chemical_group = house.group("Chemicals");
        let textile_group = house.group("Textiles");
        let cleaners = house.category(chemical_group, "Cleaners");
        let shirts = house.category(textile_group, "Shirts");
        let blankets = house.category(textile_group, "Blankets");

        house.item(
            "Bleach",
            cleaners,
            cm(10.0, 10.0, 8.0),
            Placement::Drawer(chemical_drawer),
        );
        house.item(
            "Blue shirt",
            shirts,
            cm(10.0, 8.0, 4.0),
            Placement::Drawer(shirt_drawer),
        );
        let blanket = house.item(
            "Wool blanket",
            blankets,
            cm(12.0, 10.0, 4.0),
            Placement::Unplaced,
        );

        let before = house.snap.clone();
        let suggestion = plan(&house.snap, &[blanket]);
        assert_eq!(house.snap, before, "planning does not apply the result");
        assert_eq!(
            drawer_of(placement_of(&suggestion, blanket).unwrap()),
            blanket_drawer
        );
        assert_ne!(
            drawer_of(placement_of(&suggestion, blanket).unwrap()),
            smaller
        );
        assert_ne!(
            drawer_of(placement_of(&suggestion, blanket).unwrap()),
            chemical_drawer
        );
    }

    #[test]
    fn a_full_drawer_yields_the_adjacent_drawer_before_another_room() {
        let mut house = House::new();
        let room = house.room("Bedroom");
        let cabinet = house.cabinet(room, "Dresser");
        let full = house.drawer(cabinet, "Top", cube(10.0));
        let adjacent = house.drawer(cabinet, "Middle", cube(16.0));
        let other = house.room("Attic");
        let attic = house.cabinet(other, "Attic chest");
        let smaller = house.drawer(attic, "Small", cube(12.0));

        let textiles = house.group("Textiles");
        let shirts = house.category(textiles, "Shirts");
        house.item(
            "Stack of shirts",
            shirts,
            cm(9.0, 8.0, 8.0),
            Placement::Drawer(full),
        );
        let extra = house.item(
            "One more shirt",
            shirts,
            cm(6.0, 5.0, 4.0),
            Placement::Unplaced,
        );

        let suggestion = plan(&house.snap, &[extra]);
        let chosen = drawer_of(placement_of(&suggestion, extra).unwrap());
        assert_eq!(chosen, adjacent);
        assert_ne!(chosen, smaller);
        assert_ne!(chosen, full);
    }

    #[test]
    fn consolidation_uses_the_smallest_adjacent_run() {
        let mut house = House::new();
        let room = house.room("Bedroom");
        let small = house.cabinet(room, "Small dresser");
        let large = house.cabinet(room, "Large dresser");
        let left = house.drawer(small, "Left", cube(10.0));
        let right = house.drawer(small, "Right", cube(10.0));
        let big_left = house.drawer(large, "Left", cm(12.0, 10.0, 10.0));
        let big_right = house.drawer(large, "Right", cm(12.0, 10.0, 10.0));

        let textiles = house.group("Textiles");
        let blankets = house.category(textiles, "Blankets");
        let first = house.item(
            "Blanket A",
            blankets,
            cm(8.0, 5.0, 10.0),
            Placement::Unplaced,
        );
        let second = house.item(
            "Blanket B",
            blankets,
            cm(8.0, 5.0, 10.0),
            Placement::Unplaced,
        );

        let suggestion = plan(&house.snap, &[first, second]);
        let targets: BTreeSet<_> = suggestion
            .consolidation
            .moves
            .iter()
            .map(|relocation| relocation.to)
            .collect();
        assert_eq!(
            targets,
            BTreeSet::from([Placement::Drawer(left), Placement::Drawer(right)])
        );
        assert!(!targets.contains(&Placement::Drawer(big_left)));
        assert!(!targets.contains(&Placement::Drawer(big_right)));
    }

    #[test]
    fn consolidation_stays_in_the_same_room_when_the_cabinet_runs_out() {
        let mut house = House::new();
        let bedroom = house.room("Bedroom");
        let attic = house.room("Attic");
        let crowded = house.cabinet(bedroom, "Crowded");
        let extra = house.cabinet(bedroom, "Extra");
        let far = house.cabinet(attic, "Attic");
        let only = house.drawer(crowded, "Only", cube(10.0));
        let extra_a = house.drawer(extra, "A", cube(10.0));
        let extra_b = house.drawer(extra, "B", cube(10.0));
        let perfect = house.drawer(far, "Perfect", cube(20.0));

        let textiles = house.group("Textiles");
        let blankets = house.category(textiles, "Blankets");
        let kept = house.item(
            "Kept blanket",
            blankets,
            cm(8.0, 5.0, 10.0),
            Placement::Drawer(only),
        );
        let loose = house.item(
            "Loose blanket",
            blankets,
            cm(8.0, 5.0, 10.0),
            Placement::Unplaced,
        );

        let suggestion = plan(&house.snap, &[loose]);
        let places: Vec<_> = suggestion
            .consolidation
            .moves
            .iter()
            .map(|relocation| relocation.to)
            .collect();
        let allowed = BTreeSet::from([Placement::Drawer(extra_a), Placement::Drawer(extra_b)]);
        assert!(
            places.iter().all(|place| allowed.contains(place)),
            "consolidation left the bedroom: {places:?}"
        );
        assert!(!places.contains(&Placement::Drawer(perfect)));
        assert!(!places.contains(&Placement::Drawer(only)));
        let _ = kept;
    }

    #[test]
    fn a_small_category_joins_a_half_empty_sibling_drawer() {
        let mut house = House::new();
        let room = house.room("Bath");
        let cabinet = house.cabinet(room, "Linen");
        let host = house.drawer(cabinet, "Towels", cube(20.0));
        let empty = house.drawer(cabinet, "Empty", cube(20.0));
        let textiles = house.group("Textiles");
        let towels = house.category(textiles, "Towels");
        let cloths = house.category(textiles, "Cloths");
        house.item(
            "Towel stack",
            towels,
            cm(12.0, 10.0, 8.0),
            Placement::Drawer(host),
        );
        let cloth = house.item("Washcloth", cloths, cm(4.0, 4.0, 2.0), Placement::Unplaced);

        let suggestion = plan(&house.snap, &[cloth]);
        assert_eq!(
            placement_of(&suggestion, cloth),
            Some(Placement::Drawer(host))
        );
        assert_ne!(
            placement_of(&suggestion, cloth),
            Some(Placement::Drawer(empty))
        );
    }

    #[test]
    fn a_category_that_fills_its_own_small_drawer_keeps_it() {
        let mut house = House::new();
        let room = house.room("Bath");
        let cabinet = house.cabinet(room, "Linen");
        let small = house.drawer(cabinet, "Small", cube(10.0));
        let host = house.drawer(cabinet, "Large", cube(30.0));
        let textiles = house.group("Textiles");
        let towels = house.category(textiles, "Towels");
        let cloths = house.category(textiles, "Cloths");
        house.item(
            "Few towels",
            towels,
            cm(10.0, 10.0, 6.0),
            Placement::Drawer(host),
        );
        let cloth = house.item(
            "Folded cloth",
            cloths,
            cm(8.0, 5.0, 5.0),
            Placement::Unplaced,
        );

        let suggestion = plan(&house.snap, &[cloth]);
        assert_eq!(
            placement_of(&suggestion, cloth),
            Some(Placement::Drawer(small))
        );
    }

    #[test]
    fn a_small_category_avoids_a_full_sibling_and_another_group() {
        let mut house = House::new();
        let room = house.room("Kitchen");
        let cabinet = house.cabinet(room, "Supplies");
        let full = house.drawer(cabinet, "Towels", cube(10.0));
        let empty = house.drawer(cabinet, "Empty", cube(10.0));
        let chemicals = house.cabinet(room, "Chemicals");
        let chemical = house.drawer(chemicals, "Cleaners", cube(10.0));
        let textiles = house.group("Textiles");
        let chemicals_group = house.group("Chemicals");
        let towels = house.category(textiles, "Towels");
        let cloths = house.category(textiles, "Cloths");
        let cleaners = house.category(chemicals_group, "Cleaners");
        house.item("Towels", towels, cm(8.0, 8.0, 6.0), Placement::Drawer(full));
        house.item(
            "Bleach",
            cleaners,
            cm(4.0, 4.0, 4.0),
            Placement::Drawer(chemical),
        );
        let cloth = house.item("Cloth", cloths, cm(3.0, 3.0, 2.0), Placement::Unplaced);

        let suggestion = plan(&house.snap, &[cloth]);
        assert_eq!(
            placement_of(&suggestion, cloth),
            Some(Placement::Drawer(empty))
        );
    }

    #[test]
    fn two_items_are_not_both_offered_the_last_gap() {
        let mut house = House::new();
        let room = house.room("Hall");
        let cabinet = house.cabinet(room, "Dresser");
        let only = house.drawer(cabinet, "Only", cube(10.0));
        let textiles = house.group("Textiles");
        let shirts = house.category(textiles, "Shirts");
        let blankets = house.category(textiles, "Blankets");
        let shirt = house.item("Shirt", shirts, cm(8.0, 5.0, 10.0), Placement::Unplaced);
        let blanket = house.item("Blanket", blankets, cm(8.0, 5.0, 10.0), Placement::Unplaced);

        let suggestion = plan(&house.snap, &[shirt, blanket]);
        let placed = suggestion
            .primary
            .iter()
            .filter(|assignment| assignment.placement == Placement::Drawer(only))
            .count();
        assert_eq!(placed, 1);
        assert_eq!(suggestion.primary_unplaced.len(), 1);
    }

    #[test]
    fn a_textile_and_a_chemical_in_one_batch_use_different_cabinets() {
        let mut house = House::new();
        let room = house.room("Hall");
        let linen = house.cabinet(room, "Linen");
        let cleaning = house.cabinet(room, "Cleaning");
        let linen_drawer = house.drawer(linen, "Drawer", cube(20.0));
        let cleaning_drawer = house.drawer(cleaning, "Drawer", cube(20.0));
        let textiles = house.group("Textiles");
        let chemicals = house.group("Chemicals");
        let shirts = house.category(textiles, "Shirts");
        let cleaners = house.category(chemicals, "Cleaners");
        let shirt = house.item("Shirt", shirts, cm(10.0, 8.0, 4.0), Placement::Unplaced);
        let bleach = house.item("Bleach", cleaners, cm(8.0, 8.0, 8.0), Placement::Unplaced);

        let suggestion = plan(&house.snap, &[shirt, bleach]);
        let shirt_drawer = drawer_of(placement_of(&suggestion, shirt).unwrap());
        let bleach_drawer = drawer_of(placement_of(&suggestion, bleach).unwrap());
        let shirt_cabinet = house.snap.drawer(shirt_drawer).unwrap().cabinet_id;
        let bleach_cabinet = house.snap.drawer(bleach_drawer).unwrap().cabinet_id;
        assert_ne!(shirt_cabinet, bleach_cabinet);
        assert!(
            (shirt_drawer == linen_drawer && bleach_drawer == cleaning_drawer)
                || (shirt_drawer == cleaning_drawer && bleach_drawer == linen_drawer)
        );
    }

    #[test]
    fn unchecked_unplaced_items_stay_out_of_the_primary_plan() {
        let mut house = House::new();
        let room = house.room("Hall");
        let cabinet = house.cabinet(room, "Dresser");
        let drawer = house.drawer(cabinet, "Drawer", cube(30.0));
        let textiles = house.group("Textiles");
        let shirts = house.category(textiles, "Shirts");
        let first = house.item("First", shirts, cm(6.0, 4.0, 2.0), Placement::Unplaced);
        let second = house.item("Second", shirts, cm(6.0, 4.0, 2.0), Placement::Unplaced);
        let third = house.item("Third", shirts, cm(6.0, 4.0, 2.0), Placement::Unplaced);

        let suggestion = plan(&house.snap, &[first, second]);
        let primary_ids: BTreeSet<_> = suggestion
            .primary
            .iter()
            .map(|assignment| assignment.item_id)
            .collect();
        assert_eq!(primary_ids, BTreeSet::from([first, second]));
        assert!(!suggestion.primary_unplaced.contains(&third));
        assert!(
            suggestion
                .consolidation
                .moves
                .iter()
                .any(|relocation| relocation.item_id == third
                    && relocation.to == Placement::Drawer(drawer))
        );
    }
}
