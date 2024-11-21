use log::{LevelFilter, Log};
use simplelog::{ColorChoice, Config, TermLogger, TerminalMode};
use crate::OctaResult;

pub fn log_init() -> OctaResult<()> {
    #[cfg(debug_assertions)]
    let log_level = LevelFilter::Debug;

    #[cfg(not(debug_assertions))]
    let log_level = LevelFilter::Info;

    TermLogger::init(
        log_level,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )?;
    
    Ok(())
}

pub fn setup_logger(
    logger: &'static dyn Log,
) -> OctaResult<()> {
    #[cfg(debug_assertions)]
    let log_level = LevelFilter::Debug;

    #[cfg(not(debug_assertions))]
    let log_level = LevelFilter::Info;
    
    log::set_max_level(log_level);
    log::set_logger(logger)?;
    Ok(())
}