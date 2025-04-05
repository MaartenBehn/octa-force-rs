use libloading::{Library, Symbol};
use notify::RecursiveMode;
use notify_debouncer_full::new_debouncer;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;
use anyhow::anyhow;

#[cfg(feature = "verbose")]
use log;
use crate::OctaResult;

/// Manages watches a library (dylib) file, loads it using
/// [`libloading::Library`] and [provides access to its
/// symbols](LibReloader::get_symbol). When the library changes, [`LibReloader`]
/// is able to unload the old version and reload the new version through
/// [`LibReloader::update`].
///
/// Note that the [`LibReloader`] itself will not actively update, i.e. does not
/// manage an update thread calling the update function. This is normally
/// managed by the [`hot_lib_reloader_macro::hot_module`] macro that also
/// manages the [about-to-load and load](crate::LibReloadNotifier) notifications.
///
/// It can load symbols from the library with [LibReloader::get_symbol].
pub struct LibReloader {
    load_counter: usize,
    lib_dir: PathBuf,
    lib_name: String,
    changed: Arc<AtomicBool>,
    lib: Option<Library>,
    watched_lib_file: PathBuf,
    loaded_lib_file: PathBuf,
    lib_file_hash: Arc<AtomicU32>,
    #[cfg(target_os = "macos")]
    codesigner: crate::codesign::CodeSigner,
    loaded_lib_name_template: Option<String>,
}

impl LibReloader {
    /// Creates a LibReloader.
    ///  `lib_dir` is expected to be the location where the library to use can
    /// be found. Probably `target/debug` normally.
    /// `lib_name` is the name of the library, not(!) the file name. It should
    /// normally be just the crate name of the cargo project you want to hot-reload.
    /// LibReloader will take care to figure out the actual file name with
    /// platform-specific prefix and extension.
    pub fn new(
        lib_dir: impl AsRef<Path>,
        lib_name: impl AsRef<str>,
        file_watch_debounce: Option<Duration>,
        loaded_lib_name_template: Option<String>,
    ) -> OctaResult<Self> {
        // find the target dir in which the build is happening and where we should find
        // the library
        let lib_dir = find_file_or_dir_in_parent_directories(lib_dir.as_ref())?;
        log::debug!("found lib dir at {lib_dir:?}");

        let load_counter = 0;

        #[cfg(target_os = "macos")]
        let codesigner = crate::codesign::CodeSigner::new();

        let (watched_lib_file, loaded_lib_file) = watched_and_loaded_library_paths(
            &lib_dir,
            &lib_name,
            load_counter,
            &loaded_lib_name_template,
        );

        let (lib_file_hash, lib) = if watched_lib_file.exists() {
            // We don't load the actual lib because this can get problems e.g. on Windows
            // where a file lock would be held, preventing the lib from changing later.
            log::debug!("copying {watched_lib_file:?} -> {loaded_lib_file:?}");
            fs::copy(&watched_lib_file, &loaded_lib_file)?;
            let hash = hash_file(&loaded_lib_file);
            #[cfg(target_os = "macos")]
            codesigner.codesign(&loaded_lib_file);
            (hash, Some(load_library(&loaded_lib_file)?))
        } else {
            log::debug!("library {watched_lib_file:?} does not yet exist");
            (0, None)
        };

        let lib_file_hash = Arc::new(AtomicU32::new(lib_file_hash));
        let changed = Arc::new(AtomicBool::new(false));
        Self::watch(
            watched_lib_file.clone(),
            lib_file_hash.clone(),
            changed.clone(),
            file_watch_debounce.unwrap_or_else(|| Duration::from_millis(500)),
        )?;

        let lib_loader = Self {
            load_counter,
            lib_dir,
            lib_name: lib_name.as_ref().to_string(),
            watched_lib_file,
            loaded_lib_file,
            lib,
            lib_file_hash,
            changed,
            #[cfg(target_os = "macos")]
            codesigner,
            loaded_lib_name_template,
        };

        Ok(lib_loader)
    }

    pub fn can_update(&mut self) -> bool {
        self.changed.load(Ordering::Acquire)
    }

    /// Checks if the watched library has changed. If it has, reload it and return
    /// true. Otherwise return false.
    pub fn update(&mut self) -> OctaResult<bool> {
        if !self.can_update() {
            return Ok(false);
        }
        
        self.reload()?;
        self.changed.store(false, Ordering::Release);

        Ok(true)
    }

    /// Reload library `self.lib_file`.
    fn reload(&mut self) -> OctaResult<()> {
        let Self {
            load_counter,
            lib_dir,
            lib_name,
            watched_lib_file,
            loaded_lib_file,
            lib,
            loaded_lib_name_template,
            ..
        } = self;

        log::info!("reloading lib {watched_lib_file:?}");

        // Close the loaded lib, copy the new lib to a file we can load, then load it.
        if let Some(lib) = lib.take() {
            lib.close()?;
            if loaded_lib_file.exists() {
                let _ = fs::remove_file(&loaded_lib_file);
            }
        }

        if watched_lib_file.exists() {
            *load_counter += 1;
            let (_, loaded_lib_file) = watched_and_loaded_library_paths(
                lib_dir,
                lib_name,
                *load_counter,
                loaded_lib_name_template,
            );
            log::debug!("copy {watched_lib_file:?} -> {loaded_lib_file:?}");
            fs::copy(watched_lib_file, &loaded_lib_file)?;
            self.lib_file_hash
                .store(hash_file(&loaded_lib_file), Ordering::Release);
            #[cfg(target_os = "macos")]
            self.codesigner.codesign(&loaded_lib_file);
            self.lib = Some(load_library(&loaded_lib_file)?);
            self.loaded_lib_file = loaded_lib_file;
        } else {
            log::warn!("trying to reload library but it does not exist");
        }

        Ok(())
    }

