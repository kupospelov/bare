use std::time::Duration;

use super::{Block, BlockDirty, Instance, Line};
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
                    if let Some(update) = state.blocks.time.instances[i].update(utc) {
                        state.mark_all_outputs_block_dirty(update);
                    }
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
    fn update(&mut self, utc: UtcDateTime) -> Option<BlockDirty> {
        let now = match utc.project(self.config.timezone.as_ref()) {
            Ok(t) => t,
            Err(e) => {
                error!("Failed to project time to timezone: {}", e);
                return None;
            }
        };

        let weekday = self.now.week_day() != now.week_day();
        let changed = self.config.format.iter().any(|item| match item {
            TimeFormatItem::Hour => self.now.hour() != now.hour(),
            TimeFormatItem::Minute => self.now.minute() != now.minute(),
            TimeFormatItem::Day => self.now.month_day() != now.month_day(),
            TimeFormatItem::Month => self.now.month() != now.month(),
            TimeFormatItem::Weekday => weekday,
            TimeFormatItem::Label(_) => false,
        });
        self.now = now;
        changed.then_some(BlockDirty {
            index: self.id,
            layout: weekday,
        })
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
        let text = match item {
            TimeFormatItem::Hour => format!("{:02}", self.now.hour()),
            TimeFormatItem::Minute => format!("{:02}", self.now.minute()),
            TimeFormatItem::Day => format!("{:02}", self.now.month_day()),
            TimeFormatItem::Month => format!("{:02}", self.now.month()),
            TimeFormatItem::Weekday => {
                let value = self.config.weekdays[self.now.week_day() as usize].clone();
                return Line {
                    height: rasterizer.get_font_size(&value, scale),
                    text: value,
                };
            }
            TimeFormatItem::Label(s) => {
                let value = s.clone();
                return Line {
                    height: rasterizer.get_font_size(&value, scale),
                    text: value,
                };
            }
        };
        Line {
            height: rasterizer.get_default_font_size(scale),
            text,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weekday_changes() {
        let rasterizer = Rasterizer::new(crate::font::load("Sans"));
        let mut config = TimeConfig::default(&ColorConfig::default());
        config.timezone = tz::TimeZone::from_posix_tz("UTC-2").unwrap();
        config.format = vec![TimeFormatItem::Weekday, TimeFormatItem::Minute];
        config.weekdays = ["su", "mo", "tu", "we", "th", "fr", "sa"].map(String::from);
        let mut group = Group::new();
        group.add(0, &config);
        let time = &mut group.instances[0];
        let height = time.get(0, &rasterizer, 1).height;

        // Minute changes
        let before_midnight = UtcDateTime::new(1970, 1, 1, 21, 59, 0, 0).unwrap();
        assert!(!time.update(before_midnight).unwrap().layout);
        let line = time.get(0, &rasterizer, 1);
        assert_eq!(line.height, height);
        assert_eq!(line.text, "th");

        // Weekday changes
        let midnight = UtcDateTime::new(1970, 1, 1, 22, 0, 0, 0).unwrap();
        assert!(time.update(midnight).unwrap().layout);
        let line = time.get(0, &rasterizer, 1);
        assert_eq!(line.height, height);
        assert_eq!(line.text, "fr");

        // Second changes (not in the format; ignored)
        let after_midnight = UtcDateTime::new(1970, 1, 1, 22, 0, 1, 0).unwrap();
        assert!(time.update(after_midnight).is_none());
        let line = time.get(0, &rasterizer, 1);
        assert_eq!(line.height, height);
        assert_eq!(line.text, "fr");
    }
}
