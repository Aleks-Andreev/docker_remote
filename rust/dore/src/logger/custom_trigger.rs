use std::sync::Once;

use log4rs::append::rolling_file::LogFile;
use log4rs::append::rolling_file::policy::compound::trigger::Trigger;

#[derive(Debug)]
pub(crate) struct  CustomTrigger {
    initial: Once,
    init_size: u64,
    max_size: u64,
}

impl CustomTrigger {
    pub fn new(init_size_kb: u64, max_size_kb: u64) -> Self {
        Self {
            initial: Once::new(),
            init_size: init_size_kb * 1024,
            max_size: max_size_kb * 1024,
        }
    }
}

impl Trigger for CustomTrigger {
    fn trigger(&self, file: &LogFile) -> anyhow::Result<bool> {
        let mut is_triggered = false;
        self.initial.call_once(|| {
            if file.len_estimate() >= self.init_size {
                is_triggered = true;
            }
        });

        if is_triggered {
            return Ok(true);
        }

        Ok(file.len_estimate() >= self.max_size)
    }

    fn is_pre_process(&self) -> bool {
        true // if true, trigger will be activated before writing text in current log
    }
}