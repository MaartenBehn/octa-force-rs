use std::fs::{self, File};

use log::{LevelFilter, Log, Metadata};
use simplelog::{ColorChoice, CombinedLogger, Config, ConfigBuilder, TermLogger, TerminalMode, WriteLogger};
use crate::OctaResult;

pub fn log_init() -> OctaResult<()> {
    let config = ConfigBuilder::new()
        .set_level_padding(simplelog::LevelPadding::Right)
        .set_location_level(LevelFilter::Off)
        .set_target_level(LevelFilter::Off)
        //.set_target_level(LevelFilter::Debug)
        .set_thread_level(LevelFilter::Off)
        .add_filter_ignore("sctk".to_string())
        .add_filter_ignore("egui_ash_renderer".to_string())
        .add_filter_ignore("egui_winit".to_string())
        .add_filter_ignore("egui".to_string())
        .add_filter_ignore("calloop".to_string())
        .add_filter_ignore("arboard".to_string())
        .add_filter_ignore("notify".to_string())
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
