use std::fs::{self, File};

use log::{LevelFilter, Log};
use simplelog::{ColorChoice, CombinedLogger, Config, TermLogger, TerminalMode, WriteLogger};
use crate::OctaResult;

pub fn log_init() -> OctaResult<()> {
    #[cfg(debug_assertions)]
    {
        let _ = fs::remove_file("trace.log"); 
        CombinedLogger::init(vec![
            TermLogger::new(
                LevelFilter::Debug,
                Config::default(),
                TerminalMode::Mixed,
                ColorChoice::Auto,
            ),
            WriteLogger::new(
                LevelFilter::Debug, 
                Config::default(), 
                File::create("trace.log")?
            ),
        ])?;
    }

    #[cfg(not(debug_assertions))]
    {
        TermLogger::init(
            LevelFilter::Info,
            Config::default(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        )?; 
    }
        
    Ok(())
}

pub fn setup_logger(
    logger: &'static dyn Log,
) -> OctaResult<()> {
    #[cfg(debug_assertions)]
    let log_level = LevelFilter::Debug;

    #[cfg(not(debug_assertions))]
    let log_level = LevelFilter::Info;
    
    log::set_logger(logger)?;
    Ok(())
}
