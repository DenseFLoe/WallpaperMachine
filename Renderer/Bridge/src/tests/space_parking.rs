//! A display in the "different wallpaper for each desktop" mode keeps the
//! scenes it leaves loaded and paused. The bridge hands the engine the project
//! files of the wallpapers it may keep and names each scene window by its
//! wallpaper, so the host can keep every window on its own desktops.

use wallpaper_core::{
    DisplayDesc, DisplayIdentity, DisplaySnapshotEntry, SceneWindowEntry,
    project::{SceneDesc, SceneHandle},
};

use crate::{BridgeErrorKind, BridgeSceneWindow, api::BridgeBuilder, engine::FakeEngineFacade};

fn display(display_id: u32) -> DisplaySnapshotEntry {
    DisplaySnapshotEntry {
        identity: DisplayIdentity::default(),
        desc: DisplayDesc::with_identity(display_id, DisplayIdentity::default(), 0, 0, 1920, 1080, 1.0),
        handle: None,
        accepts_pointer_input: false,
        paused: false,
        window_active: false,
        assignment: None,
    }
}

fn bridge(engine: &FakeEngineFacade) -> crate::api::WallpaperBridge {
    engine.set_snapshot(vec![display(7)]);
    BridgeBuilder::new(engine.clone())
        .with_state(crate::actor::state::BridgeActorState::default())
        .build()
        .unwrap()
}

fn window(scene_path: &str, window_number: Option<isize>, parked: bool) -> SceneWindowEntry {
    let desc = display(7).desc;
    SceneWindowEntry {
        display_id: 7,
        handle: SceneHandle::new(3),
        desc: SceneDesc::new(desc, scene_path, "/assets", 30, false),
        window_number,
        parked,
        ready: true,
    }
}

#[tokio::test(flavor = "current_thread")]
async fn a_display_hands_the_engine_the_project_files_of_the_wallpapers_it_keeps() {
    let engine = FakeEngineFacade::default();
    let bridge = bridge(&engine);

    bridge
        .set_display_parked_wallpapers("7".into(), vec!["100".into(), "200".into()])
        .await
        .unwrap();
    bridge.set_display_parked_wallpapers("7".into(), Vec::new()).await.unwrap();

    let calls = engine.parked_scene_calls();
    assert_eq!(calls.len(), 2);
    let (display_id, paths) = &calls[0];
    assert_eq!(*display_id, 7);
    assert_eq!(paths.len(), 2);
    for (path, id) in paths.iter().zip(["100", "200"]) {
        assert!(path.ends_with(&format!("/{id}/project.json")), "{path}");
    }
    assert_eq!(calls[1], (7, Vec::new()), "an empty list frees what the display keeps");
}

#[tokio::test(flavor = "current_thread")]
async fn a_display_id_that_is_not_a_number_is_refused() {
    let engine = FakeEngineFacade::default();
    let bridge = bridge(&engine);

    let error = bridge
        .set_display_parked_wallpapers("main".into(), vec!["100".into()])
        .await
        .unwrap_err();
    assert_eq!(error.kind(), BridgeErrorKind::InvalidInput);
    assert!(engine.parked_scene_calls().is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn each_scene_window_is_named_by_its_wallpaper() {
    let engine = FakeEngineFacade::default();
    let bridge = bridge(&engine);
    engine.set_scene_windows(vec![
        window("/workshop/100/project.json", Some(42), false),
        window("/workshop/200/project.json", Some(43), true),
        window("/workshop/300/project.json", None, true),
    ]);

    assert_eq!(
        bridge.scene_windows(),
        vec![
            BridgeSceneWindow { display_id: 7, wallpaper_id: "100".into(), window_number: 42, parked: false, ready: true },
            BridgeSceneWindow { display_id: 7, wallpaper_id: "200".into(), window_number: 43, parked: true, ready: true },
        ],
        "a scene without a window has nothing to pin"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn preparing_an_unvisited_scene_preserves_selection_assignments_and_saved_options() {
    let engine = FakeEngineFacade::default();
    let bridge = bridge(&engine);
    bridge.inject_scene_project_for_test("100", "Aurora", r#"{"type":"scene","file":"scene.json"}"#).await;
    bridge.inject_scene_wallpaper_config_for_test("100", "Scene").await;
    bridge.inject_scene_project_for_test("200", "Basalt", r#"{"type":"scene","file":"scene.json"}"#).await;
    bridge.inject_scene_wallpaper_config_for_test("200", "Scene").await;
    bridge.set_display_config_enabled("100".into(), "primary".into(), true).await.unwrap();
    bridge.apply_wallpaper_options("100".into(), true).await.unwrap();
    let before = bridge.all_snapshots().await.unwrap();
    let reply = bridge.preload_desktop_wallpaper(7, "200".into()).await.unwrap();
    assert!(reply.web.is_none() && reply.video.is_none());
    assert_eq!(bridge.all_snapshots().await.unwrap(), before, "preparation never commits or selects a wallpaper");
    let prepared = engine.preloaded_scene_calls();
    assert_eq!(prepared.len(), 1);
    assert_eq!(prepared[0].display.display_id, 7);
    assert!(prepared[0].scene_path.ends_with("/200/project.json"));
    assert!(prepared[0].audio_muted);
}
