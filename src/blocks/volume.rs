use super::{Block, BlockDirty, Fd, Instance, Line};
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
    proxy::node::{NodeChangeMask, NodeEvents},
    proxy::{ProxyEvents, metadata::Metadata, node::Node, registry::RegistryEvents},
    some_closure, types,
};
use pipewire_native_spa::{
    param::{ParamType, props::Prop},
    pod::{RawPodOwned, parser::Parser},
};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock},
};

#[derive(Debug, Default, Clone, Copy)]
struct Sink {
    percent: Option<u8>,
    mute: bool,
    down: bool,
    idle: bool,
}

#[derive(Debug, Default)]
struct SinkState {
    id: u32,
    percent: Option<u8>,
    mute: bool,
    props: HashMap<String, String>,
}

impl SinkState {
    fn match_props(&self, props: &HashMap<String, String>) -> bool {
        props
            .iter()
            .all(|(key, value)| self.props.get(key) == Some(value))
    }

    fn set_props(&mut self, props: &Properties, watched_props: &[String]) {
        self.props.clear();
        for key in watched_props {
            if let Some(value) = props.get(key) {
                self.props.insert(key.clone(), value.to_owned());
            }
        }
    }
}

#[derive(Default)]
struct Sinks {
    default_sink: Option<String>,
    sinks: HashMap<String, SinkState>,
    watched_props: Vec<String>,
}

impl Sinks {
    fn add(&mut self, id: u32, name: String, props: &Properties) {
        let sink = self.sinks.entry(name).or_default();
        sink.id = id;
        sink.set_props(props, &self.watched_props);
        debug!(
            "PipeWire node #{}: created with properties {:?}",
            sink.id, sink.props
        );
    }

    fn set_props(&mut self, name: &str, props: &Properties) {
        if let Some(sink) = self.sinks.get_mut(name) {
            sink.set_props(props, &self.watched_props);
            debug!(
                "PipeWire node #{}: changed properties {:?}",
                sink.id, sink.props
            );
        }
    }

    fn remove(&mut self, id: u32) {
        self.sinks.retain(|_, sink| {
            if sink.id == id {
                debug!("PipeWire node #{}: removed", id);
                false
            } else {
                true
            }
        });
    }

    fn find(&self, properties: &HashMap<String, String>) -> Sink {
        let found = if properties.is_empty() {
            self.default_sink
                .as_ref()
                .and_then(|n| self.sinks.get_key_value(n))
        } else {
            self.sinks
                .iter()
                .find(|(_, sink)| sink.match_props(properties))
        };

        if let Some((name, sink)) = found {
            Sink {
                percent: sink.percent,
                mute: sink.mute,
                down: false,
                idle: self.default_sink.as_ref() != Some(name),
            }
        } else {
            Sink {
                down: true,
                ..Default::default()
            }
        }
    }

    fn retain_default_sink(&mut self) {
        if let Some(sink) = self.default_sink.as_ref() {
            self.sinks.retain(|key, _| key == sink);
        } else {
            self.sinks.clear();
        }
    }
}

pub struct Group {
    pub instances: Vec<Volume>,
    token: Option<RegistrationToken>,

    // The library requires captured variables to satisfy Send.
    sinks: Arc<RwLock<Sinks>>,
}

