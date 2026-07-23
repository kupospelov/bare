use std::time::Duration;

use super::{Block, Instance, Line};
use crate::blocks::FormatItem;
use crate::config::{BlockConfig, ColorConfig, TimeConfig, TimeFormatItem};
use crate::raster::Rasterizer;
use crate::state::State;
use crate::{debug, error, fail};
use calloop::RegistrationToken;
use calloop::timer::{TimeoutAction, Timer};
use tz::{DateTime, UtcDateTime};

pub struct Group {
    pub instances: Vec<Time>,
    token: Option<RegistrationToken>,
}

impl Group {
    pub fn new() -> Self {
        Self {
            instances: Vec::new(),
            token: None,
        }
    }

    pub fn add(&mut self, id: usize, config: &TimeConfig) -> Instance {
        let n = self.instances.len();
        let now = match UtcDateTime::UNIX_EPOCH.project(config.timezone.as_ref()) {
            Ok(t) => t,
            Err(e) => {
                fail!("Failed to project time to timezone: {}", e);
            }
        };
        self.instances.push(Time {
            id,
            now,
            config: config.clone(),
        });
        Instance::Time(n)
    }

    pub fn register_events(&mut self, handle: &calloop::LoopHandle<'_, State>) {
        if self.instances.is_empty() {
            return;
        }

        if let Some(token) = self.token {
            handle.remove(token);
        }

        let token = handle
            .insert_source(Timer::immediate(), |instant, _, state| {
                let utc = match UtcDateTime::now() {
                    Ok(t) => {
                        debug!("Read UTC time: {}", t);
                        t
                    }
                    Err(e) => {
                        error!("Cannot read UTC time: {}", e);
                        return TimeoutAction::Drop;
                    }
                };

                for i in 0..state.blocks.time.instances.len() {
                    let id = {
                        let instance = &mut state.blocks.time.instances[i];

                        if !instance.update(utc) {
                            continue;
                        }
                        instance.id
                    };

                    state.mark_all_outputs_block_dirty(id);
                }

                TimeoutAction::ToInstant(instant + Duration::from_secs(60 - utc.second() as u64))
            })
            .expect("Failed to insert time group timer");

        self.token = Some(token);
    }
}

pub struct Time {
    id: usize,
    now: DateTime,
    config: TimeConfig,
}

impl Time {
    fn update(&mut self, utc: UtcDateTime) -> bool {
        let now = match utc.project(self.config.timezone.as_ref()) {
            Ok(t) => t,
            Err(e) => {
                error!("Failed to project time to timezone: {}", e);
                return false;
            }
        };

        let changed = self.config.format.iter().any(|item| match item {
            TimeFormatItem::Hour => self.now.hour() != now.hour(),
            TimeFormatItem::Minute => self.now.minute() != now.minute(),
            TimeFormatItem::Day => self.now.month_day() != now.month_day(),
            TimeFormatItem::Month => self.now.month() != now.month(),
            TimeFormatItem::Label(_) => false,
        });
        self.now = now;
        changed
    }
}

impl Block for Time {
    fn block(&self) -> &BlockConfig {
        &self.config.block
    }

    fn colors(&self) -> &ColorConfig {
        &self.config.color
    }

    fn len(&self) -> usize {
        self.config.format.len()
    }

    fn get(&self, index: usize, rasterizer: &Rasterizer, scale: i32) -> Line {
        let item = &self.config.format[index];
        Line {
            height: item.height(rasterizer, scale),
            text: match item {
                TimeFormatItem::Hour => format!("{:02}", self.now.hour()),
                TimeFormatItem::Minute => format!("{:02}", self.now.minute()),
                TimeFormatItem::Day => format!("{:02}", self.now.month_day()),
                TimeFormatItem::Month => format!("{:02}", self.now.month()),
                TimeFormatItem::Label(s) => s.clone(),
            },
        }
    }
}
