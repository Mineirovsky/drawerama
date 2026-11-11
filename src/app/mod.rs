mod categories;
mod items;
mod rooms;
mod suggest_ui;

use eframe::egui;

use std::collections::HashSet;
use std::path::PathBuf;

use drawerama::domain::{
    CategoryId, Centimeters, DrawerId, FurnitureId, GroupId, ItemId, Placement, RoomId, Snapshot,
    Suggestion, plan,
};
use drawerama::storage::{SaveItem, Store};

pub struct DraweramaApp {
    store: Store,
    db_path: PathBuf,
    snapshot: Snapshot,
    screen: Screen,
    error: Option<String>,
    status: Option<String>,
    draft_room: String,
    draft_furniture_name: String,
    draft_kind_shelf: bool,
    draft_shelf_dims: DimDraft,
    draft_drawer_name: String,
    draft_drawer_dims: DimDraft,
    draft_group: String,
    draft_category_group: Option<GroupId>,
    draft_category: String,
    item_form: ItemForm,
    search: String,
    selected_unplaced: HashSet<ItemId>,
    suggestion: Option<Suggestion>,
    suggestion_batch: Vec<ItemId>,
    suggestion_return: Screen,
    drawer_draft: Option<(DrawerId, String, DimDraft)>,
    shelf_draft: Option<(FurnitureId, String, DimDraft)>,
    room_draft: Option<(RoomId, String)>,
    furniture_name_draft: Option<(FurnitureId, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Screen {
    Rooms,
    Room(RoomId),
    Cabinet(FurnitureId),
    Drawer(DrawerId),
    Shelf(FurnitureId),
    Categories,
    Item(Option<ItemId>),
    Unplaced,
    AllItems,
    Suggestion,
}

#[derive(Clone)]
struct DimDraft {
    width: f64,
    height: f64,
    depth: f64,
}

impl DimDraft {
    fn cm(&self) -> Centimeters {
        Centimeters {
            width: self.width,
            height: self.height,
            depth: self.depth,
        }
    }

    fn from_cm(size: Centimeters) -> Self {
        Self {
            width: size.width,
            height: size.height,
            depth: size.depth,
        }
    }

    fn drawer() -> Self {
        Self {
            width: 40.0,
            height: 15.0,
            depth: 40.0,
        }
    }

    fn shelf() -> Self {
        Self {
            width: 80.0,
            height: 35.0,
            depth: 30.0,
        }
    }

    fn item() -> Self {
        Self {
            width: 20.0,
            height: 10.0,
            depth: 20.0,
        }
    }
}

#[derive(Clone)]
struct ItemForm {
    name: String,
    group_id: Option<GroupId>,
    category_id: Option<CategoryId>,
    new_group: String,
    new_category: String,
    dims: DimDraft,
    placement: Placement,
}

impl Default for ItemForm {
    fn default() -> Self {
        Self {
            name: String::new(),
            group_id: None,
            category_id: None,
            new_group: String::new(),
            new_category: String::new(),
            dims: DimDraft::item(),
            placement: Placement::Unplaced,
        }
    }
}

impl DraweramaApp {
    pub fn new(store: Store, snapshot: Snapshot, db_path: PathBuf) -> Self {
        let draft_category_group = snapshot.groups.first().map(|group| group.id);
        Self {
            store,
            db_path,
            snapshot,
            screen: Screen::Rooms,
            error: None,
            status: None,
            draft_room: String::new(),
            draft_furniture_name: String::new(),
            draft_kind_shelf: false,
            draft_shelf_dims: DimDraft::shelf(),
            draft_drawer_name: String::new(),
            draft_drawer_dims: DimDraft::drawer(),
            draft_group: String::new(),
            draft_category_group,
            draft_category: String::new(),
            item_form: ItemForm::default(),
            search: String::new(),
            selected_unplaced: HashSet::new(),
            suggestion: None,
            suggestion_batch: Vec::new(),
            suggestion_return: Screen::Unplaced,
            drawer_draft: None,
            shelf_draft: None,
            room_draft: None,
            furniture_name_draft: None,
        }
    }

    fn act<T>(&mut self, result: anyhow::Result<T>, ok: &str) {
        match result {
            Ok(_) => match self.store.load() {
                Ok(snapshot) => {
                    self.snapshot = snapshot;
                    self.status = Some(ok.to_string());
                    self.error = None;
                }
                Err(err) => self.error = Some(err.to_string()),
            },
            Err(err) => self.error = Some(err.to_string()),
        }
    }

    fn open_item(&mut self, id: Option<ItemId>, return_to: Screen) {
        self.suggestion_return = return_to;
        if let Some(id) = id {
            if let Some(item) = self.snapshot.item(id).cloned() {
                self.item_form = ItemForm {
                    name: item.name,
                    group_id: self.snapshot.group_of(item.category_id),
                    category_id: Some(item.category_id),
                    new_group: String::new(),
                    new_category: String::new(),
                    dims: DimDraft::from_cm(item.size),
                    placement: item.placement,
                };
            }
        }
        self.screen = Screen::Item(id);
    }

    fn open_suggestion(&mut self, batch: Vec<ItemId>, return_to: Screen) {
        self.suggestion = Some(plan(&self.snapshot, &batch));
        self.suggestion_batch = batch;
        self.suggestion_return = return_to;
        self.screen = Screen::Suggestion;
        self.status = None;
    }

    fn save_item_form(&mut self) -> anyhow::Result<ItemId> {
        let Some(category_id) = self.item_form.category_id else {
            anyhow::bail!("Choose a category");
        };
        self.store.save_item(SaveItem {
            id: match self.screen {
                Screen::Item(id) => id,
                _ => None,
            },
            name: self.item_form.name.clone(),
            category_id,
            size: self.item_form.dims.cm(),
            placement: self.item_form.placement,
        })
    }
}

impl eframe::App for DraweramaApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::left("nav")
            .resizable(false)
            .exact_size(180.0)
            .show(ui, |ui| self.nav(ui));
        egui::Panel::bottom("footer").show(ui, |ui| {
            ui.label(format!("Data file: {}", self.db_path.display()));
        });
        egui::CentralPanel::default().show(ui, |ui| {
            if let Some(error) = self.error.clone() {
                ui.horizontal(|ui| {
                    ui.colored_label(egui::Color32::from_rgb(210, 90, 80), error);
                    if ui.button("Dismiss").clicked() {
                        self.error = None;
                    }
                });
            }
            if let Some(status) = &self.status {
                ui.label(status);
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                let screen = self.screen.clone();
                match screen {
                    Screen::Rooms => self.ui_rooms(ui),
                    Screen::Room(id) => self.ui_room(ui, id),
                    Screen::Cabinet(id) => self.ui_cabinet(ui, id),
                    Screen::Drawer(id) => self.ui_drawer(ui, id),
                    Screen::Shelf(id) => self.ui_shelf(ui, id),
                    Screen::Categories => self.ui_categories(ui),
                    Screen::Item(id) => self.ui_item(ui, id),
                    Screen::Unplaced => self.ui_unplaced(ui),
                    Screen::AllItems => self.ui_all_items(ui),
                    Screen::Suggestion => self.ui_suggestion(ui),
                }
            });
        });
    }
}

