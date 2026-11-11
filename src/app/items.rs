use eframe::egui;

use drawerama::domain::{CategoryId, GroupId, ItemId, Placement};

use super::{DraweramaApp, ItemForm, Screen, dim_controls};

impl DraweramaApp {
    pub(super) fn ui_item(&mut self, ui: &mut egui::Ui, id: Option<ItemId>) {
        if ui.button("Back").clicked() {
            self.screen = self.suggestion_return.clone();
        }
        ui.heading(if id.is_some() { "Item" } else { "New item" });

        ui.horizontal(|ui| {
            ui.label("Name");
            ui.text_edit_singleline(&mut self.item_form.name);
        });

        self.group_picker(ui);
        self.category_picker(ui);
        ui.label("Dimensions");
        dim_controls(ui, &mut self.item_form.dims);
        self.placement_picker(ui);

        ui.horizontal(|ui| {
            if ui.button("Save").clicked() {
                match self.save_item_form() {
                    Ok(saved) => {
                        self.act(Ok(()), "Item saved");
                        self.screen = Screen::Item(Some(saved));
                    }
                    Err(err) => self.error = Some(err.to_string()),
                }
            }
            if ui.button("Save and suggest").clicked() {
                match self.save_item_form() {
                    Ok(saved) => {
                        self.act(Ok(()), "Item saved");
                        let return_to = Screen::Item(Some(saved));
                        self.open_suggestion(vec![saved], return_to);
                    }
                    Err(err) => self.error = Some(err.to_string()),
                }
            }
            if let Some(id) = id {
                if ui.button("Delete item").clicked() {
                    let result = self.store.delete_item(id);
                    self.act(result, "Item deleted");
                    if self.error.is_none() {
                        self.screen = self.suggestion_return.clone();
                    }
                }
            }
        });
    }

