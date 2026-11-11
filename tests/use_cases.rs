use drawerama::domain::{Centimeters, FurnitureKind, Placement, plan};
use drawerama::storage::{SaveItem, Store};

fn size(width: f64, height: f64, depth: f64) -> Centimeters {
    Centimeters {
        width,
        height,
        depth,
    }
}

#[test]
fn the_five_use_cases_register_a_home_and_suggest_a_drawer() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(&dir.path().join("home.db")).unwrap();

    let bedroom = store.create_room("Bedroom").unwrap();
    let cabinet = store.create_cabinet(bedroom, "Dresser").unwrap();
    let shelf = store
        .create_shelf(bedroom, "Top shelf", size(80.0, 30.0, 30.0))
        .unwrap();
    let top = store
        .create_drawer(cabinet, "Top", size(40.0, 12.0, 40.0))
        .unwrap();
    let bottom = store
        .create_drawer(cabinet, "Bottom", size(40.0, 20.0, 40.0))
        .unwrap();

    let textiles = store.create_group("Textiles").unwrap();
    let shirts = store.create_category(textiles, "Shirts").unwrap();
    let shirt = store
        .save_item(SaveItem {
            id: None,
            name: "Blue shirt".into(),
            category_id: shirts,
            size: size(25.0, 4.0, 20.0),
            placement: Placement::Unplaced,
        })
        .unwrap();

    let snap = store.load().unwrap();
    assert_eq!(snap.rooms.len(), 1);
    assert!(matches!(
        snap.furniture(cabinet).unwrap().kind,
        FurnitureKind::Cabinet
    ));
    assert!(matches!(
        snap.furniture(shelf).unwrap().kind,
        FurnitureKind::Shelf(_)
    ));
    assert_eq!(snap.drawers_in(cabinet).len(), 2);
    assert_eq!(snap.item(shirt).unwrap().placement, Placement::Unplaced);

    let suggestion = plan(&snap, &[shirt]);
    let placement = suggestion
        .primary
        .iter()
        .find(|assignment| assignment.item_id == shirt)
        .map(|assignment| assignment.placement);
    assert!(
        matches!(placement, Some(Placement::Drawer(id)) if id == top || id == bottom),
        "expected a dresser drawer, got {placement:?}"
    );

    store
        .apply_placements(&[(shirt, placement.unwrap())])
        .unwrap();
    let snap = store.load().unwrap();
    assert_ne!(snap.item(shirt).unwrap().placement, Placement::Unplaced);
    let path = snap.describe(snap.item(shirt).unwrap().placement);
    assert!(path.contains("Bedroom"));
    assert!(path.contains("Dresser"));
}
