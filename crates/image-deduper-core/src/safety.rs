use crate::config::Config;

pub struct SafetyManager {
    _config: Config,
}

impl SafetyManager {
    /// Create a new SafetyManager with the provided configuration
    pub fn new(config: &Config) -> Self {
        Self {
            _config: config.clone(),
        }
    }
}