impl Group {
    pub fn new() -> Self {
        Self {
            instances: Vec::new(),
            token: None,
            sinks: Arc::new(RwLock::new(Sinks {
                default_sink: None,
                sinks: HashMap::new(),
                watched_props: Vec::new(),
            })),
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

            // Keep the current default sink to avoid flicker.
            self.sinks.write().unwrap().retain_default_sink();
        } else {
            pipewire::init();
            debug!("PipeWire initialized");

            let props: HashSet<_> = self
                .instances
                .iter()
                .flat_map(|i| i.config.properties.keys().cloned())
                .collect();
            debug!("PipeWire properties to watch: {:?}", props);
            self.sinks.write().unwrap().watched_props = props.into_iter().collect();
        };

        let sinks = self.sinks.clone();
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

                        sinks.write().unwrap().add(id, name.clone(), props);
                        let object = registry.bind(id, interface, version).unwrap();
                        let node = object.downcast::<Node>().unwrap();
                        node.subscribe_params(&[ParamType::Props])
                            .expect("Failed to subscribe node");
                        node.add_listener(NodeEvents {
                            info: Some(Box::new({
                                let sinks = sinks.clone();
                                let name = name.clone();
                                move |info| {
                                    if info.mask.contains(NodeChangeMask::PROPS) {
                                        sinks.write().unwrap().set_props(&name, info.props);
                                    }
                                }
                            })),

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
            global_remove: some_closure!([^(sinks)] id, {
                sinks.write().unwrap().remove(id);
            }),
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
                    let sinks = sinks.read().unwrap();

                    for i in 0..state.blocks.volume.instances.len() {
                        let dirty = {
                            let instance = &mut state.blocks.volume.instances[i];
                            let sink = sinks.find(&instance.config.properties);
                            let Some(update) = instance.update(sink) else {
                                continue;
                            };
                            update
                        };

                        state.mark_all_outputs_block_dirty(dirty);
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
        let Some(sink) = sinks.sinks.get_mut(node_name) else {
            debug!("PipeWire node {}: not in the map", node_name);
            return;
        };
        if let Some(v) = volume {
            sink.percent = Some(v);
        }
        if let Some(m) = mute {
            sink.mute = m;
        }

        debug!("PipeWire node {}: {:?}", node_name, sink);
    }
}

pub struct Volume {
    id: usize,
    sink: Sink,
    config: VolumeConfig,
}

impl Volume {
    pub fn new(id: usize, config: &VolumeConfig) -> Self {
        Self {
            id,
            sink: Sink {
                down: !config.properties.is_empty(),
                ..Default::default()
            },
            config: config.clone(),
        }
    }

    fn update(&mut self, sink: Sink) -> Option<BlockDirty> {
        let layout = self.sink.down != sink.down
            || self.sink.idle != sink.idle
            || self.sink.mute != sink.mute;
        if layout || self.sink.percent != sink.percent {
            self.sink = sink;
            Some(BlockDirty {
                index: self.id,
                layout,
            })
        } else {
            None
        }
    }

    fn format(&self) -> &[VolumeFormatItem] {
        if self.sink.down {
            &self.config.down.format
        } else if self.sink.idle {
            &self.config.idle.format
        } else if self.sink.mute {
            &self.config.muted.format
        } else {
            &self.config.format
        }
    }
}

impl Block for Volume {
    fn block(&self) -> &BlockConfig {
        &self.config.block
    }

    fn colors(&self) -> &ColorConfig {
        if self.sink.down {
            &self.config.down.color
        } else if self.sink.idle {
            &self.config.idle.color
        } else if self.sink.mute {
            &self.config.muted.color
        } else {
            &self.config.color
        }
    }

    fn len(&self) -> usize {
        self.format().len()
    }

    fn get(&self, index: usize, rasterizer: &Rasterizer, scale: i32) -> Line {
        let item = &self.format()[index];
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sink_selection() {
        let properties = HashMap::from([("device.api".into(), "alsa".into())]);
        let mut sinks = Sinks {
            default_sink: Some("other".into()),
            sinks: HashMap::from([
                (
                    "first".into(),
                    SinkState {
                        id: 1,
                        percent: Some(10),
                        props: properties.clone(),
                        ..Default::default()
                    },
                ),
                (
                    "second".into(),
                    SinkState {
                        id: 2,
                        percent: Some(20),
                        ..Default::default()
                    },
                ),
                (
                    "other".into(),
                    SinkState {
                        id: 3,
                        percent: Some(30),
                        ..Default::default()
                    },
                ),
            ]),
            ..Default::default()
        };

        // first, idle
        let sink = sinks.find(&properties);
        assert_eq!(sink.percent, Some(10));
        assert!(sink.idle);

        // first, default
        sinks.default_sink = Some("first".into());
        let sink = sinks.find(&properties);
        assert_eq!(sink.percent, Some(10));
        assert!(!sink.idle);

        // first, idle
        sinks.default_sink = Some("other".into());
        let sink = sinks.find(&properties);
        assert_eq!(sink.percent, Some(10));
        assert!(sink.idle);

        // default
        let sink = sinks.find(&HashMap::new());
        assert_eq!(sink.percent, Some(30));
        assert!(!sink.idle);

        // down
        let properties = HashMap::from([
            ("device.api".into(), "alsa".into()),
            ("device.bus".into(), "pci".into()),
        ]);
        let sink = sinks.find(&properties);
        assert!(sink.down);
    }

    #[test]
    fn state_changes() {
        let mut config = VolumeConfig::default(&ColorConfig::default());
        config.format = vec![VolumeFormatItem::Label("VOL".into())];
        config.muted.format = vec![
            VolumeFormatItem::Label("MUT".into()),
            VolumeFormatItem::Volume,
        ];
        let mut volume = Volume::new(3, &config);

        // Initialize
        let dirty = volume
            .update(Sink {
                percent: Some(50),
                mute: false,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(volume.format(), config.format);
        assert_eq!(volume.colors(), &config.color);
        assert_eq!(
            dirty,
            BlockDirty {
                index: 3,
                layout: false
            }
        );

        // Volume changes
        let dirty = volume
            .update(Sink {
                percent: Some(40),
                mute: false,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(volume.format(), config.format);
        assert_eq!(volume.colors(), &config.color);
        assert_eq!(
            dirty,
            BlockDirty {
                index: 3,
                layout: false,
            }
        );

        // Mute
        let dirty = volume
            .update(Sink {
                percent: Some(40),
                mute: true,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(volume.format(), config.muted.format);
        assert_eq!(volume.colors(), &config.muted.color);
        assert_eq!(
            dirty,
            BlockDirty {
                index: 3,
                layout: true,
            }
        );

        // Unmute
        let dirty = volume
            .update(Sink {
                percent: Some(40),
                mute: false,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(volume.format(), config.format);
        assert_eq!(volume.colors(), &config.color);
        assert_eq!(
            dirty,
            BlockDirty {
                index: 3,
                layout: true,
            }
        );

        // Remove
        let dirty = volume
            .update(Sink {
                percent: Some(40),
                mute: false,
                down: true,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(volume.format(), config.down.format);
        assert_eq!(volume.colors(), &config.down.color);
        assert_eq!(
            dirty,
            BlockDirty {
                index: 3,
                layout: true,
            }
        );
    }
}
