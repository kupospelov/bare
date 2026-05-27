use super::{Block, Fd, Instance};
use crate::config::{ColorConfig, VolumeConfig, VolumeFormatItem};
use crate::render;
use crate::state::State;
use crate::{debug, error};
use pipewire as pw;
use pw::spa::param::ParamType;
use pw::spa::pod::deserialize::PodDeserializer;
use pw::spa::pod::{Pod, Value, ValueArray};
use pw::spa::sys::{
    SPA_PROP_channelVolumes as SPA_PROP_CHANNEL_VOLUMES, SPA_PROP_mute as SPA_PROP_MUTE,
};
use pw::types::ObjectType;
use std::cell::RefCell;
use std::collections::HashMap;
use std::os::fd::AsRawFd;
use std::rc::Rc;

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
}

impl Group {
    pub fn new() -> Self {
        Self {
            instances: Vec::new(),
        }
    }

    pub fn add(&mut self, config: &VolumeConfig) -> Instance {
        let n = self.instances.len();
        self.instances.push(Volume::new(config));
        Instance::Volume(n)
    }

    pub fn register_events(&self, handle: &calloop::LoopHandle<'_, State>) {
        if self.instances.is_empty() {
            return;
        }

        let pw = PipeWire::new().expect("Failed to connect to PipeWire");
        let fd = pw.main_loop.loop_().fd().as_raw_fd();
        handle
            .insert_source(
                calloop::generic::Generic::new(
                    Fd(fd),
                    calloop::Interest::READ,
                    calloop::Mode::Level,
                ),
                move |_, _, state| {
                    // Bind pw to capture the whole struct.
                    let _ = &pw;

                    pw.main_loop.loop_().iterate(std::time::Duration::ZERO);
                    let current = pw.sinks.borrow().current();
                    for i in 0..state.blocks.order.len() {
                        if let Instance::Volume(j) = state.blocks.order[i]
                            && state.blocks.volume.instances[j].update(&current)
                        {
                            state.mark_all_outputs_block_dirty(i);
                        }
                    }

                    Ok(calloop::PostAction::Continue)
                },
            )
            .expect("Failed to insert volume group fd");
    }
}

struct PipeWire {
    main_loop: pw::main_loop::MainLoopRc,
    _context: pw::context::ContextRc,
    _core: pw::core::CoreRc,
    _registry: pw::registry::RegistryRc,
    _registry_listener: pw::registry::Listener,
    _proxies: Rc<RefCell<HashMap<u32, ProxyEntry>>>,
    sinks: Rc<RefCell<Sinks>>,
}

struct ProxyEntry {
    _proxy: Box<dyn pw::proxy::ProxyT>,
    _listener: Box<dyn pw::proxy::Listener>,
}

impl PipeWire {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let main_loop = pw::main_loop::MainLoopRc::new(None)?;
        let context = pw::context::ContextRc::new(&main_loop, None)?;
        let core = context.connect_rc(None)?;
        let registry = core.get_registry_rc()?;
        let sinks = Rc::new(RefCell::new(Sinks::default()));
        let proxies: Rc<RefCell<HashMap<u32, ProxyEntry>>> = Rc::new(RefCell::new(HashMap::new()));
        let registry_listener = build_registry_listener(&registry, sinks.clone(), proxies.clone());
        Ok(Self {
            main_loop,
            _context: context,
            _core: core,
            _registry: registry,
            _registry_listener: registry_listener,
            _proxies: proxies,
            sinks,
        })
    }
}

pub struct Volume {
    sink: SinkState,
    config: VolumeConfig,
}

