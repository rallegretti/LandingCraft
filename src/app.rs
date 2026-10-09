//! Launcher state and actions. The pages themselves live in `app/*.rs`.

mod detail;
mod dialogs;
mod library;
mod prefs;
mod titlebar;
mod widgets;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use eframe::egui::{self, ScrollArea, Stroke, TextureHandle, Ui};
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::catalog::APPS;
use crate::detect::{self, Processes};
use crate::installer::{self, Manifest, Stage};
use crate::releases::{self, Asset, Cached, Release};
use crate::settings::{self, Settings};
use crate::{portal, theme};

const STORAGE_KEY: &str = "landingcraft";
/// Skip the startup check if the cached release lists are newer than this.
const STARTUP_RECHECK: u64 = 15 * 60;
/// While the launcher stays open, look again this often.
const BACKGROUND_RECHECK: u64 = 6 * 60 * 60;

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
enum Page {
    #[default]
    All,
    Installed,
    App(usize),
    Settings,
}

/// UI state remembered between sessions (window-level, not critical).
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct UiState {
    page: Page,
}

/// An app found outside the install folder.
struct External {
    path: PathBuf,
    custom: bool,
}

struct Job {
    version: Version,
    uninstalling: bool,
    stage: Stage,
    done: u64,
    total: u64,
    cancel: Arc<AtomicBool>,
}

impl Job {
    fn fraction(&self) -> f32 {
        if self.total == 0 { 0.0 } else { (self.done as f64 / self.total as f64) as f32 }
    }
}

enum Event {
    Releases { idx: usize, result: Result<Cached, String> },
    Progress { idx: usize, stage: Stage, done: u64, total: u64 },
    Installed { idx: usize, result: Result<Manifest, String> },
    Uninstalled { idx: usize, result: Result<(), String> },
    MoveProgress { done: usize },
    Moved { to: PathBuf, result: Result<(), String> },
    FolderPicked(Result<Option<PathBuf>, String>),
    LinkFailed { url: String, err: String },
}

enum Dialog {
    Uninstall(usize),
    Move { to: PathBuf, apps: Vec<usize>, bytes: u64, progress: Option<usize> },
}

struct Toast {
    text: String,
    until: f64,
}

struct GpuInfo {
    active: eframe::wgpu::AdapterInfo,
    all: Vec<eframe::wgpu::AdapterInfo>,
}

pub struct Launcher {
    ui_state: UiState,
    settings: Settings,
    icons: Vec<TextureHandle>,
    glow: TextureHandle,

    managed: Vec<Option<Manifest>>,
    managed_size: Vec<u64>,
    external: Vec<Option<External>>,
    releases: Vec<Option<Cached>>,
    release_errors: Vec<Option<String>>,
    checking: usize,
    jobs: Vec<Option<Job>>,
    moving: bool,
    processes: Processes,

    path_edit: Vec<String>,
    dir_edit: String,
    dialog: Option<Dialog>,
    toasts: Vec<Toast>,
    events: (Sender<Event>, Receiver<Event>),
    ctx: egui::Context,
    gpu: Option<GpuInfo>,
    /// Whether this window has the launcher's own title bar (fixed at startup).
    custom_frame: bool,
    handoff: titlebar::Handoff,
    /// Screenshot builds: open this app's More menu on the app page.
    #[cfg(feature = "screenshot")]
    demo_menu: Option<usize>,
}

/// Sends events from worker threads and wakes the UI.
#[derive(Clone)]
struct Notify {
    tx: Sender<Event>,
    ctx: egui::Context,
}

impl Notify {
    fn send(&self, e: Event) {
        let _ = self.tx.send(e);
        self.ctx.request_repaint();
    }
}

impl Launcher {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install(&cc.egui_ctx);

        let ui_state: UiState = cc
            .storage
            .and_then(|s| eframe::get_value(s, STORAGE_KEY))
            .unwrap_or_default();
        let settings = Settings::load();

