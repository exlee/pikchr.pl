use super::*;

struct ZoomWindow {
    id: egui::Id,
    visible: bool,
    content: String,
}

impl Visible for ZoomWindow {
    fn visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}

impl Id for ZoomWindow {
    fn get_id(&self) -> egui::Id {
        self.id
    }
}

impl HasMenu for ZoomWindow {}
impl RenderToggle for ZoomWindow {}

impl InnerWindow for ZoomWindow {
    fn inner_window(
        &mut self,
        _ctx: &Context,
        ui: &mut Ui,
        _tx: Sender<Msg>,
        _background: DiagramBackground,
    ) {
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_sized(
                    ui.available_size(),
                    egui::TextEdit::multiline(&mut self.content),
                );
            });
    }
}

impl MiniWindow for ZoomWindow {
    fn get_title(&self) -> String {
        "Zoom test".to_owned()
    }

    fn help_topic(&self) -> HelpTopic {
        HelpTopic::Overview
    }
}

#[test]
fn window_zoom_is_scoped_by_window_id_and_resets_to_workspace_default() {
    let ctx = Context::default();
    let first = egui::Id::new("first-window");
    let second = egui::Id::new("second-window");

    set_window_zoom_factor(&ctx, first, 1.4);
    assert!((window_zoom_factor(&ctx, first) - 1.4).abs() < f32::EPSILON);
    assert!((window_zoom_factor(&ctx, second) - 1.0).abs() < f32::EPSILON);

    set_window_zoom_factor(&ctx, first, 1.0);
    assert!((window_zoom_factor(&ctx, first) - 1.0).abs() < f32::EPSILON);
}

#[test]
fn window_zoom_is_clamped_to_supported_range() {
    let ctx = Context::default();
    let id = egui::Id::new("window");

    set_window_zoom_factor(&ctx, id, 0.0);
    assert!((window_zoom_factor(&ctx, id) - MIN_WINDOW_ZOOM).abs() < f32::EPSILON);

    set_window_zoom_factor(&ctx, id, 10.0);
    assert!((window_zoom_factor(&ctx, id) - MAX_WINDOW_ZOOM).abs() < f32::EPSILON);
}

#[test]
fn zoom_style_does_not_expand_a_window_to_the_workspace_width() {
    use egui_kittest::Harness;

    let id = egui::Id::new("zoom-window");
    let (tx, _rx) = tokio::sync::mpsc::channel(8);
    let mut window = ZoomWindow {
        id,
        visible: true,
        content: "content".to_owned(),
    };
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1200.0, 800.0))
        .build(move |ctx| {
            window.show(ctx, tx.clone(), DiagramBackground::White);
        });

    harness.run_steps(2);
    let initial = harness
        .ctx
        .memory(|memory| memory.area_rect(id).expect("window should be visible"));
    assert!(
        initial.width() < 600.0,
        "unzoomed window filled workspace: {initial:?}"
    );

    set_window_zoom_factor(&harness.ctx, id, 1.5);
    harness.run_steps(2);
    let rect = harness
        .ctx
        .memory(|memory| memory.area_rect(id).expect("window should be visible"));

    assert!(
        rect.width() < 600.0,
        "zoomed window filled workspace: {rect:?}"
    );
    assert!(
        rect.height() > initial.height(),
        "outer window chrome did not scale: {initial:?} -> {rect:?}"
    );
}

#[derive(Default)]
struct WatchHarness {
    initialized: bool,
}

impl Id for WatchHarness {
    fn get_id(&self) -> egui::Id {
        egui::Id::new("watch-harness")
    }
}

impl Initialize for WatchHarness {
    fn is_initialized(&self) -> bool {
        self.initialized
    }

    fn set_initialized(&mut self) {
        self.initialized = true;
    }
}

impl InitializeWatchTx for WatchHarness {
    type ChangeData = usize;

    fn watch_change_fn(data: Self::ChangeData) -> Msg {
        Msg::RenameWindow(egui::Id::new("watch-harness"), data.to_string())
    }

    fn set_watch_tx(&mut self, _tx: watch::Sender<Self::ChangeData>) {}

    fn empty_change_data() -> Self::ChangeData {
        0
    }
}

#[tokio::test]
async fn watched_change_waits_for_message_queue_capacity() {
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(1);
    event_tx.send(Msg::CheckDependencies).await.unwrap();
    let (watch_tx, watch_rx) = watch::channel(0);
    watch_tx.send(7).unwrap();

    let forwarder = tokio::spawn(forward_watch_changes(
        watch_rx,
        event_tx,
        WatchHarness::watch_change_fn,
    ));
    tokio::task::yield_now().await;

    assert!(matches!(
        event_rx.recv().await,
        Some(Msg::CheckDependencies)
    ));
    let forwarded = tokio::time::timeout(tokio::time::Duration::from_secs(1), event_rx.recv())
        .await
        .expect("watched update should wait for queue capacity");
    assert!(matches!(
        forwarded,
        Some(Msg::RenameWindow(_, content)) if content == "7"
    ));

    drop(watch_tx);
    tokio::time::timeout(tokio::time::Duration::from_secs(1), forwarder)
        .await
        .expect("forwarder should stop after watch closes")
        .unwrap();
}