impl Volume {
    pub fn new(config: &VolumeConfig) -> Self {
        Self {
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

    fn item_text(&self, item: &VolumeFormatItem) -> String {
        match item {
            VolumeFormatItem::Volume => match self.sink.percent {
                Some(p) => format!("{}", p),
                None => "??".into(),
            },
            VolumeFormatItem::Label(s) => s.clone(),
        }
    }

    fn item_height(item: &VolumeFormatItem, font_size: u32) -> u32 {
        match item {
            VolumeFormatItem::Volume => font_size,
            VolumeFormatItem::Label(s) => font_size * 2 / s.len().max(1) as u32,
        }
    }
}

impl Block for Volume {
    fn layout(&self, font_size: u32, scale: i32) -> render::BlockLayout {
        let items = &self.config.format;
        let separator = super::inner_margin(font_size);
        let gaps = items.len().saturating_sub(1) as i32;
        let height: i32 = items
            .iter()
            .map(|i| Self::item_height(i, font_size) as i32)
            .sum::<i32>()
            + gaps * separator;
        let block = self.config.block.scaled(scale);
        render::BlockLayout {
            content: height,
            height: block.height.unwrap_or(height) + block.margins[0] + block.margins[2],
            config: block,
        }
    }

    fn colors(&self) -> &ColorConfig {
        if self.sink.mute {
            &self.config.muted.color
        } else {
            &self.config.color
        }
    }

    fn render(
        &mut self,
        renderer: &mut crate::render::Renderer,
        map: &mut render::Map<'_>,
        region: render::Region,
        font_size: u32,
    ) {
        let color = if self.sink.mute {
            &self.config.muted.color
        } else {
            &self.config.color
        };
        let margin = super::inner_margin(font_size);
        let mut y = region.y;
        for item in &self.config.format {
            let h = Self::item_height(item, font_size);
            let text = self.item_text(item);
            renderer.render_text(
                map,
                render::Region {
                    x: region.x,
                    y,
                    w: region.w,
                    h,
                },
                &text,
                color.text,
                color.background,
                h,
            );
            y += h as i32 + margin;
        }
    }
}

fn build_registry_listener(
    registry: &pw::registry::RegistryRc,
    sinks: Rc<RefCell<Sinks>>,
    proxies: Rc<RefCell<HashMap<u32, ProxyEntry>>>,
) -> pw::registry::Listener {
    let registry_weak = registry.downgrade();
    registry
        .add_listener_local()
        .global({
            let proxies = proxies.clone();
            move |obj| {
                let Some(registry) = registry_weak.upgrade() else {
                    return;
                };
                let entry = match obj.type_ {
                    ObjectType::Metadata => bind_default_metadata(&registry, obj, &sinks),
                    ObjectType::Node => bind_audio_sink(&registry, obj, &sinks),
                    _ => return,
                };
                if let Some(entry) = entry {
                    proxies.borrow_mut().insert(obj.id, entry);
                }
            }
        })
        .global_remove(move |id| {
            proxies.borrow_mut().remove(&id);
        })
        .register()
}

fn bind_default_metadata(
    registry: &pw::registry::RegistryRc,
    obj: &pw::registry::GlobalObject<&pw::spa::utils::dict::DictRef>,
    sinks: &Rc<RefCell<Sinks>>,
) -> Option<ProxyEntry> {
    let props = obj.props?;
    if props.get("metadata.name") != Some("default") {
        return None;
    }
    let metadata: pw::metadata::Metadata = registry
        .bind(obj)
        .inspect_err(|e| error!("Failed to bind default metadata: {:?}", e))
        .ok()?;
    let listener = metadata
        .add_listener_local()
        .property({
            let sinks = sinks.clone();
            move |_subject, key, _type_, value| {
                if key == Some("default.audio.sink") {
                    // Parse JSON like `{"name":"sink_name"}`
                    if let Some(name) = value.and_then(|v| v.split('"').nth(3)) {
                        debug!("Default sink = {}", name);
                        sinks.borrow_mut().default_sink = Some(name.to_string());
                    }
                }
                0
            }
        })
        .register();
    Some(ProxyEntry {
        _proxy: Box::new(metadata),
        _listener: Box::new(listener),
    })
}

fn bind_audio_sink(
    registry: &pw::registry::RegistryRc,
    obj: &pw::registry::GlobalObject<&pw::spa::utils::dict::DictRef>,
    sinks: &Rc<RefCell<Sinks>>,
) -> Option<ProxyEntry> {
    let props = obj.props?;
    if props.get("media.class") != Some("Audio/Sink") {
        return None;
    }
    let name = props.get("node.name")?.to_string();
    let node: pw::node::Node = registry
        .bind(obj)
        .inspect_err(|e| error!("Failed to bind sink {}: {:?}", obj.id, e))
        .ok()?;
    node.subscribe_params(&[ParamType::Props]);
    let listener = node
        .add_listener_local()
        .param({
            let sinks = sinks.clone();
            move |_seq, _id, _idx, _next, pod| {
                let Some(pod) = pod else { return };
                let Some((vol, mute)) = parse_props(pod) else {
                    return;
                };
                let mut s = sinks.borrow_mut();
                let entry = s.sinks.entry(name.clone()).or_default();
                if let Some(v) = vol {
                    entry.percent = Some(v);
                    debug!("Volume {}: {}%", name, v);
                }
                if let Some(m) = mute {
                    entry.mute = m;
                    debug!("Volume {}: mute={}", name, m);
                }
            }
        })
        .register();
    Some(ProxyEntry {
        _proxy: Box::new(node),
        _listener: Box::new(listener),
    })
}

fn parse_props(pod: &Pod) -> Option<(Option<u8>, Option<bool>)> {
    let (_, value) = PodDeserializer::deserialize_any_from(pod.as_bytes()).ok()?;
    let Value::Object(obj) = value else {
        return None;
    };
    let mut vol = None;
    let mut mute = None;
    for prop in obj.properties {
        match prop.key {
            SPA_PROP_CHANNEL_VOLUMES => {
                if let Value::ValueArray(ValueArray::Float(arr)) = prop.value
                    && !arr.is_empty()
                {
                    let max = arr.iter().fold(0.0_f32, |m, v| f32::max(m, *v)).max(0.0);
                    let percent = (max.cbrt() * 100.0).round();
                    vol = Some(percent.clamp(0.0, 255.0) as u8);
                }
            }
            SPA_PROP_MUTE => {
                if let Value::Bool(b) = prop.value {
                    mute = Some(b);
                }
            }
            _ => {}
        }
    }
    Some((vol, mute))
}
