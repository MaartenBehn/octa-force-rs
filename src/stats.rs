use crate::Queue;
use std::time::Duration;
use egui::Align2;
#[cfg(debug_assertions)]
use puffin_egui::puffin;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StatsDisplayMode {
    None,
    Basic,
    Full,
}

impl StatsDisplayMode {
    pub(crate) fn next(self) -> Self {
        match self {
            Self::None => Self::Basic,
            Self::Basic => Self::Full,
            Self::Full => Self::None,
        }
    }
}

#[derive(Debug)]
pub(crate) struct FrameStats {
    // we collect gpu timings the frame after it was computed
    // so we keep frame times for the two last frames
    previous_frame_time: Duration,
    pub(crate) frame_time: Duration,
    previous_compute_time: Duration,
    pub(crate) compute_time: Duration,
    pub(crate) gpu_time: Duration,
    frame_time_ms_log: Queue<f32>,
    compute_time_ms_log: Queue<f32>,
    gpu_time_ms_log: Queue<f32>,
    pub(crate) total_frame_count: usize,
    frame_count: usize,
    fps_counter: usize,
    timer: Duration,
    pub(crate) stats_display_mode: StatsDisplayMode,
}

impl FrameStats {
    const ONE_SEC: Duration = Duration::from_secs(1);
    const MAX_LOG_SIZE: usize = 1000;

    pub fn new() -> Self {
        #[cfg(debug_assertions)]
        puffin::set_scopes_on(true);

        Self {
            previous_frame_time: Default::default(),
            frame_time: Default::default(),
            previous_compute_time: Default::default(),
            compute_time: Default::default(),
            gpu_time: Default::default(),
            frame_time_ms_log: Queue::new(FrameStats::MAX_LOG_SIZE),
            compute_time_ms_log: Queue::new(FrameStats::MAX_LOG_SIZE),
            gpu_time_ms_log: Queue::new(FrameStats::MAX_LOG_SIZE),
            total_frame_count: Default::default(),
            frame_count: Default::default(),
            fps_counter: Default::default(),
            timer: Default::default(),
            stats_display_mode: StatsDisplayMode::None,
        }
    }

    pub(crate) fn toggle_stats(&mut self) {
        self.stats_display_mode = self.stats_display_mode.next();
    }

    pub(crate) fn tick(&mut self) {
        // push log
        self.frame_time_ms_log
            .push(self.previous_frame_time.as_millis() as _);
        self.compute_time_ms_log
            .push(self.previous_compute_time.as_millis() as _);
        self.gpu_time_ms_log.push(self.gpu_time.as_millis() as _);

        // increment counter
        self.total_frame_count += 1;
        self.frame_count += 1;
        self.timer += self.frame_time;

        // reset counter if a sec has passed
        if self.timer > FrameStats::ONE_SEC {
            self.fps_counter = self.frame_count;
            self.frame_count = 0;
            self.timer -= FrameStats::ONE_SEC;
        }
    }

    pub(crate) fn set_cpu_time(&mut self, frame_time: Duration, compute_time: Duration) {
        self.previous_frame_time = self.frame_time;
        self.previous_compute_time = self.compute_time;

        self.frame_time = frame_time;
        self.compute_time = compute_time;
    }

    pub(crate) fn set_gpu_time(&mut self, gpu_time: Duration) {
        self.gpu_time = gpu_time;
    }

    pub(crate) fn build_perf_ui(&mut self, ctx: &egui::Context) {
        if matches!(
            self.stats_display_mode,
            StatsDisplayMode::Basic | StatsDisplayMode::Full
        ) {
            egui::Window::new("Frame stats")
                .anchor(Align2::RIGHT_TOP, [-5.0, 5.0])
                .collapsible(false)
                .interactable(false)
                .resizable(false)
                .drag_to_scroll(false)
                .fixed_size(&[200.0, 100.0])
                .show(ctx, |ui| {
                    ui.set_width(ui.available_width());
                    ui.set_height(ui.available_height());

                    ui.label("Framerate");
                    ui.label(format!("{} fps", self.fps_counter));
                    ui.label("Frametimes");
                    ui.label(format!("All - {:?}", self.frame_time));
                    ui.label(format!("CPU - {:?}", self.compute_time));
                    ui.label(format!("GPU - {:?}", self.gpu_time));
                });
        }

        if matches!(self.stats_display_mode, StatsDisplayMode::Full) {
            egui::TopBottomPanel::bottom("frametime_graphs").show(ctx, |ui| {
                ui.add_space(5.0);
                ui.label("All in ms");
                build_frametime_plot(ui, "Frames", &self.frame_time_ms_log.0);
                ui.add_space(5.0);
                ui.label("CPU in ms");
                build_frametime_plot(ui, "CPU", &self.compute_time_ms_log.0);
                ui.add_space(5.0);
                ui.label("GPU in ms");
                build_frametime_plot(ui, "GPU", &self.gpu_time_ms_log.0);
                ui.add_space(5.0);
            });

            #[cfg(debug_assertions)]
            {
                puffin_egui::profiler_window(ctx);
                puffin::GlobalProfiler::lock().new_frame();
            }
        }
    }
}

fn build_frametime_plot(ui: &mut egui::Ui, id: impl std::hash::Hash, points: &[f32]) {
    let points: egui_plot::PlotPoints = points
        .iter()
        .enumerate()
        .map(|(i, v)| [i as f64, *v as f64])
        .collect();

    egui_plot::Plot::new(id)
        // .width(width)
        .height(80.0)
        .allow_boxed_zoom(false)
        .allow_double_click_reset(false)
        .allow_drag(false)
        .allow_scroll(false)
        .allow_zoom(false)
        .show_axes([false, true])
        .show(ui, |plot| {
            plot.line(egui_plot::Line::new(points));
        });
}
