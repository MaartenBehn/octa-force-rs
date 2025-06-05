use libloading::Symbol;
use log::Log;
use crate::hot_reloading::lib_reloader::LibReloader;
use crate::OctaResult;

pub mod lib_reloader;
pub mod codesign;

#[derive(Clone, Debug)]
pub struct HotReloadConfig{
    pub lib_dir: String, 
    pub lib_name: String
}

pub struct HotReloadController {
    pub lib_reloader: LibReloader,
    pub active: bool
}

impl HotReloadController {
    pub fn new(hot_reload_config: HotReloadConfig) -> OctaResult<Self> {
        let lib_reloader = LibReloader::new(
            hot_reload_config.lib_dir,
            hot_reload_config.lib_name, None, None)?;
        
        unsafe {
            let call: Symbol<unsafe extern "C" fn(&'static dyn Log) -> OctaResult<()>> =
                lib_reloader.get_symbol("init_hot_reload")?;
            call(log::logger())?;
        }
        
        Ok(HotReloadController {
            lib_reloader,
            active: false,
        })
    }
}
