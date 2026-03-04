use std::fs::{self, File};

use log::{LevelFilter, Log};
use simplelog::{ColorChoice, CombinedLogger, ConfigBuilder, TermLogger, TerminalMode, WriteLogger};
use crate::OctaResult;

pub fn log_init() -> OctaResult<()> {
    let config = ConfigBuilder::new()
        .set_level_padding(simplelog::LevelPadding::Right)
        .set_location_level(LevelFilter::Off)
        .set_thread_level(LevelFilter::Off)
        .set_target_level(LevelFilter::Off)
        //.set_target_level(LevelFilter::Error) // to findout which module loggs stuff
        .add_filter_ignore_str("sctk")
        .add_filter_ignore_str("egui_ash_renderer")
        .add_filter_ignore_str("egui_winit")
        .add_filter_ignore_str("tracing::span")
        .add_filter_ignore_str("winit::window")
        .add_filter_ignore_str("egui")
        .add_filter_ignore_str("calloop")
        .add_filter_ignore_str("arboard")
        .add_filter_ignore_str("notify")
        .set_time_offset_to_local().expect("Failed to set Time Zone!")
        .build();

    let _ = fs::remove_file("trace.log"); 
    CombinedLogger::init(vec![
        TermLogger::new(
            LevelFilter::Debug,
            config.to_owned(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
        WriteLogger::new(
            LevelFilter::Trace, 
            config, 
            File::create("trace.log")?
        ),
    ])?;
    
    Ok(())
}

pub fn setup_logger(logger: &'static dyn Log, level: LevelFilter) -> OctaResult<()> {
    log::set_max_level(level);
    log::set_logger(logger)?;
    Ok(())
}
