use crate::Queue;
use ash::vk::Extent2D;
use imgui::{Condition, Ui};
use std::time::Duration;

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
    pub(crate) total_frame_count: u32,
    frame_count: u32,
    fps_counter: u32,
    timer: Duration,
}

impl Default for FrameStats {
    fn default() -> Self {
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
        }
    }
}

impl FrameStats {
    const ONE_SEC: Duration = Duration::from_secs(1);
    const MAX_LOG_SIZE: usize = 1000;

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

    pub(crate) fn set_frame_time(&mut self, frame_time: Duration, compute_time: Duration) {
        self.previous_frame_time = self.frame_time;
        self.previous_compute_time = self.compute_time;

        self.frame_time = frame_time;
        self.compute_time = compute_time;
    }

    pub(crate) fn set_gpu_time_time(&mut self, gpu_time: Duration) {
        self.gpu_time = gpu_time;
    }

    pub(crate) fn build_perf_ui(&mut self, ui: &Ui, mode: StatsDisplayMode, extent: Extent2D) {
        let width = extent.width as f32;
        let height = extent.height as f32;

        if matches!(mode, StatsDisplayMode::Basic | StatsDisplayMode::Full) {
            ui.window("Frame stats")
                .focus_on_appearing(false)
                .no_decoration()
                .bg_alpha(0.5)
                .position([5.0, 5.0], Condition::Always)
                .size([160.0, 140.0], Condition::FirstUseEver)
                .build(|| {
                    ui.text("Framerate");
                    ui.label_text("fps", self.fps_counter.to_string());
                    ui.text("Frametimes");
                    ui.label_text("Frame", format!("{:?}", self.frame_time));
                    ui.label_text("CPU", format!("{:?}", self.compute_time));
                    ui.label_text("GPU", format!("{:?}", self.gpu_time));
                });
        }

        if matches!(mode, StatsDisplayMode::Full) {
            let graph_size = [width - 80.0, 40.0];
            const SCALE_MIN: f32 = 0.0;
            const SCALE_MAX: f32 = 17.0;

            ui.window("Frametime graphs")
                .focus_on_appearing(false)
                .no_decoration()
                .bg_alpha(0.5)
                .position([5.0, height - 145.0], Condition::Always)
                .size([width - 10.0, 140.0], Condition::Always)
                .build(|| {
                    ui.plot_lines("Frame", &self.frame_time_ms_log.0)
                        .scale_min(SCALE_MIN)
                        .scale_max(SCALE_MAX)
                        .graph_size(graph_size)
                        .build();
                    ui.plot_lines("CPU", &self.compute_time_ms_log.0)
                        .scale_min(SCALE_MIN)
                        .scale_max(SCALE_MAX)
                        .graph_size(graph_size)
                        .build();
                    ui.plot_lines("GPU", &self.gpu_time_ms_log.0)
                        .scale_min(SCALE_MIN)
                        .scale_max(SCALE_MAX)
                        .graph_size(graph_size)
                        .build();
                });
        }
    }
}
