use eframe::egui;

use super::table::Column;
use super::{DraweramaApp, table};

impl DraweramaApp {
    pub(super) fn ui_categories(&mut self, ui: &mut egui::Ui) {
        ui.heading("Categories");
        ui.label("A group is the mixing rule. Shirts and blankets can share a cabinet. Chemicals cannot share one with textiles.");
        ui.horizontal(|ui| {
            ui.label("New group");
            ui.text_edit_singleline(&mut self.draft_group);
            if ui.button("Add group").clicked() {
                let name = self.draft_group.clone();
                match self.store.create_group(&name) {
                    Ok(id) => {
                        self.draft_category_group = Some(id);
                        self.act(Ok(()), "Group added");
                        self.draft_group.clear();
                    }
                    Err(err) => self.error = Some(err.to_string()),
                }
            }
        });

        ui.add_space(8.0);
        let groups = self.snapshot.groups.clone();
        if groups.is_empty() {
            ui.label("No groups yet.");
        } else {
            table::show(
                ui,
                "groups",
                &[Column::new("Name"), Column::new("Actions")],
                groups.len(),
                |ui, row, col| {
                    let group = &groups[row];
                    match col {
                        0 => {
                            ui.label(&group.name);
                        }
                        1 => {
                            ui.horizontal(|ui| {
                                if ui.button("Rename").clicked() {
                                    self.open_rename(
                                        super::RenameTarget::Group(group.id),
                                        &group.name,
                                    );
                                }
                                if ui.button("Delete").clicked() {
                                    let result = self.store.delete_group(group.id);
                                    self.act(result, "Group deleted");
                                }
                            });
                        }
                        _ => {}
                    }
                },
            );
        }

        ui.add_space(12.0);
        let categories: Vec<_> = self
            .snapshot
            .categories
            .iter()
            .map(|category| {
                let group = self
                    .snapshot
                    .group(category.group_id)
                    .map(|group| group.name.clone())
                    .unwrap_or_else(|| "Unknown".to_string());
                (category.id, category.name.clone(), group)
            })
            .collect();
        if categories.is_empty() {
            ui.label("No categories yet.");
        } else {
            table::show(
                ui,
                "categories",
                &[
                    Column::new("Name"),
                    Column::new("Group"),
                    Column::new("Actions"),
                ],
                categories.len(),
                |ui, row, col| {
                    let (id, name, group) = &categories[row];
                    match col {
                        0 => {
                            ui.label(name);
                        }
                        1 => {
                            ui.label(group);
                        }
                        2 => {
                            ui.horizontal(|ui| {
                                if ui.button("Rename").clicked() {
                                    self.open_rename(super::RenameTarget::Category(*id), name);
                                }
                                if ui.button("Delete").clicked() {
                                    let result = self.store.delete_category(*id);
                                    self.act(result, "Category deleted");
                                }
                            });
                        }
                        _ => {}
                    }
                },
            );
        }

        ui.separator();
        ui.label("Add a category to a group");
        let group_labels: Vec<_> = groups
            .iter()
            .map(|group| (group.id, group.name.clone()))
            .collect();
        egui::ComboBox::from_id_salt("category-group")
            .selected_text(
                self.draft_category_group
                    .and_then(|id| self.snapshot.group(id).map(|group| group.name.clone()))
                    .unwrap_or_else(|| "Choose a group".to_string()),
            )
            .show_ui(ui, |ui| {
                for (id, name) in &group_labels {
                    ui.selectable_value(&mut self.draft_category_group, Some(*id), name);
                }
            });
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.draft_category);
            if ui.button("Add category").clicked() {
                let Some(group_id) = self.draft_category_group else {
                    self.error = Some("Choose a group first".to_string());
                    return;
                };
                let name = self.draft_category.clone();
                let result = self.store.create_category(group_id, &name);
                self.act(result, "Category added");
                if self.error.is_none() {
                    self.draft_category.clear();
                }
            }
        });
    }
}

impl DraweramaApp {
    fn open_rename(&mut self, target: super::RenameTarget, name: &str) {
        self.error = None;
        self.rename = Some(super::RenameDraft {
            target,
            name: name.to_string(),
            focus: true,
        });
    }

    pub(super) fn show_rename_modal(&mut self, ctx: &egui::Context) {
        if self.rename.is_none() {
            return;
        }
        let title = match self.rename.as_ref().map(|draft| &draft.target) {
            Some(super::RenameTarget::Group(_)) => "Rename group",
            Some(super::RenameTarget::Category(_)) => "Rename category",
            None => return,
        };
        let error = self.error.clone();
        let mut confirm = false;
        let mut cancel = false;
        let response = egui::Modal::new(egui::Id::unique("rename")).show(ctx, |ui| {
            ui.set_min_width(280.0);
            ui.heading(title);
            let Some(draft) = &mut self.rename else {
                return;
            };
            let editor = ui.text_edit_singleline(&mut draft.name);
            if draft.focus {
                editor.request_focus();
                draft.focus = false;
            }
            if let Some(error) = &error {
                ui.colored_label(egui::Color32::from_rgb(210, 90, 80), error);
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Confirm").clicked() {
                    confirm = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });
        if response.should_close() {
            cancel = true;
        }
        if confirm {
            self.confirm_rename();
        } else if cancel {
            self.rename = None;
            self.error = None;
        }
    }

    fn confirm_rename(&mut self) {
        let Some(draft) = self.rename.clone() else {
            return;
        };
        let (result, message) = match draft.target {
            super::RenameTarget::Group(id) => {
                (self.store.rename_group(id, &draft.name), "Group renamed")
            }
            super::RenameTarget::Category(id) => (
                self.store.rename_category(id, &draft.name),
                "Category renamed",
            ),
        };
        self.act(result, message);
        if self.error.is_none() {
            self.rename = None;
        }
    }
}
