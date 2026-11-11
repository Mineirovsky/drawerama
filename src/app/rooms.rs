use eframe::egui;

use drawerama::domain::{DrawerId, FurnitureId, FurnitureKind, Placement, RoomId};

use super::table::Column;
use super::{DimDraft, DraweramaApp, Screen, dim_controls, table, usage_bar};

impl DraweramaApp {
    pub(super) fn ui_rooms(&mut self, ui: &mut egui::Ui) {
        ui.heading("Rooms");
        ui.label("A room holds cabinets and shelves.");
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.draft_room);
            if ui.button("Add room").clicked() {
                let name = self.draft_room.clone();
                let result = self.store.create_room(&name);
                self.act(result, "Room added");
                if self.error.is_none() {
                    self.draft_room.clear();
                }
            }
        });
        ui.add_space(8.0);
        if self.snapshot.rooms.is_empty() {
            ui.label("No rooms yet.");
            return;
        }
        let rooms = self.snapshot.rooms.clone();
        table::show(
            ui,
            "rooms",
            &[Column::new("Name"), Column::new("Actions")],
            rooms.len(),
            |ui, row, col| {
                let room = &rooms[row];
                match col {
                    0 => {
                        ui.label(&room.name);
                    }
                    1 => {
                        ui.horizontal(|ui| {
                            if ui.button("Open").clicked() {
                                self.screen = Screen::Room(room.id);
                            }
                            if ui.button("Delete").clicked() {
                                let result = self.store.delete_room(room.id);
                                self.act(result, "Room deleted. Items inside are now unplaced.");
                            }
                        });
                    }
                    _ => {}
                }
            },
        );
    }

    pub(super) fn ui_room(&mut self, ui: &mut egui::Ui, room_id: RoomId) {
        let Some(room) = self.snapshot.room(room_id).cloned() else {
            missing(ui, "That room no longer exists.");
            return;
        };
        if ui.button("Back to rooms").clicked() {
            self.screen = Screen::Rooms;
        }
        ui.heading(&room.name);
        if self.room_draft.as_ref().map(|(id, _)| *id) != Some(room_id) {
            self.room_draft = Some((room_id, room.name.clone()));
        }
        if let Some((_, name)) = self.room_draft.as_mut() {
            ui.horizontal(|ui| {
                ui.label("Name");
                ui.text_edit_singleline(name);
            });
        }
        if ui.button("Save name").clicked() {
            if let Some((_, name)) = self.room_draft.clone() {
                let result = self.store.rename_room(room_id, &name);
                self.act(result, "Room renamed");
            }
        }

        ui.add_space(8.0);
        ui.label("Cabinets hold drawers. A shelf holds items directly.");
        let furniture = self
            .snapshot
            .furniture_in(room_id)
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        if furniture.is_empty() {
            ui.label("Nothing in this room yet.");
        } else {
            table::show(
                ui,
                "furniture",
                &[
                    Column::new("Name"),
                    Column::new("Kind"),
                    Column::new("Actions"),
                ],
                furniture.len(),
                |ui, row, col| {
                    let piece = &furniture[row];
                    let kind = match piece.kind {
                        FurnitureKind::Cabinet => "Cabinet",
                        FurnitureKind::Shelf(_) => "Shelf",
                    };
                    match col {
                        0 => {
                            ui.label(&piece.name);
                        }
                        1 => {
                            ui.label(kind);
                        }
                        2 => {
                            ui.horizontal(|ui| {
                                if ui.button("Open").clicked() {
                                    self.screen = match piece.kind {
                                        FurnitureKind::Cabinet => Screen::Cabinet(piece.id),
                                        FurnitureKind::Shelf(_) => Screen::Shelf(piece.id),
                                    };
                                }
                                if ui.button("Delete").clicked() {
                                    let result = self.store.delete_furniture(piece.id);
                                    self.act(result, "Removed. Items inside are now unplaced.");
                                }
                            });
                        }
                        _ => {}
                    }
                },
            );
        }

        ui.add_space(12.0);
        ui.separator();
        ui.label("Add furniture");
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.draft_kind_shelf, false, "Cabinet");
            ui.radio_value(&mut self.draft_kind_shelf, true, "Shelf");
        });
        ui.horizontal(|ui| {
            ui.label("Name");
            ui.text_edit_singleline(&mut self.draft_furniture_name);
        });
        if self.draft_kind_shelf {
            dim_controls(ui, &mut self.draft_shelf_dims);
        }
        if ui.button("Add").clicked() {
            let name = self.draft_furniture_name.clone();
            let result = if self.draft_kind_shelf {
                self.store
                    .create_shelf(room_id, &name, self.draft_shelf_dims.cm())
                    .map(|_| ())
            } else {
                self.store.create_cabinet(room_id, &name).map(|_| ())
            };
            self.act(result, "Furniture added");
            if self.error.is_none() {
                self.draft_furniture_name.clear();
            }
        }
    }

    pub(super) fn ui_cabinet(&mut self, ui: &mut egui::Ui, cabinet_id: FurnitureId) {
        let Some(cabinet) = self.snapshot.furniture(cabinet_id).cloned() else {
            missing(ui, "That cabinet no longer exists.");
            return;
        };
        if !matches!(cabinet.kind, FurnitureKind::Cabinet) {
            missing(ui, "That is not a cabinet.");
            return;
        }
        if ui.button("Back to room").clicked() {
            self.screen = Screen::Room(cabinet.room_id);
        }
        ui.heading(&cabinet.name);
        self.furniture_name_editor(ui, cabinet_id, &cabinet.name);

        ui.add_space(8.0);
        ui.label("Drawer order is physical adjacency. The top of the list is the first drawer.");
        let drawers = self
            .snapshot
            .drawers_in(cabinet_id)
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        if drawers.is_empty() {
            ui.label("No drawers yet.");
        } else {
            let last = drawers.len().saturating_sub(1);
            table::show(
                ui,
                "drawers",
                &[
                    Column::new("Name"),
                    Column::new("Order"),
                    Column::new("Size"),
                    Column::new("Actions"),
                ],
                drawers.len(),
                |ui, row, col| {
                    let drawer = &drawers[row];
                    match col {
                        0 => {
                            ui.label(&drawer.name);
                        }
                        1 => {
                            ui.label((drawer.position + 1).to_string());
                        }
                        2 => {
                            let size = drawer.size;
                            ui.label(format!(
                                "{:.0}×{:.0}×{:.0} cm",
                                size.width, size.height, size.depth
                            ));
                        }
                        3 => {
                            ui.horizontal(|ui| {
                                if ui.button("Open").clicked() {
                                    self.screen = Screen::Drawer(drawer.id);
                                }
                                if ui.add_enabled(row > 0, egui::Button::new("Up")).clicked() {
                                    let result = self.store.move_drawer(drawer.id, -1);
                                    self.act(result, "Drawer moved");
                                }
                                if ui
                                    .add_enabled(row < last, egui::Button::new("Down"))
                                    .clicked()
                                {
                                    let result = self.store.move_drawer(drawer.id, 1);
                                    self.act(result, "Drawer moved");
                                }
                                if ui.button("Delete").clicked() {
                                    let result = self.store.delete_drawer(drawer.id);
                                    self.act(result, "Drawer deleted. Its items are now unplaced.");
                                }
                            });
                        }
                        _ => {}
                    }
                },
            );
        }

        ui.add_space(12.0);
        ui.separator();
        ui.label("Add drawer");
        ui.horizontal(|ui| {
            ui.label("Name");
            ui.text_edit_singleline(&mut self.draft_drawer_name);
        });
        dim_controls(ui, &mut self.draft_drawer_dims);
        if ui.button("Add drawer").clicked() {
            let name = self.draft_drawer_name.clone();
            let result = self
                .store
                .create_drawer(cabinet_id, &name, self.draft_drawer_dims.cm());
            self.act(result, "Drawer added");
            if self.error.is_none() {
                self.draft_drawer_name.clear();
            }
        }
    }

    pub(super) fn ui_drawer(&mut self, ui: &mut egui::Ui, drawer_id: DrawerId) {
        let Some(drawer) = self.snapshot.drawer(drawer_id).cloned() else {
            missing(ui, "That drawer no longer exists.");
            return;
        };
        let cabinet_id = drawer.cabinet_id;
        if ui.button("Back to cabinet").clicked() {
            self.screen = Screen::Cabinet(cabinet_id);
        }
        ui.heading(&drawer.name);
        if self.drawer_draft.as_ref().map(|(id, _, _)| *id) != Some(drawer_id) {
            self.drawer_draft = Some((
                drawer_id,
                drawer.name.clone(),
                DimDraft::from_cm(drawer.size),
            ));
        }
        if let Some((_, name, dims)) = self.drawer_draft.as_mut() {
            ui.horizontal(|ui| {
                ui.label("Name");
                ui.text_edit_singleline(name);
            });
            dim_controls(ui, dims);
        }
        if ui.button("Save drawer").clicked() {
            if let Some((_, name, dims)) = self.drawer_draft.clone() {
                let result = self.store.update_drawer(drawer_id, &name, dims.cm());
                self.act(result, "Drawer saved");
            }
        }
        ui.add_space(8.0);
        let items: Vec<_> = self
            .snapshot
            .items_in_drawer(drawer_id)
            .into_iter()
            .cloned()
            .collect();
        let refs: Vec<_> = items.iter().collect();
        usage_bar(ui, drawer.size, &refs);
        self.item_list(ui, &items, Placement::Drawer(drawer_id));
    }

    pub(super) fn ui_shelf(&mut self, ui: &mut egui::Ui, shelf_id: FurnitureId) {
        let Some(shelf) = self.snapshot.furniture(shelf_id).cloned() else {
            missing(ui, "That shelf no longer exists.");
            return;
        };
        let FurnitureKind::Shelf(size) = shelf.kind else {
            missing(ui, "That is not a shelf.");
            return;
        };
        if ui.button("Back to room").clicked() {
            self.screen = Screen::Room(shelf.room_id);
        }
        ui.heading(&shelf.name);
        if self.shelf_draft.as_ref().map(|(id, _, _)| *id) != Some(shelf_id) {
            self.shelf_draft = Some((shelf_id, shelf.name.clone(), DimDraft::from_cm(size)));
        }
        if let Some((_, name, dims)) = self.shelf_draft.as_mut() {
            ui.horizontal(|ui| {
                ui.label("Name");
                ui.text_edit_singleline(name);
            });
            dim_controls(ui, dims);
        }
        if ui.button("Save shelf").clicked() {
            if let Some((_, name, dims)) = self.shelf_draft.clone() {
                let rename = self.store.rename_furniture(shelf_id, &name);
                let resize = self.store.update_shelf_size(shelf_id, dims.cm());
                let result = rename.and(resize);
                self.act(result, "Shelf saved");
            }
        }
        ui.add_space(8.0);
        let items: Vec<_> = self
            .snapshot
            .items_on_shelf(shelf_id)
            .into_iter()
            .cloned()
            .collect();
        let refs: Vec<_> = items.iter().collect();
        usage_bar(ui, size, &refs);
        self.item_list(ui, &items, Placement::Shelf(shelf_id));
    }

    fn furniture_name_editor(&mut self, ui: &mut egui::Ui, id: FurnitureId, current: &str) {
        if self
            .furniture_name_draft
            .as_ref()
            .map(|(draft_id, _)| *draft_id)
            != Some(id)
        {
            self.furniture_name_draft = Some((id, current.to_string()));
        }
        if let Some((_, name)) = self.furniture_name_draft.as_mut() {
            ui.horizontal(|ui| {
                ui.label("Name");
                ui.text_edit_singleline(name);
            });
        }
        if ui.button("Save name").clicked() {
            if let Some((_, name)) = self.furniture_name_draft.clone() {
                let result = self.store.rename_furniture(id, &name);
                self.act(result, "Name saved");
            }
        }
    }

    fn item_list(
        &mut self,
        ui: &mut egui::Ui,
        items: &[drawerama::domain::Item],
        placement: Placement,
    ) {
        ui.add_space(8.0);
        if items.is_empty() {
            ui.label("No items here.");
        } else {
            let rows: Vec<_> = items
                .iter()
                .map(|item| {
                    let category = self
                        .snapshot
                        .category(item.category_id)
                        .map(|category| category.name.clone())
                        .unwrap_or_else(|| "Unknown".to_string());
                    (item.id, item.name.clone(), category)
                })
                .collect();
            let return_to = self.screen.clone();
            table::show(
                ui,
                "placed-items",
                &[
                    Column::new("Name"),
                    Column::new("Category"),
                    Column::new("Actions"),
                ],
                rows.len(),
                |ui, row, col| {
                    let (id, name, category) = &rows[row];
                    match col {
                        0 => {
                            ui.label(name);
                        }
                        1 => {
                            ui.label(category);
                        }
                        2 => {
                            if ui.button("Open").clicked() {
                                self.open_item(Some(*id), return_to.clone());
                            }
                        }
                        _ => {}
                    }
                },
            );
        }
        if ui.button("Add item here").clicked() {
            self.item_form = super::ItemForm {
                placement,
                ..super::ItemForm::default()
            };
            self.open_item(None, self.screen.clone());
        }
    }
}

fn missing(ui: &mut egui::Ui, text: &str) {
    ui.label(text);
}