    /// Watch for changes of `lib_file`.
    fn watch(
        lib_file: impl AsRef<Path>,
        lib_file_hash: Arc<AtomicU32>,
        changed: Arc<AtomicBool>,
        debounce: Duration,
    ) -> OctaResult<()> {
        let lib_file = lib_file.as_ref().to_path_buf();
        log::info!("start watching changes of file {}", lib_file.display());

        // File watcher thread. We watch `self.lib_file`, when it changes and we haven't
        // a pending change still waiting to be loaded, set `self.changed` to true. This
        // then gets picked up by `self.update`.
        thread::spawn(move || {

            let lib_file_copy = lib_file.to_owned();
            let mut debouncer = new_debouncer(debounce, None, move |_event| {
                if hash_file(&lib_file_copy) == lib_file_hash.load(Ordering::Acquire) ||
                    !lib_file_copy.exists() ||
                    changed.load(Ordering::Acquire)
                {
                    // file not changed
                    return;
                }

                log::debug!("Lib changed",);

                changed.store(true, Ordering::Release);
            }).expect("creating notify debouncer");

            loop {
                let _ = debouncer
                    .watch(&lib_file, RecursiveMode::NonRecursive);
                //log::debug!("{:?}", res);
            }
            
        });
        Ok(())
    }

    /// Get a pointer to a function or static variable by symbol name. Just a
    /// wrapper around [libloading::Library::get].
    ///
    /// The `symbol` may not contain any null bytes, with the exception of the
    /// last byte. Providing a null-terminated `symbol` may help to avoid an
    /// allocation. The symbol is interpreted as is, no mangling.
    ///
    /// # Safety
    ///
    /// Users of this API must specify the correct type of the function or variable loaded.
    pub unsafe fn get_symbol<T>(&self, name: &str) -> OctaResult<Symbol<T>> {
        match &self.lib {
            None => Err(anyhow!(format!("{name}(...) not found!"))),
            Some(lib) => Ok(lib.get(name.as_bytes())?),
        }
    }

    /// Helper to log from the macro without requiring the user to have the log
    /// crate around
    #[doc(hidden)]
    pub fn log_info(what: impl std::fmt::Display) {
        log::info!("{}", what);
    }
}

/// Deletes the currently loaded lib file if it exists
impl Drop for LibReloader {
    fn drop(&mut self) {       
        if self.loaded_lib_file.exists() {
            log::trace!("removing {:?}", self.loaded_lib_file);
            let _ = fs::remove_file(&self.loaded_lib_file);
        }
    }
}

fn watched_and_loaded_library_paths(
    lib_dir: impl AsRef<Path>,
    lib_name: impl AsRef<str>,
    load_counter: usize,
    loaded_lib_name_template: &Option<impl AsRef<str>>,
) -> (PathBuf, PathBuf) {
    let lib_dir = &lib_dir.as_ref();

    // sort out os dependent file name
    #[cfg(target_os = "macos")]
    let (prefix, ext) = ("lib", "dylib");
    #[cfg(target_os = "linux")]
    let (prefix, ext) = ("lib", "so");
    #[cfg(target_os = "windows")]
    let (prefix, ext) = ("", "dll");
    let lib_name = format!("{prefix}{}", lib_name.as_ref());

    let watched_lib_file = lib_dir.join(&lib_name).with_extension(ext);

    let loaded_lib_filename = match loaded_lib_name_template {
        Some(loaded_lib_name_template) => {
            let result = loaded_lib_name_template
                .as_ref()
                .replace("{lib_name}", &lib_name)
                .replace("{load_counter}", &load_counter.to_string())
                .replace("{pid}", &std::process::id().to_string());
            #[cfg(feature = "uuid")]
            {
                result.replace("{uuid}", &uuid::Uuid::new_v4().to_string())
            }
            #[cfg(not(feature = "uuid"))]
            {
                result
            }
        }
        None => format!("{lib_name}-hot-{load_counter}"),
    };
    let loaded_lib_file = lib_dir.join(loaded_lib_filename).with_extension(ext);
    (watched_lib_file, loaded_lib_file)
}

/// Try to find that might be a relative path such as `target/debug/` by walking
/// up the directories, starting from cwd. This helps finding the lib when the
/// app was started from a directory that is not the project/workspace root.
fn find_file_or_dir_in_parent_directories(
    file: impl AsRef<Path>,
) -> OctaResult<PathBuf> {
    let mut file = file.as_ref().to_path_buf();
    if !file.exists() && file.is_relative() {
        if let Ok(cwd) = std::env::current_dir() {
            let mut parent_dir = Some(cwd.as_path());
            while let Some(dir) = parent_dir {
                if dir.join(&file).exists() {
                    file = dir.join(&file);
                    break;
                }
                parent_dir = dir.parent();
            }
        }
    }

    if file.exists() {
        Ok(file)
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("file {file:?} does not exist"),
        )
        .into())
    }
}

fn load_library(lib_file: impl AsRef<Path>) -> OctaResult<Library> {
    Ok(unsafe { Library::new(lib_file.as_ref()) }?)
}

fn hash_file(f: impl AsRef<Path>) -> u32 {
    fs::read(f.as_ref())
        .map(|content| crc32fast::hash(&content))
        .unwrap_or_default()
}
