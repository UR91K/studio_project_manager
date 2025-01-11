#![allow(dead_code, unused_variables)]
use std::collections::HashSet;
use std::path::PathBuf;
use uuid::Uuid;

use crate::models::{AbletonVersion, KeySignature, Plugin, PluginFormat, Sample, TimeSignature};
use crate::scan::scanner::ScanResult;

/// Builder for creating test LiveSets with specific properties
pub struct LiveSetBuilder {
    samples: HashSet<Sample>,
    plugins: HashSet<Plugin>,
    tempo: f64,
    time_signature: TimeSignature,
    furthest_bar: Option<f64>,
    key_signature: Option<KeySignature>,
    version: AbletonVersion,
}

impl LiveSetBuilder {
    pub fn new() -> Self {
        Self {
            samples: HashSet::new(),
            plugins: HashSet::new(),
            tempo: 120.0,
            time_signature: TimeSignature::default(), // 4/4
            furthest_bar: None,
            key_signature: None,
            version: AbletonVersion {
                major: 11,
                minor: 0,
                patch: 0,
                beta: false,
            },
        }
    }

    pub fn with_plugin(mut self, name: &str) -> Self {
        let plugin = Plugin {
            id: Uuid::new_v4(),
            name: name.to_string(),
            plugin_id: None,
            module_id: None,
            dev_identifier: format!("device:vst3:{}", name),
            vendor: None,
            version: None,
            sdk_version: None,
            flags: None,
            scanstate: None,
            enabled: None,
            plugin_format: PluginFormat::VST3AudioFx,
            installed: false,
        };
        self.plugins.insert(plugin);
        self
    }

    pub fn with_installed_plugin(mut self, name: &str, vendor: Option<String>) -> Self {
        let plugin = Plugin {
            id: Uuid::new_v4(),
            name: name.to_string(),
            plugin_id: Some(1), // Dummy ID
            module_id: Some(1), // Dummy ID
            dev_identifier: format!("device:vst3:{}", name),
            vendor,
            version: Some("1.0.0".to_string()),
            sdk_version: Some("1.0.0".to_string()),
            flags: Some(0),
            scanstate: Some(0),
            enabled: Some(1),
            plugin_format: PluginFormat::VST3AudioFx,
            installed: true,
        };
        self.plugins.insert(plugin);
        self
    }

    pub fn with_sample(mut self, name: &str) -> Self {
        let sample = Sample::new(
            name.to_string(),
            PathBuf::from(format!("/path/to/{}", name)),
        );
        self.samples.insert(sample);
        self
    }

    pub fn with_tempo(mut self, tempo: f64) -> Self {
        self.tempo = tempo;
        self
    }

    pub fn with_time_signature(mut self, numerator: u8, denominator: u8) -> Self {
        self.time_signature = TimeSignature { numerator, denominator };
        self
    }

    pub fn with_furthest_bar(mut self, bar: f64) -> Self {
        self.furthest_bar = Some(bar);
        self
    }

    pub fn with_key_signature(mut self, key_signature: KeySignature) -> Self {
        self.key_signature = Some(key_signature);
        self
    }

    pub fn with_version(mut self, major: u32, minor: u32, patch: u32, beta: bool) -> Self {
        self.version = AbletonVersion {
            major,
            minor,
            patch,
            beta,
        };
        self
    }

    pub fn build(self) -> ScanResult {
        ScanResult {
            version: self.version,
            samples: self.samples,
            plugins: self.plugins,
            tempo: self.tempo,
            time_signature: self.time_signature,
            furthest_bar: self.furthest_bar,
            key_signature: self.key_signature,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_basic() {
        let result = LiveSetBuilder::new()
            .with_plugin("Serum")
            .with_sample("kick.wav")
            .with_tempo(140.0)
            .build();

        assert_eq!(result.tempo, 140.0);
        assert_eq!(result.plugins.len(), 1);
        assert_eq!(result.samples.len(), 1);
        
        let plugin = result.plugins.iter().next().unwrap();
        assert_eq!(plugin.name, "Serum");
        
        let sample = result.samples.iter().next().unwrap();
        assert_eq!(sample.name, "kick.wav");
    }

    #[test]
    fn test_builder_with_installed_plugin() {
        let result = LiveSetBuilder::new()
            .with_installed_plugin("Serum", Some("Xfer Records".to_string()))
            .build();

        let plugin = result.plugins.iter().next().unwrap();
        assert_eq!(plugin.name, "Serum");
        assert_eq!(plugin.vendor, Some("Xfer Records".to_string()));
        assert!(plugin.installed);
    }
} 