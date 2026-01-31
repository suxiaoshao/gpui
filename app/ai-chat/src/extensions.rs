use crate::{
    APP_NAME,
    config::AiChatConfig,
    errors::{AiChatError, AiChatResult},
};
use gpui::{App, Context, Global};
use std::{collections::HashMap, path::PathBuf};
use tracing::{Level, event};
use wasmtime::{Config, Engine, Store, component::*};
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
    let mut config = Config::new();
    config.async_support(true);
    let engine = Engine::new(&config).map_err(|_err| AiChatError::WasmtimeEngineCreationFailed)?;
    Ok(engine)
}

fn get_all_components(
    engine: &Engine,
    extensions_path: PathBuf,
) -> AiChatResult<HashMap<String, (Component, ExtensionConfig)>> {
    let mut map = HashMap::new();
    for entry in std::fs::read_dir(extensions_path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let wasm_path = path.join(WASM_FILE_NAME);
            let config_path = path.join(CONFIG_FILE_NAME);
            let config = ExtensionConfig::load(config_path)?;
            let component = Component::from_file(engine, &wasm_path)
                .map_err(|_| AiChatError::WasmtimeComponentCreationFailed(wasm_path))?;
            map.insert(config.name.clone(), (component, config));
        }
    }
    Ok(map)
}

pub(crate) struct ExtensionContainer {
    engine: Engine,
    component_map: HashMap<String, (Component, ExtensionConfig)>,
    linker: Linker<ExtensionState>,
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
        // engine
        let engine = initialize_wasmtime_engine()?;
        let component_map = get_all_components(&engine, extensions_path)?;

        // linker
        let mut linker = Linker::new(&engine);
        wasmtime_wasi::p2::add_to_linker_async(&mut linker)
            .map_err(|_| AiChatError::WasmtimeError)?;
        Extension::add_to_linker::<_, HasSelf<_>>(&mut linker, |state: &mut ExtensionState| state)
            .map_err(|_| AiChatError::WasmtimeError)?;
        Ok(Self {
            engine,
            component_map,
            linker,
        })
    }
    pub(crate) fn load_from_app<'a, R>(
        app_handle: &'a mut Context<'a, R>,
    ) -> AiChatResult<&'a Self> {
        let extension_container = app_handle.global::<Self>();
        Ok(extension_container)
    }
    pub(crate) async fn get_extension<'app, R>(
        &self,
        name: &str,
        app_handle: &mut Context<'app, R>,
    ) -> AiChatResult<ExtensionRunner> {
        let (component, extension_config) = self
            .component_map
            .get(name)
            .ok_or(AiChatError::ExtensionNotFound(name.to_string()))?;
        let config = AiChatConfig::get()?;
        let mut store = Store::new(&self.engine, ExtensionState::new(config));
        let bindings = Extension::instantiate_async(&mut store, component, &self.linker)
            .await
            .map_err(|_| AiChatError::ExtensionError(name.to_string()))?;
        Ok(ExtensionRunner {
            extension: bindings,
            store,
            config: extension_config.clone(),
        })
    }
    pub(crate) fn get_all_config(&self) -> Vec<ExtensionConfig> {
        self.component_map
            .iter()
            .map(|(_, (_, config))| config)
            .cloned()
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
