use super::{Block, Fd, Instance, Line};
use crate::blocks::FormatItem;
use crate::config::{BlockConfig, ColorConfig, VolumeConfig, VolumeFormatItem};
use crate::raster::Rasterizer;
use crate::state::State;
use crate::{debug, error, warning};
use calloop::RegistrationToken;
use pipewire_native::{
    self as pipewire,
    context::Context,
    main_loop::MainLoop,
    properties::Properties,
    proxy::metadata::MetadataEvents,
    proxy::node::NodeEvents,
    proxy::{ProxyEvents, metadata::Metadata, node::Node, registry::RegistryEvents},
    some_closure, types,
};
use pipewire_native_spa::{
    param::{ParamType, props::Prop},
    pod::{RawPodOwned, parser::Parser},
};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

#[derive(Default, Clone, PartialEq)]
struct SinkState {
    percent: Option<u8>,
    mute: bool,
}

#[derive(Default)]
struct Sinks {
    default_sink: Option<String>,
    sinks: HashMap<String, SinkState>,
}

impl Sinks {
    fn current(&self) -> SinkState {
        self.default_sink
            .as_ref()
            .and_then(|n| self.sinks.get(n))
            .cloned()
            .unwrap_or_default()
    }
}

pub struct Group {
    pub instances: Vec<Volume>,
    token: Option<RegistrationToken>,
}

impl Group {
    pub fn new() -> Self {
        Self {
            instances: Vec::new(),
            token: None,
        }
    }

    pub fn add(&mut self, id: usize, config: &VolumeConfig) -> Instance {
        let n = self.instances.len();
        self.instances.push(Volume::new(id, config));
        Instance::Volume(n)
    }

    pub fn register_events(&mut self, handle: &calloop::LoopHandle<'_, State>) {
        if self.instances.is_empty() {
            return;
        }

        if let Some(token) = self.token {
            handle.remove(token);
        }

        pipewire::init();
        debug!("PipeWire initialized");

        // The library requires captured variables to satisfy Send.
        let sinks = Arc::new(RwLock::new(Sinks {
            default_sink: None,
            sinks: HashMap::new(),
        }));

        let properties = Properties::new();
        let main_loop = MainLoop::new(&properties).unwrap();
        let context = Context::new(&main_loop, properties).expect("Failed to create context");
        let core = context
            .connect(None)
            .expect("Failed to connect to the server");
        let registry = core.registry().expect("Failed to create registry");

        registry.add_listener(RegistryEvents {
            global: some_closure!([registry ^(sinks)] id, _perms, interface, version, props, {
                match interface {
                    types::interface::METADATA => {
                        let object = registry.bind(id, interface, version).unwrap();

                        let metadata = object.downcast::<Metadata>().unwrap();
                        metadata.add_listener({
                            let sinks = sinks.clone();
                            MetadataEvents {
                                property: Some(Box::new(move |_id, key, _type, value| {
                                    if key == Some("default.audio.sink") {
                                        let name = value.and_then(|v| v.split('"').nth(3)).map(|name| name.to_string());

                                        debug!("Default sink = {:?}", name);
                                        sinks.write().unwrap().default_sink = name;
                                    }
                                })),
                            }
                        });

                        let proxy = object.downcast_proxy::<Metadata>().unwrap();
                        proxy.add_listener(ProxyEvents {
                            removed: some_closure!([] {}),
                            ..Default::default()
                        });
                    },
                    types::interface::NODE => {
                        let Some(name) = props.get("node.name").map(|s| s.to_owned()) else {
                            warning!("PipeWire: node does not have a name");
                            return;
                        };
                        let Some(class) = props.get("media.class") else {
                            debug!("PipeWire: node {} does not have a class", name);
                            return;
                        };

                        // Match both Audio/Sink and Audio/Sink/Internal.
                        let Some(suffix) = class.strip_prefix("Audio/Sink") else {
                            return;
                        };
                        if suffix == "/Internal" {
                            warning!("PipeWire: node {} is internal, its properties may not be displayed correctly", name);
                        }

                        let object = registry.bind(id, interface, version).unwrap();
                        let node = object.downcast::<Node>().unwrap();
                        node.subscribe_params(&[ParamType::Props])
                            .expect("Failed to subscribe node");
                        node.add_listener(NodeEvents {
                            info: Some(Box::new(move |_| {})),

                            param: Some(Box::new({
                                let sinks = sinks.clone();
                                move |_, param_type, _, _, pod: &RawPodOwned| {
                                    if param_type == ParamType::Props {
                                        update_sink_volume(&name, pod, &sinks);
                                    }
                                }
                            })),
                        });

                        let proxy = object.downcast_proxy::<Node>().unwrap();
                        proxy.add_listener(
                            ProxyEvents {
                                removed: some_closure!([] {}),
                                ..Default::default()
                            }
                        );
                    },
                    _ => {
                        return;
                    }
                };
            }),
            global_remove: some_closure!([] _id, {}),
        });

        let fd = main_loop.get_fd();
        let token = handle
            .insert_source(
                calloop::generic::Generic::new(
                    Fd(fd),
                    calloop::Interest::READ,
                    calloop::Mode::Level,
                ),
                move |_, _, state| {
                    // Capture context.
                    let _ = &context;

                    let _ = main_loop.iterate(Some(std::time::Duration::ZERO));
                    let current = sinks.read().unwrap().current();

                    for i in 0..state.blocks.volume.instances.len() {
                        let id = {
                            let instance = &mut state.blocks.volume.instances[i];
                            if !instance.update(&current) {
                                continue;
                            }
                            instance.id
                        };

                        state.mark_all_outputs_block_dirty(id);
                    }

                    Ok(calloop::PostAction::Continue)
                },
            )
            .expect("Failed to insert volume group fd");

        self.token = Some(token);
    }
}

