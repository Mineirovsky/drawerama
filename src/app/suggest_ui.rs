use eframe::egui;

use drawerama::domain::plan;

use super::DraweramaApp;

impl DraweramaApp {
    pub(super) fn ui_suggestion(&mut self, ui: &mut egui::Ui) {
        if ui.button("Back").clicked() {
            self.screen = self.suggestion_return.clone();
        }
        ui.heading("Suggestion");
        if self.suggestion.is_none() {
            let batch = self.suggestion_batch.clone();
            self.suggestion = Some(plan(&self.snapshot, &batch));
        }
        let Some(suggestion) = self.suggestion.clone() else {
            return;
        };

        ui.label(
            "This places only the items you asked about. Everything already stored stays put.",
        );
        if suggestion.primary.is_empty() && suggestion.primary_unplaced.is_empty() {
            ui.label("No items were included.");
        }
        for assignment in &suggestion.primary {
            let name = self.item_name(assignment.item_id);
            ui.label(format!(
                "{name} → {}",
                self.snapshot.describe(assignment.placement)
            ));
            ui.label(format!("    {}", assignment.note));
        }
        for id in &suggestion.primary_unplaced {
            ui.label(format!("{} — no allowed space found", self.item_name(*id)));
        }
        if ui
            .add_enabled(
                !suggestion.primary.is_empty(),
                egui::Button::new("Apply this placement"),
            )
            .clicked()
        {
            let placements: Vec<_> = suggestion
                .primary
                .iter()
                .map(|assignment| (assignment.item_id, assignment.placement))
                .collect();
            let batch = self.suggestion_batch.clone();
            let result = self.store.apply_placements(&placements);
            self.act(result, "Placement applied");
            self.suggestion = Some(plan(&self.snapshot, &batch));
        }

        ui.add_space(12.0);
        ui.separator();
        ui.heading("Reorganize these categories");
        ui.label("Optional. This can move items that are already stored, and it includes other unplaced items of the same categories.");
        let consolidation = &suggestion.consolidation;
        if consolidation.already_optimal {
            ui.label("The stored arrangement already matches these rules.");
        }
        for relocation in &consolidation.moves {
            let name = self.item_name(relocation.item_id);
            ui.label(format!(
                "{name}: {} → {}",
                self.snapshot.describe(relocation.from),
                self.snapshot.describe(relocation.to)
            ));
            ui.label(format!("    {}", relocation.note));
        }
        for id in &consolidation.not_assigned {
            ui.label(format!("{} — still no allowed space", self.item_name(*id)));
        }
        if ui
            .add_enabled(
                !consolidation.moves.is_empty(),
                egui::Button::new("Apply reorganization"),
            )
            .clicked()
        {
            let placements: Vec<_> = consolidation
                .moves
                .iter()
                .map(|relocation| (relocation.item_id, relocation.to))
                .collect();
            let batch = self.suggestion_batch.clone();
            let result = self.store.apply_placements(&placements);
            self.act(result, "Reorganization applied");
            self.suggestion = Some(plan(&self.snapshot, &batch));
        }
    }

    fn item_name(&self, id: drawerama::domain::ItemId) -> String {
        self.snapshot
            .item(id)
            .map(|item| item.name.clone())
            .unwrap_or_else(|| "Missing item".to_string())
    }
}
