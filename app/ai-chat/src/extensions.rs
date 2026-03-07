use crate::{
    APP_NAME,
    config::AiChatConfig,
    errors::{AiChatError, AiChatResult},
};
use gpui::{App, Global};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tracing::{Level, event};
use wasmtime::{Engine, Store, component::*};
mod component;
pub(crate) use component::{ChatRequest, Extension, ExtensionState};

const EXTENSION_FOLDER: &str = "extensions";
const WASM_FILE_NAME: &str = "extension.wasm";
const CONFIG_FILE_NAME: &str = "config.toml";

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub(crate) struct ExtensionConfig {
    pub(crate) name: String,
    pub(crate) icon: Option<String>,
    pub(crate) description: Option<String>,
}

impl ExtensionConfig {
    fn load(config_path: PathBuf) -> AiChatResult<Self> {
        let config = std::fs::read_to_string(config_path)?;
        let data = toml::from_str(&config)?;
        Ok(data)
    }
}

fn initialize_wasmtime_engine() -> AiChatResult<Engine> {
    let engine = Engine::default();
    Ok(engine)
}

#[derive(Clone)]
struct ExtensionDescriptor {
    config: ExtensionConfig,
    wasm_path: PathBuf,
}

struct ExtensionRuntime {
    engine: Engine,
    component_map: HashMap<String, Component>,
    linker: Linker<ExtensionState>,
}

fn load_all_extensions(
    extensions_path: &Path,
) -> AiChatResult<HashMap<String, ExtensionDescriptor>> {
    let mut map = HashMap::new();
    for entry in std::fs::read_dir(extensions_path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let wasm_path = path.join(WASM_FILE_NAME);
            let config_path = path.join(CONFIG_FILE_NAME);
            let config = ExtensionConfig::load(config_path)?;
            map.insert(config.name.clone(), ExtensionDescriptor { config, wasm_path });
        }
    }
    Ok(map)
}

fn initialize_runtime(
    extensions: &HashMap<String, ExtensionDescriptor>,
) -> AiChatResult<ExtensionRuntime> {
    let engine = initialize_wasmtime_engine()?;
    let mut component_map = HashMap::new();
    for (name, descriptor) in extensions {
        let component = Component::from_file(&engine, &descriptor.wasm_path).map_err(|_| {
            AiChatError::WasmtimeComponentCreationFailed(descriptor.wasm_path.clone())
        })?;
        component_map.insert(name.clone(), component);
    }

    let mut linker = Linker::new(&engine);
    wasmtime_wasi::p2::add_to_linker_async(&mut linker).map_err(|_| AiChatError::WasmtimeError)?;
    Extension::add_to_linker::<_, HasSelf<_>>(&mut linker, |state: &mut ExtensionState| state)
        .map_err(|_| AiChatError::WasmtimeError)?;
    Ok(ExtensionRuntime {
        engine,
        component_map,
        linker,
    })
}

#[derive(Clone)]
pub(crate) struct ExtensionContainer {
    extensions: Arc<HashMap<String, ExtensionDescriptor>>,
    runtime: Arc<Mutex<Option<Arc<ExtensionRuntime>>>>,
}

impl Global for ExtensionContainer {}

impl ExtensionContainer {
    fn path() -> AiChatResult<PathBuf> {
        let file = dirs_next::config_dir()
            .ok_or(AiChatError::DbPath)?
            .join(APP_NAME)
            .join(EXTENSION_FOLDER);
        if !file.exists() {
            std::fs::create_dir_all(&file)?;
        }
        Ok(file)
    }

    pub(crate) fn new() -> AiChatResult<Self> {
        let extensions_path = Self::path()?;
        let extensions = load_all_extensions(&extensions_path)?;
        Ok(Self {
            extensions: Arc::new(extensions),
            runtime: Arc::new(Mutex::new(None)),
        })
    }

    fn runtime(&self) -> AiChatResult<Arc<ExtensionRuntime>> {
        let mut runtime = self
            .runtime
            .lock()
            .map_err(|_| AiChatError::ExtensionRuntimeError)?;
        if let Some(runtime) = runtime.as_ref() {
            return Ok(runtime.clone());
        }

        event!(Level::INFO, "initialize extension runtime lazily");
        let initialized = Arc::new(initialize_runtime(self.extensions.as_ref())?);
        *runtime = Some(initialized.clone());
        Ok(initialized)
    }

    pub(crate) async fn get_extension(&self, name: &str) -> AiChatResult<ExtensionRunner> {
        let extension = self
            .extensions
            .get(name)
            .ok_or(AiChatError::ExtensionNotFound(name.to_string()))?;
        let runtime = self.runtime()?;
        let component = runtime
            .component_map
            .get(name)
            .ok_or(AiChatError::ExtensionNotFound(name.to_string()))?;
        let config = AiChatConfig::get()?;
        let mut store = Store::new(&runtime.engine, ExtensionState::new(config));
        let bindings = Extension::instantiate_async(&mut store, component, &runtime.linker)
            .await
            .map_err(|_| AiChatError::ExtensionError(name.to_string()))?;
        Ok(ExtensionRunner {
            extension: bindings,
            store,
            config: extension.config.clone(),
        })
    }

    pub(crate) fn get_all_config(&self) -> Vec<ExtensionConfig> {
        self.extensions
            .values()
            .map(|extension| extension.config.clone())
            .collect()
    }
}

pub(crate) struct ExtensionRunner {
    pub(crate) extension: Extension,
    pub(crate) store: Store<ExtensionState>,
    pub(crate) config: ExtensionConfig,
}

pub fn init(cx: &mut App) {
    match inner_init(cx) {
        Ok(_) => {}
        Err(err) => {
            event!(Level::ERROR, error = ?err);
        }
    }
}

fn inner_init(cx: &mut App) -> AiChatResult<()> {
    let extension_config = ExtensionContainer::new()?;
    cx.set_global(extension_config);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_extensions_dir() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir()
            .join(format!("ai-chat-extensions-test-{}-{unique}", std::process::id()))
    }

    #[test]
    fn load_all_extensions_skips_wasm_compilation() {
        let root = temp_extensions_dir();
        let extension_dir = root.join("example");
        fs::create_dir_all(&extension_dir).unwrap();
        fs::write(
            extension_dir.join(CONFIG_FILE_NAME),
            r#"
name = "Example"
icon = "bolt"
description = "example extension"
"#,
        )
        .unwrap();
        fs::write(extension_dir.join(WASM_FILE_NAME), b"not a valid wasm module").unwrap();

        let loaded = load_all_extensions(&root).unwrap();
        let descriptor = loaded.get("Example").unwrap();
        assert_eq!(descriptor.config.name, "Example");
        assert_eq!(descriptor.wasm_path, extension_dir.join(WASM_FILE_NAME));

        fs::remove_dir_all(root).unwrap();
    }
}
