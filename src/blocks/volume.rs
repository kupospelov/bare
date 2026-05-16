use super::{Block, Fd};
use crate::config::{ColorConfig, VolumeConfig};
use crate::render;
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
struct State {
    default_sink: Option<String>,
    sinks: HashMap<String, SinkState>,
}

impl State {
    fn current(&self) -> SinkState {
        self.default_sink
            .as_ref()
            .and_then(|n| self.sinks.get(n))
            .cloned()
            .unwrap_or_default()
    }
}

pub struct Volume {
    _context: pw::context::ContextRc,
    _core: pw::core::CoreRc,
    _registry: pw::registry::RegistryRc,
    _registry_listener: pw::registry::Listener,
    _proxies: Rc<RefCell<HashMap<u32, ProxyEntry>>>,
    main_loop: pw::main_loop::MainLoopRc,
    state: Rc<RefCell<State>>,
    config: VolumeConfig,
}

struct ProxyEntry {
    _proxy: Box<dyn pw::proxy::ProxyT>,
    _listener: Box<dyn pw::proxy::Listener>,
}

impl Volume {
    pub fn new(config: &VolumeConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let main_loop = pw::main_loop::MainLoopRc::new(None)?;
        let context = pw::context::ContextRc::new(&main_loop, None)?;
        let core = context.connect_rc(None)?;
        let registry = core.get_registry_rc()?;
        let state = Rc::new(RefCell::new(State::default()));
        let proxies: Rc<RefCell<HashMap<u32, ProxyEntry>>> = Rc::new(RefCell::new(HashMap::new()));
        let registry_listener = build_registry_listener(&registry, state.clone(), proxies.clone());
        Ok(Self {
            _context: context,
            _core: core,
            _registry: registry,
            _registry_listener: registry_listener,
            _proxies: proxies,
            main_loop,
            state,
            config: config.clone(),
        })
    }
}

impl Block for Volume {
    fn layout(&self, font_size: u32) -> render::BlockLayout {
        render::BlockLayout {
            height: font_size as i32 + super::inner_margin(font_size) + (font_size * 2 / 3) as i32,
            config: self.config.block.clone(),
        }
    }

    fn colors(&self) -> &ColorConfig {
        if self.state.borrow().current().mute {
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
        let state = self.state.borrow().current();
        let value = match state.percent {
            Some(p) => format!("{}", p),
            None => "??".to_string(),
        };
        let color = if state.mute {
            &self.config.muted.color
        } else {
            &self.config.color
        };

        let margin = super::inner_margin(font_size);
        let label_size = font_size * 2 / 3;
        renderer.render_text(
            map,
            render::Region {
                x: region.x,
                y: region.y,
                w: region.w,
                h: label_size,
            },
            "VOL",
            color.text,
            color.background,
            label_size,
        );
        renderer.render_text(
            map,
            render::Region {
                x: region.x,
                y: region.y + label_size as i32 + margin,
                w: region.w,
                h: font_size,
            },
            &value,
            color.text,
            color.background,
            font_size,
        );
    }

    fn fd(&self) -> Option<calloop::generic::Generic<Fd>> {
        Some(calloop::generic::Generic::new(
            Fd(self.main_loop.loop_().fd().as_raw_fd()),
            calloop::Interest::READ,
            calloop::Mode::Level,
        ))
    }

    fn on_fd(&mut self) -> bool {
        let before = self.state.borrow().current();
        self.main_loop.loop_().iterate(std::time::Duration::ZERO);
        let after = self.state.borrow().current();
        before != after
    }
}

fn build_registry_listener(
    registry: &pw::registry::RegistryRc,
    state: Rc<RefCell<State>>,
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
                    ObjectType::Metadata => bind_default_metadata(&registry, obj, &state),
                    ObjectType::Node => bind_audio_sink(&registry, obj, &state),
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
    state: &Rc<RefCell<State>>,
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
            let state = state.clone();
            move |_subject, key, _type_, value| {
                if key == Some("default.audio.sink") {
                    // Parse JSON like `{"name":"sink_name"}`
                    if let Some(name) = value.and_then(|v| v.split('"').nth(3)) {
                        debug!("Default sink = {}", name);
                        state.borrow_mut().default_sink = Some(name.to_string());
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
    state: &Rc<RefCell<State>>,
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
            let state = state.clone();
            move |_seq, _id, _idx, _next, pod| {
                let Some(pod) = pod else { return };
                let Some((vol, mute)) = parse_props(pod) else {
                    return;
                };
                let mut s = state.borrow_mut();
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