    fn group_picker(&mut self, ui: &mut egui::Ui) {
        ui.label("Group");
        let groups: Vec<_> = self
            .snapshot
            .groups
            .iter()
            .map(|group| (group.id, group.name.clone()))
            .collect();
        if groups.is_empty() {
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.item_form.new_group);
                if ui.button("Add group").clicked() {
                    let name = self.item_form.new_group.clone();
                    match self.store.create_group(&name) {
                        Ok(id) => {
                            self.act(Ok(()), "Group added");
                            self.item_form.group_id = Some(id);
                            self.item_form.new_group.clear();
                        }
                        Err(err) => self.error = Some(err.to_string()),
                    }
                }
            });
            return;
        }
        egui::ComboBox::from_id_salt("item-group")
            .selected_text(self.group_name(self.item_form.group_id))
            .show_ui(ui, |ui| {
                for (id, name) in &groups {
                    ui.selectable_value(&mut self.item_form.group_id, Some(*id), name);
                }
            });
        if let Some(group_id) = self.item_form.group_id {
            if self
                .item_form
                .category_id
                .and_then(|id| self.snapshot.category(id))
                .is_some_and(|category| category.group_id != group_id)
            {
                self.item_form.category_id = None;
            }
        }
    }

    fn category_picker(&mut self, ui: &mut egui::Ui) {
        ui.label("Category");
        let Some(group_id) = self.item_form.group_id else {
            ui.label("Choose a group before the category.");
            return;
        };
        let categories: Vec<_> = self
            .snapshot
            .categories
            .iter()
            .filter(|category| category.group_id == group_id)
            .map(|category| (category.id, category.name.clone()))
            .collect();
        egui::ComboBox::from_id_salt("item-category")
            .selected_text(self.category_name(self.item_form.category_id))
            .show_ui(ui, |ui| {
                for (id, name) in &categories {
                    ui.selectable_value(&mut self.item_form.category_id, Some(*id), name);
                }
            });
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.item_form.new_category);
            if ui.button("Add category").clicked() {
                let name = self.item_form.new_category.clone();
                match self.store.create_category(group_id, &name) {
                    Ok(id) => {
                        self.act(Ok(()), "Category added");
                        self.item_form.category_id = Some(id);
                        self.item_form.new_category.clear();
                    }
                    Err(err) => self.error = Some(err.to_string()),
                }
            }
        });
    }

    fn placement_picker(&mut self, ui: &mut egui::Ui) {
        ui.label("Location");
        let mut options = vec![(Placement::Unplaced, "Unplaced".to_string())];
        for furniture in &self.snapshot.furniture {
            match furniture.kind {
                drawerama::domain::FurnitureKind::Shelf(_) => {
                    options.push((
                        Placement::Shelf(furniture.id),
                        self.snapshot.describe(Placement::Shelf(furniture.id)),
                    ));
                }
                drawerama::domain::FurnitureKind::Cabinet => {
                    for drawer in self.snapshot.drawers_in(furniture.id) {
                        options.push((
                            Placement::Drawer(drawer.id),
                            self.snapshot.describe(Placement::Drawer(drawer.id)),
                        ));
                    }
                }
            }
        }
        let selected = self.snapshot.describe(self.item_form.placement);
        egui::ComboBox::from_id_salt("item-placement")
            .selected_text(selected)
            .show_ui(ui, |ui| {
                for (placement, label) in options {
                    ui.selectable_value(&mut self.item_form.placement, placement, label);
                }
            });
    }

    fn group_name(&self, id: Option<GroupId>) -> String {
        id.and_then(|id| self.snapshot.group(id).map(|group| group.name.clone()))
            .unwrap_or_else(|| "Choose a group".to_string())
    }

    fn category_name(&self, id: Option<CategoryId>) -> String {
        id.and_then(|id| {
            self.snapshot
                .category(id)
                .map(|category| category.name.clone())
        })
        .unwrap_or_else(|| "Choose a category".to_string())
    }

    pub(super) fn ui_unplaced(&mut self, ui: &mut egui::Ui) {
        ui.heading("Unplaced");
        ui.label("Check items, then ask for one suggestion for the whole batch.");
        ui.horizontal(|ui| {
            if ui.button("Select all").clicked() {
                self.selected_unplaced = self
                    .snapshot
                    .items
                    .iter()
                    .filter(|item| item.placement == Placement::Unplaced)
                    .map(|item| item.id)
                    .collect();
            }
            if ui.button("Clear").clicked() {
                self.selected_unplaced.clear();
            }
            let count = self.selected_unplaced.len();
            if ui
                .add_enabled(count > 0, egui::Button::new(format!("Suggest for {count}")))
                .clicked()
            {
                let mut batch: Vec<_> = self.selected_unplaced.iter().copied().collect();
                batch.sort();
                self.open_suggestion(batch, Screen::Unplaced);
            }
            if ui.button("New item").clicked() {
                self.item_form = ItemForm::default();
                self.open_item(None, Screen::Unplaced);
            }
        });

        let rows: Vec<_> = self
            .snapshot
            .items
            .iter()
            .filter(|item| item.placement == Placement::Unplaced)
            .map(|item| {
                (
                    item.id,
                    item.name.clone(),
                    self.category_label(item.category_id),
                )
            })
            .collect();
        if rows.is_empty() {
            ui.label("Every item has a place.");
        }
        for (id, name, category) in rows {
            ui.push_id(id.0, |ui| {
                ui.horizontal(|ui| {
                    let mut on = self.selected_unplaced.contains(&id);
                    if ui.checkbox(&mut on, "").changed() {
                        if on {
                            self.selected_unplaced.insert(id);
                        } else {
                            self.selected_unplaced.remove(&id);
                        }
                    }
                    if ui.button(&name).clicked() {
                        self.open_item(Some(id), Screen::Unplaced);
                    }
                    ui.label(&category);
                    if ui.button("Suggest").clicked() {
                        self.open_suggestion(vec![id], Screen::Unplaced);
                    }
                });
            });
        }
    }

    pub(super) fn ui_all_items(&mut self, ui: &mut egui::Ui) {
        ui.heading("All items");
        ui.horizontal(|ui| {
            ui.label("Find");
            ui.text_edit_singleline(&mut self.search);
        });
        let query = self.search.trim().to_lowercase();
        let rows: Vec<_> = self
            .snapshot
            .items
            .iter()
            .filter(|item| query.is_empty() || item.name.to_lowercase().contains(&query))
            .map(|item| {
                (
                    item.id,
                    item.name.clone(),
                    self.category_label(item.category_id),
                    self.snapshot.describe(item.placement),
                )
            })
            .collect();
        if rows.is_empty() {
            ui.label("No matching items.");
        }
        for (id, name, category, place) in rows {
            ui.push_id(id.0, |ui| {
                ui.horizontal(|ui| {
                    if ui.button(&name).clicked() {
                        self.open_item(Some(id), Screen::AllItems);
                    }
                    ui.label(category);
                    ui.label(place);
                });
            });
        }
    }

    fn category_label(&self, id: CategoryId) -> String {
        let Some(category) = self.snapshot.category(id) else {
            return "Unknown".to_string();
        };
        let group = self
            .snapshot
            .group(category.group_id)
            .map(|group| group.name.as_str())
            .unwrap_or("Unknown");
        format!("{group} / {}", category.name)
    }
}