        let icons = APPS
            .iter()
            .map(|app| {
                let img = image::load_from_memory_with_format(app.icon_webp, image::ImageFormat::WebP)
                    .expect("bundled icon decodes")
                    .to_rgba8();
                let size = [img.width() as usize, img.height() as usize];
                let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
                let opts = egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear));
                cc.egui_ctx.load_texture(app.id, color, opts)
            })
            .collect();

        let gpu = cc.wgpu_render_state.as_ref().map(|rs| GpuInfo {
            active: rs.adapter.get_info(),
            all: rs.available_adapters.iter().map(|a| a.get_info()).collect(),
        });

        let cache = releases::load_cache();
        let n = APPS.len();
        let mut launcher = Self {
            custom_frame: !settings.native_title_bar,
            handoff: titlebar::Handoff::default(),
            #[cfg(feature = "screenshot")]
            demo_menu: None,
            path_edit: APPS
                .iter()
                .map(|a| settings.custom_paths.get(a.id).cloned().unwrap_or_default())
                .collect(),
            dir_edit: settings::display_path(&settings.install_dir),
            ui_state,
            settings,
            icons,
            glow: theme::glow_texture(&cc.egui_ctx),
            managed: (0..n).map(|_| None).collect(),
            managed_size: vec![0; n],
            external: (0..n).map(|_| None).collect(),
            releases: APPS.iter().map(|a| cache.get(a.id).cloned()).collect(),
            release_errors: vec![None; n],
            checking: 0,
            jobs: (0..n).map(|_| None).collect(),
            moving: false,
            processes: Processes::default(),
            dialog: None,
            toasts: Vec::new(),
            events: channel(),
            ctx: cc.egui_ctx.clone(),
            gpu,
        };

        if let Err(e) = std::fs::create_dir_all(&launcher.settings.install_dir) {
            launcher.toast(format!(
                "Couldn't create {}: {e}",
                settings::display_path(&launcher.settings.install_dir)
            ));
        }
        for app in &APPS {
            installer::tidy(&launcher.settings.install_dir.join(app.id));
        }
        launcher.rescan();

        let stale = launcher
            .releases
            .iter()
            .any(|r| r.as_ref().is_none_or(|c| releases::now().saturating_sub(c.checked) > STARTUP_RECHECK));
        if launcher.settings.check_on_startup && stale {
            launcher.check_updates();
        }
        #[cfg(feature = "screenshot")]
        launcher.demo_action();
        launcher
    }

    /// Screenshot builds only: `LANDINGCRAFT_DEMO=<action>:<app id or path>` runs
    /// one action at startup so its UI can be captured without clicking.
    #[cfg(feature = "screenshot")]
    fn demo_action(&mut self) {
        let Ok(spec) = std::env::var("LANDINGCRAFT_DEMO") else { return };
        let (action, arg) = spec.split_once(':').unwrap_or((spec.as_str(), ""));
        let idx = APPS.iter().position(|a| a.id == arg);
        match (action, idx) {
            ("install", Some(i)) => self.install(i),
            ("uninstall", Some(i)) => self.dialog = Some(Dialog::Uninstall(i)),
            ("menu", Some(i)) => self.demo_menu = Some(i),
            ("move", _) => self.request_install_dir(PathBuf::from(arg)),
            _ => eprintln!("unknown LANDINGCRAFT_DEMO {spec:?}"),
        }
    }

    fn notify(&self) -> Notify {
        Notify { tx: self.events.0.clone(), ctx: self.ctx.clone() }
    }

    fn base(&self) -> &Path {
        &self.settings.install_dir
    }

    fn save_settings(&mut self) {
        if let Err(e) = self.settings.save() {
            self.toast(format!("Couldn't save settings: {e}"));
        }
    }

    fn rescan(&mut self) {
        for (i, app) in APPS.iter().enumerate() {
            self.managed[i] = installer::read_manifest(&self.settings.install_dir, app.id);
            self.managed_size[i] = if self.managed[i].is_some() {
                installer::disk_usage(&self.settings.install_dir.join(app.id))
            } else {
                0
            };
            self.external[i] = match self.settings.custom_paths.get(app.id).map(PathBuf::from) {
                Some(p) if detect::is_executable(&p) => Some(External { path: p, custom: true }),
                _ => detect::find(app.id).map(|path| External { path, custom: false }),
            };
        }
    }

    // ---------------------------------------------------------------- queries

    /// Executable that "Open" should start: managed copy first, then others.
    fn launch_path(&self, idx: usize) -> Option<PathBuf> {
        if let Some(m) = &self.managed[idx] {
            let exe = m.exe_path(self.base());
            return detect::is_executable(&exe).then_some(exe);
        }
        self.external[idx].as_ref().map(|e| e.path.clone())
    }

    fn is_installed(&self, idx: usize) -> bool {
        self.managed[idx].is_some() || self.external[idx].is_some()
    }

    fn installed_count(&self) -> usize {
        (0..APPS.len()).filter(|&i| self.is_installed(i)).count()
    }

    fn choice(&self, idx: usize) -> Option<releases::Choice<'_>> {
        let cached = self.releases[idx].as_ref()?;
        releases::latest_compatible(APPS[idx].id, &cached.releases, self.settings.format)
    }

    fn latest_version(&self, idx: usize) -> Option<Version> {
        self.choice(idx).map(|c| c.release.version.clone())
    }

    /// Newer stable version than the managed one, if any.
    fn update_for(&self, idx: usize) -> Option<Version> {
        let installed = &self.managed[idx].as_ref()?.version;
        self.latest_version(idx).filter(|v| v > installed)
    }

    fn updates(&self) -> Vec<usize> {
        (0..APPS.len()).filter(|&i| self.update_for(i).is_some()).collect()
    }

    fn busy(&self, idx: usize) -> bool {
        self.jobs[idx].is_some() || self.moving
    }

    fn running(&self, idx: usize) -> bool {
        self.processes.running(APPS[idx].id) > 0
    }

    fn last_checked(&self) -> Option<u64> {
        self.releases.iter().filter_map(|r| r.as_ref().map(|c| c.checked)).max()
    }

    // ---------------------------------------------------------------- actions

    fn toast(&mut self, text: impl Into<String>) {
        let now = self.ctx.input(|i| i.time);
        self.toasts.push(Toast { text: text.into(), until: now + 5.0 });
        self.ctx.request_repaint();
    }

    fn open_url(&self, url: String) {
        let notify = self.notify();
        std::thread::spawn(move || {
            if let Err(e) = portal::open_uri(&url) {
                notify.send(Event::LinkFailed { url, err: e.to_string() });
            }
        });
    }

    fn show_folder(&mut self, path: &Path) {
        if let Err(e) = portal::show_folder(path) {
            self.ctx.copy_text(path.display().to_string());
            self.toast(format!("Couldn't open the file manager ({e}). Path copied to the clipboard."));
        }
    }

    fn launch(&mut self, idx: usize) {
        let app = &APPS[idx];
        let Some(path) = self.launch_path(idx) else {
            self.toast(format!("{} is missing its executable; reinstall it from the More menu", app.name()));
            return;
        };
        match self.processes.launch(app.id, &path) {
            Ok(()) => self.toast(format!("Starting {}…", app.name())),
            Err(e) => self.toast(format!("Couldn't start {}: {e}", app.name())),
        }
    }

    fn check_updates(&mut self) {
        if self.checking > 0 {
            return;
        }
        for (idx, app) in APPS.iter().enumerate() {
            let previous = self.releases[idx].clone();
            let notify = self.notify();
            let id = app.id;
            self.checking += 1;
            std::thread::spawn(move || {
                let result = releases::fetch(id, previous.as_ref());
                notify.send(Event::Releases { idx, result });
            });
        }
    }

    /// Install the latest compatible stable release, or update/reinstall to it.
    fn install(&mut self, idx: usize) {
        let app = &APPS[idx];
        if self.busy(idx) {
            return;
        }
        if self.running(idx) && self.managed[idx].is_some() {
            self.toast(format!("Close {} before updating it", app.name()));
            return;
        }
        let Some(choice) = self.choice(idx) else {
            self.toast(format!("No stable {} build for Linux {} is available yet", app.name(), releases::arch()));
            return;
        };
        let (release, asset, format): (Release, Asset, _) =
            (choice.release.clone(), choice.asset.clone(), choice.format);
        let base = self.settings.install_dir.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        self.jobs[idx] = Some(Job {
            version: release.version.clone(),
            uninstalling: false,
            stage: Stage::Downloading,
            done: 0,
            total: asset.size,
            cancel: cancel.clone(),
        });

        let notify = self.notify();
        std::thread::spawn(move || {
            let mut last = Instant::now() - Duration::from_secs(1);
            let mut last_stage = None;
            let mut progress = |stage: Stage, done: u64, total: u64| {
                // Throttle to ~30 updates a second.
                if last_stage != Some(stage) || done == total || last.elapsed() >= Duration::from_millis(33) {
                    last = Instant::now();
                    last_stage = Some(stage);
                    notify.send(Event::Progress { idx, stage, done, total });
                }
            };
            let result = installer::install(&base, APPS[idx].id, &release, &asset, format, &cancel, &mut progress);
            notify.send(Event::Installed { idx, result });
        });
    }

    fn update_all(&mut self) {
        for idx in self.updates() {
            if !self.running(idx) {
                self.install(idx);
            }
        }
    }

    fn cancel(&mut self, idx: usize) {
        if let Some(job) = &self.jobs[idx] {
            job.cancel.store(true, Ordering::Relaxed);
        }
    }

    fn uninstall(&mut self, idx: usize) {
        if self.busy(idx) {
            return;
        }
        let base = self.settings.install_dir.clone();
        let notify = self.notify();
        // Show as busy while the folder is deleted.
        self.jobs[idx] = Some(Job {
            version: self.managed[idx].as_ref().map_or(Version::new(0, 0, 0), |m| m.version.clone()),
            uninstalling: true,
            stage: Stage::Finishing,
            done: 0,
            total: 0,
            cancel: Arc::new(AtomicBool::new(false)),
        });
        std::thread::spawn(move || {
            let result = installer::uninstall(&base, APPS[idx].id);
            notify.send(Event::Uninstalled { idx, result });
        });
    }

    fn pick_install_dir(&mut self) {
        let start = self.settings.install_dir.clone();
        let notify = self.notify();
        std::thread::spawn(move || {
            let start = if start.is_dir() { start } else { settings::home() };
            let result = portal::pick_folder("Choose where Crafting Apps are installed", &start)
                .map_err(|e| e.to_string());
            notify.send(Event::FolderPicked(result));
        });
    }

    /// Switch the install folder, moving managed apps if there are any.
    fn request_install_dir(&mut self, to: PathBuf) {
        let from = self.settings.install_dir.clone();
        if to == from {
            return;
        }
        if to.starts_with(&from) && APPS.iter().any(|a| to.starts_with(from.join(a.id))) {
            self.toast("The new folder can't be inside an app's own folder");
            return;
        }
        if self.jobs.iter().any(Option::is_some) || self.moving {
            self.toast("Wait for installs to finish before changing the folder");
            return;
        }
        let apps: Vec<usize> = (0..APPS.len()).filter(|&i| self.managed[i].is_some()).collect();
        if apps.is_empty() {
            self.apply_install_dir(to);
        } else {
            let bytes = apps.iter().map(|&i| self.managed_size[i]).sum();
            self.dialog = Some(Dialog::Move { to, apps, bytes, progress: None });
        }
    }

    fn start_move(&mut self) {
        let Some(Dialog::Move { to, apps, progress, .. }) = &mut self.dialog else { return };
        if apps.iter().any(|&i| self.processes.running(APPS[i].id) > 0) {
            self.toast("Close running apps before moving them");
            return;
        }
        *progress = Some(0);
        self.moving = true;
        let from = self.settings.install_dir.clone();
        let to = to.clone();
        let ids: Vec<&'static str> = apps.iter().map(|&i| APPS[i].id).collect();
        let notify = self.notify();
        std::thread::spawn(move || {
            let mut report = |done| notify.send(Event::MoveProgress { done });
            let result = installer::move_installs(&from, &to, &ids, &mut report);
            notify.send(Event::Moved { to, result });
        });
    }

    fn apply_install_dir(&mut self, to: PathBuf) {
        if let Err(e) = std::fs::create_dir_all(&to) {
            self.toast(format!("Couldn't use {}: {e}", to.display()));
            return;
        }
        self.settings.install_dir = to;
        self.dir_edit = settings::display_path(&self.settings.install_dir);
        self.save_settings();
        self.rescan();
        self.toast(format!("Apps now install to {}", self.dir_edit));
    }

    fn set_custom_path(&mut self, idx: usize) {
        let app = &APPS[idx];
        let raw = self.path_edit[idx].trim().to_owned();
        if raw.is_empty() {
            self.settings.custom_paths.remove(app.id);
        } else {
            match settings::parse_user_path(&raw) {
                Some(p) if detect::is_executable(&p) => {
                    self.settings.custom_paths.insert(app.id.to_owned(), p.display().to_string());
                    self.toast(format!("{} will use {raw} when the launcher doesn't manage it", app.name()));
                }
                _ => {
                    self.toast(format!("{raw} isn't an executable file"));
                    return;
                }
            }
        }
        self.save_settings();
        self.rescan();
    }

    // ----------------------------------------------------------------- events

    fn handle_events(&mut self) {
        while let Ok(event) = self.events.1.try_recv() {
            match event {
                Event::Releases { idx, result } => {
                    self.checking = self.checking.saturating_sub(1);
                    match result {
                        Ok(cached) => {
                            self.releases[idx] = Some(cached);
                            self.release_errors[idx] = None;
                        }
                        Err(e) => self.release_errors[idx] = Some(e),
                    }
                    if self.checking == 0 {
                        let cache: HashMap<String, Cached> = APPS
                            .iter()
                            .zip(&self.releases)
                            .filter_map(|(a, r)| r.clone().map(|r| (a.id.to_owned(), r)))
                            .collect();
                        releases::save_cache(&cache);
                        let n = self.updates().len();
                        if n > 0 {
                            self.toast(format!("{n} update{} available", if n == 1 { "" } else { "s" }));
                        }
                    }
                }
                Event::Progress { idx, stage, done, total } => {
                    if let Some(job) = &mut self.jobs[idx] {
                        job.stage = stage;
                        job.done = done;
                        job.total = total;
                    }
                }
                Event::Installed { idx, result } => {
                    let app = &APPS[idx];
                    let was = self.managed[idx].as_ref().map(|m| m.version.clone());
                    self.jobs[idx] = None;
                    match result {
                        Ok(m) => {
                            let msg = match was {
                                Some(v) if v < m.version => format!("{} updated to v{}", app.name(), m.version),
                                Some(_) => format!("{} v{} reinstalled", app.name(), m.version),
                                None => format!("{} v{} installed", app.name(), m.version),
                            };
                            self.toast(msg);
                        }
                        Err(e) if e == installer::CANCELLED => self.toast(format!("{} install cancelled", app.name())),
                        Err(e) => self.toast(format!("{}: {e}", app.name())),
                    }
                    self.rescan();
                }
                Event::Uninstalled { idx, result } => {
                    self.jobs[idx] = None;
                    match result {
                        Ok(()) => self.toast(format!("{} uninstalled", APPS[idx].name())),
                        Err(e) => self.toast(format!("Couldn't uninstall {}: {e}", APPS[idx].name())),
                    }
                    self.rescan();
                }
                Event::MoveProgress { done } => {
                    if let Some(Dialog::Move { progress, .. }) = &mut self.dialog {
                        *progress = Some(done);
                    }
                }
                Event::Moved { to, result } => {
                    self.moving = false;
                    self.dialog = None;
                    match result {
                        Ok(()) => self.apply_install_dir(to),
                        Err(e) => {
                            self.toast(format!("Apps were left where they were: {e}"));
                            self.rescan();
                        }
                    }
                }
                Event::FolderPicked(Ok(Some(path))) => self.request_install_dir(path),
                Event::FolderPicked(Ok(None)) => {}
                Event::FolderPicked(Err(e)) => {
                    self.toast(format!("The folder picker isn't available ({e}); type a path instead"));
                }
                Event::LinkFailed { url, err } => {
                    self.ctx.copy_text(url.clone());
                    self.toast(format!("Couldn't open a browser ({err}). Link copied: {url}"));
                }
            }
        }
    }

    fn show_toasts(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        self.toasts.retain(|t| t.until > now);
        if self.toasts.is_empty() {
            return;
        }
        egui::Area::new(egui::Id::new("toasts"))
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-20.0, -20.0))
            .order(egui::Order::Tooltip)
            .interactable(false)
            .show(ctx, |ui| {
                for t in &self.toasts {
                    let fade = ((t.until - now) as f32 / 0.4).clamp(0.0, 1.0);
                    egui::Frame::new()
                        .fill(theme::SURFACE_HI.gamma_multiply(fade))
                        .stroke(Stroke::new(1.0, theme::BORDER.gamma_multiply(fade)))
                        .corner_radius(8)
                        .inner_margin(egui::vec2(14.0, 10.0))
                        .shadow(egui::Shadow {
                            offset: [0, 6],
                            blur: 18,
                            spread: 0,
                            color: egui::Color32::from_black_alpha((90.0 * fade) as u8),
                        })
                        .show(ui, |ui| {
                            ui.set_max_width(420.0);
                            ui.label(egui::RichText::new(&t.text).color(theme::TEXT.gamma_multiply(fade)));
                        });
                }
            });
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

