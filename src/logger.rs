use std::fs::{self, File};

use log::{LevelFilter, Log};
use simplelog::{ColorChoice, CombinedLogger, Config, ConfigBuilder, TermLogger, TerminalMode, WriteLogger};
use crate::OctaResult;

pub fn log_init() -> OctaResult<()> {
    let config = ConfigBuilder::new()
        .set_level_padding(simplelog::LevelPadding::Right)
        .set_location_level(LevelFilter::Off)
        .set_target_level(LevelFilter::Off)
        //.set_target_level(LevelFilter::Debug)
        .set_thread_level(LevelFilter::Off)
        .add_filter_ignore(format!("{}", "sctk"))
        .add_filter_ignore(format!("{}", "egui_ash_renderer"))
        .add_filter_ignore(format!("{}", "egui_winit"))
        .add_filter_ignore(format!("{}", "egui"))
        .add_filter_ignore(format!("{}", "calloop"))
        .add_filter_ignore(format!("{}", "arboard"))
        .set_time_to_local(true)
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

pub fn setup_logger(
    logger: &'static dyn Log,
) -> OctaResult<()> {
    log::set_logger(logger)?;
    Ok(())
}