fn update_sink_volume(node_name: &str, pod: &RawPodOwned, sinks: &Arc<RwLock<Sinks>>) {
    let mut parser = Parser::new(pod.data());
    let mut volume = None;
    let mut mute = None;
    let result = parser.pop_object_raw(|p, _type, _id: u32| {
        for (key, _flags, value) in p {
            let Ok(key) = Prop::try_from(key) else {
                warning!("Skipping unknown key: {}", key);
                continue;
            };

            match key {
                Prop::ChannelVolumes => {
                    if let Ok(value) = value.decode::<Vec<f32>>() {
                        let max = value.iter().fold(0.0_f32, |m, v| f32::max(m, *v)).max(0.0);
                        let percent = (max.cbrt() * 100.0).round();
                        volume = Some(percent.clamp(0.0, 255.0) as u8);
                    }
                }
                Prop::Mute => {
                    if let Ok(value) = value.decode::<bool>() {
                        mute = Some(value);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    });

    if let Err(e) = result {
        error!("Failed to parse volume: {:?}", e);
        return;
    }

    if volume.is_some() || mute.is_some() {
        let mut sinks = sinks.write().unwrap();
        let sink = sinks.sinks.entry(node_name.to_string()).or_default();
        if let Some(v) = volume {
            sink.percent = Some(v);
        }
        if let Some(m) = mute {
            sink.mute = m;
        }
    }
}

pub struct Volume {
    id: usize,
    sink: SinkState,
    config: VolumeConfig,
}

impl Volume {
    pub fn new(id: usize, config: &VolumeConfig) -> Self {
        Self {
            id,
            sink: SinkState::default(),
            config: config.clone(),
        }
    }

    fn update(&mut self, current: &SinkState) -> bool {
        if &self.sink == current {
            return false;
        }
        self.sink = current.clone();
        true
    }
}

impl Block for Volume {
    fn block(&self) -> &BlockConfig {
        &self.config.block
    }

    fn colors(&self) -> &ColorConfig {
        if self.sink.mute {
            &self.config.muted.color
        } else {
            &self.config.color
        }
    }

    fn len(&self) -> usize {
        self.config.format.len()
    }

    fn get(&self, index: usize, rasterizer: &Rasterizer, scale: i32) -> Line {
        let item = &self.config.format[index];
        Line {
            height: item.height(rasterizer, scale),
            text: match item {
                VolumeFormatItem::Volume => match self.sink.percent {
                    Some(p) => format!("{}", p),
                    None => "...".into(),
                },
                VolumeFormatItem::Label(s) => s.clone(),
            },
        }
    }
}