impl eframe::App for Launcher {
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        let pos = ctx.input(|i| i.pointer.latest_pos()).unwrap_or_default();
        self.handoff.patch(raw_input, pos);
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_events();
        self.processes.poll();
        if self.processes.any_running() {
            ctx.request_repaint_after(Duration::from_millis(500));
        }
        if self.settings.check_on_startup
            && self.checking == 0
            && self.last_checked().is_some_and(|t| releases::now().saturating_sub(t) > BACKGROUND_RECHECK)
        {
            self.check_updates();
        }
        if self.checking > 0 || self.jobs.iter().any(Option::is_some) {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        if self.custom_frame {
            egui::Panel::top("titlebar")
                .exact_size(titlebar::HEIGHT)
                .resizable(false)
                .show_separator_line(false)
                .frame(egui::Frame::new().fill(theme::SIDEBAR))
                .show(ui, |ui| self.title_bar(ui));
        }
        egui::Panel::left("sidebar")
            .exact_size(240.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(theme::SIDEBAR)
                    .inner_margin(egui::Margin { left: 8, right: 8, top: 0, bottom: 0 })
                    .stroke(Stroke::new(1.0, theme::BORDER)),
            )
            .show(ui, |ui| self.sidebar(ui));

        egui::CentralPanel::no_frame()
            .frame(egui::Frame::new().fill(theme::BG))
            .show(ui, |ui| {
                let page = self.ui_state.page;
                ScrollArea::vertical()
                    .id_salt(match page {
                        Page::App(i) => i + 10,
                        Page::All => 0,
                        Page::Installed => 1,
                        Page::Settings => 2,
                    })
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        egui::Frame::new()
                            .inner_margin(egui::Margin { left: 40, right: 40, top: 0, bottom: 0 })
                            .show(ui, |ui| match page {
                                Page::All => self.home(ui, false),
                                Page::Installed => self.home(ui, true),
                                Page::App(i) => self.app_page(ui, i.min(APPS.len() - 1)),
                                Page::Settings => self.settings_page(ui),
                            });
                    });
            });

        if self.custom_frame {
            self.window_edges(ui);
        }
        let ctx = ui.ctx().clone();
        self.dialogs(&ctx);
        self.show_toasts(&ctx);
        #[cfg(feature = "screenshot")]
        crate::screenshot::tick(&ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, STORAGE_KEY, &self.ui_state);
    }

    fn on_exit(&mut self) {
        // Stop downloads promptly; their partial files are cleaned up next start.
        for job in self.jobs.iter().flatten() {
            job.cancel.store(true, Ordering::Relaxed);
        }
    }
}
