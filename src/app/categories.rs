use eframe::egui;

use super::DraweramaApp;

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
        }
        for group in &groups {
            ui.heading(&group.name);
            let categories: Vec<_> = self
                .snapshot
                .categories
                .iter()
                .filter(|category| category.group_id == group.id)
                .cloned()
                .collect();
            if categories.is_empty() {
                ui.label("No categories in this group.");
            }
            for category in categories {
                ui.push_id(category.id.0, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(&category.name);
                        if ui.button("Delete").clicked() {
                            let result = self.store.delete_category(category.id);
                            self.act(result, "Category deleted");
                        }
                    });
                });
            }
            if ui.button(format!("Delete group {}", group.name)).clicked() {
                let result = self.store.delete_group(group.id);
                self.act(result, "Group deleted");
            }
            ui.add_space(6.0);
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