impl DraweramaApp {
    fn nav(&mut self, ui: &mut egui::Ui) {
        ui.heading("Drawerama");
        ui.add_space(8.0);
        let unplaced = self
            .snapshot
            .items
            .iter()
            .filter(|item| item.placement == Placement::Unplaced)
            .count();
        let screen = self.screen.clone();
        if ui
            .selectable_label(matches!(screen, Screen::Rooms | Screen::Room(_)), "Rooms")
            .clicked()
        {
            self.screen = Screen::Rooms;
        }
        if ui
            .selectable_label(matches!(screen, Screen::Categories), "Categories")
            .clicked()
        {
            self.screen = Screen::Categories;
        }
        if ui
            .selectable_label(matches!(screen, Screen::AllItems), "All items")
            .clicked()
        {
            self.screen = Screen::AllItems;
        }
        if ui
            .selectable_label(
                matches!(screen, Screen::Unplaced),
                format!("Unplaced ({unplaced})"),
            )
            .clicked()
        {
            self.screen = Screen::Unplaced;
        }
    }
}

fn dim_controls(ui: &mut egui::Ui, dims: &mut DimDraft) {
    egui::Grid::new(ui.scope_id().with("dims"))
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("Width");
            ui.add(
                egui::DragValue::new(&mut dims.width)
                    .range(0.1..=5000.0)
                    .suffix(" cm")
                    .speed(0.5),
            );
            ui.end_row();
            ui.label("Height");
            ui.add(
                egui::DragValue::new(&mut dims.height)
                    .range(0.1..=5000.0)
                    .suffix(" cm")
                    .speed(0.5),
            );
            ui.end_row();
            ui.label("Depth");
            ui.add(
                egui::DragValue::new(&mut dims.depth)
                    .range(0.1..=5000.0)
                    .suffix(" cm")
                    .speed(0.5),
            );
            ui.end_row();
        });
}

fn usage_bar(ui: &mut egui::Ui, size: Centimeters, items: &[&drawerama::domain::Item]) {
    let usage = drawerama::domain::usage_of(size, items);
    let fraction = usage.fraction().clamp(0.0, 1.0) as f32;
    ui.add(egui::ProgressBar::new(fraction).text(format!(
        "{} used of {} usable ({:.0}%)",
        drawerama::domain::format_liters(usage.used_cm3),
        drawerama::domain::format_liters(usage.usable_cm3),
        usage.fraction() * 100.0
    )));
    ui.label(format!(
        "Inside dimensions: {:.1} × {:.1} × {:.1} cm",
        size.width, size.height, size.depth
    ));
}
