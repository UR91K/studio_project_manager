//! Common test utilities and shared setup

#![allow(unused)]
use chrono::{DateTime, Local};
use rand::{thread_rng, Rng};
use std::collections::HashSet;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Once};
use uuid::Uuid;

use studio_project_manager::live_set::LiveSet;
use studio_project_manager::models::{
    AbletonVersion, KeySignature, Plugin, PluginFormat, Sample, TimeSignature,
};
use studio_project_manager::scan::parser::ParseResult;

pub mod builders;
pub mod helpers;

// Global INIT for all tests - ensures logger is initialized only once across all tests
static INIT: Once = Once::new();

/// Shared test setup function that can be used across all test files
/// This should be called at the beginning of each test to ensure proper logging setup
pub fn setup(log_level: &str) {
    let _ = INIT.call_once(|| {
        let _ = env::set_var("RUST_LOG", log_level);
        if let Err(_) = env_logger::try_init() {
            // Logger already initialized, that's fine
        }
    });
}

/// Builder for creating test LiveSets with specific properties
#[derive(Debug)]
pub struct LiveSetBuilder {
    pub plugins: HashSet<Plugin>,
    pub samples: HashSet<Sample>,
    pub tempo: f64,
    pub time_signature: TimeSignature,
    pub furthest_bar: Option<f64>,
    pub key_signature: Option<KeySignature>,
    pub version: AbletonVersion,
    pub created_time: Option<DateTime<Local>>,
    pub modified_time: Option<DateTime<Local>>,
}

impl LiveSetBuilder {
    pub fn new() -> Self {
        Self {
            plugins: HashSet::new(),
            samples: HashSet::new(),
            tempo: 120.0,
            time_signature: TimeSignature::default(),
            furthest_bar: None,
            key_signature: None,
            version: AbletonVersion::default(),
            created_time: None,
            modified_time: None,
        }
    }

    pub fn with_created_time(mut self, time: DateTime<Local>) -> Self {
        self.created_time = Some(time);
        self
    }

    pub fn with_modified_time(mut self, time: DateTime<Local>) -> Self {
        self.modified_time = Some(time);
        self
    }

    pub fn with_plugin(mut self, name: &str) -> Self {
        self.plugins.insert(Plugin {
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
        });
        self
    }

    pub fn with_installed_plugin(mut self, name: &str, vendor: Option<String>) -> Self {
        self.plugins.insert(Plugin {
            id: Uuid::new_v4(),
            name: name.to_string(),
            plugin_id: Some(1),
            module_id: Some(1),
            dev_identifier: format!("device:vst3:{}", name),
            vendor,
            version: Some("1.0.0".to_string()),
            sdk_version: Some("1.0.0".to_string()),
            flags: Some(0),
            scanstate: Some(0),
            enabled: Some(1),
            plugin_format: PluginFormat::VST3AudioFx,
            installed: true,
        });
        self
    }

    pub fn with_sample(mut self, name: &str) -> Self {
        self.samples.insert(Sample {
            id: Uuid::new_v4(),
            name: name.to_string(),
            path: PathBuf::from(name),
            is_present: true,
        });
        self
    }

    pub fn with_tempo(mut self, tempo: f64) -> Self {
        self.tempo = tempo;
        self
    }

    pub fn with_time_signature(mut self, numerator: u8, denominator: u8) -> Self {
        self.time_signature = TimeSignature {
            numerator,
            denominator,
        };
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

    pub fn build(self) -> ParseResult {
        ParseResult {
            plugins: self.plugins,
            samples: self.samples,
            tempo: self.tempo,
            time_signature: self.time_signature,
            furthest_bar: self.furthest_bar,
            key_signature: self.key_signature,
            version: self.version,
        }
    }
}

pub fn random_letter() -> char {
    let mut rng = thread_rng();
    char::from(rng.gen_range(b'A'..=b'Z'))
}

pub fn generate_dev_identifier(plugin_format: PluginFormat) -> String {
    let (dev_type, category) = plugin_format.to_dev_type_and_category();
    let random_uuid = Uuid::new_v4();

    if dev_type == "vst" {
        // VST2 uses a numeric identifier with an optional `n` query parameter
        let random_number: u32 = thread_rng().gen(); // Generate a random 32-bit number
        format!(
            "device:{}:{}:{}?n=Test%20Plugin",
            dev_type, category, random_number
        )
    } else {
        // VST3 uses a UUID
        format!("device:{}:{}:{}", dev_type, category, random_uuid)
    }
}

pub fn generate_mock_plugin(index: usize) -> Plugin {
    let plugin_format = PluginFormat::random();
    Plugin {
        id: Uuid::new_v4(),
        plugin_id: None,
        module_id: None,
        name: format!("Test Plugin {}", index),
        dev_identifier: generate_dev_identifier(plugin_format),
        plugin_format,
        vendor: Some(format!("Test Vendor {}", random_letter())),
        version: Some("1.0.0".to_string()),
        sdk_version: Some("3.7.0".to_string()),
        flags: Some(0),
        scanstate: Some(1),
        enabled: Some(1),
        installed: true,
    }
}

pub fn generate_mock_live_set(index: usize) -> LiveSet {
    let path = format!("C:/Projects/Test Project {}.als", index);
    let path = Path::new(&path).to_path_buf();

    // Create mock plugins
    let plugins: HashSet<_> = (0..3).map(|i| generate_mock_plugin(i)).collect();

    // Create mock samples
    let samples: HashSet<_> = (0..3)
        .map(|i| Sample {
            id: Uuid::new_v4(),
            name: format!("Test Sample {}_{}", index, i),
            path: Path::new(&format!("C:/Samples/Test Sample {}_{}.wav", index, i)).to_path_buf(),
            is_present: true,
        })
        .collect();

    LiveSet {
        is_active: true,
        id: Uuid::new_v4(),
        file_path: path.clone(),
        name: path.file_name().unwrap().to_string_lossy().to_string(),
        file_hash: format!("test_hash_{}", index),
        created_time: Local::now(),
        modified_time: Local::now(),
        last_parsed_timestamp: Local::now(),
        ableton_version: AbletonVersion {
            major: 11,
            minor: 0,
            patch: 0,
            beta: false,
        },
        key_signature: None,
        tempo: 120.0,
        time_signature: TimeSignature {
            numerator: 4,
            denominator: 4,
        },
        furthest_bar: Some(64.0),
        plugins,
        samples,
        tags: HashSet::new(),
        estimated_duration: Some(chrono::Duration::seconds(240)),
    }
}

pub fn generate_test_live_sets_vec(count: usize) -> Vec<LiveSet> {
    (0..count).map(generate_mock_live_set).collect()
}

pub fn generate_test_live_sets_arc(count: usize) -> Arc<Vec<LiveSet>> {
    Arc::new(generate_test_live_sets_vec(count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_builder_basic() {
        setup("error");
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
        setup("error");
        let result = LiveSetBuilder::new()
            .with_installed_plugin("Serum", Some("Xfer Records".to_string()))
            .build();

        let plugin = result.plugins.iter().next().unwrap();
        assert_eq!(plugin.name, "Serum");
        assert_eq!(plugin.vendor, Some("Xfer Records".to_string()));
        assert!(plugin.installed);
    }

    #[test]
    fn test_generate_mock_plugin() {
        setup("error");
        let plugin = generate_mock_plugin(1);

        // Basic fields
        assert!(!plugin.name.is_empty(), "Plugin should have a name");
        assert!(
            !plugin.dev_identifier.is_empty(),
            "Plugin should have a dev_identifier"
        );
        assert!(plugin.installed, "Plugin should be installed by default");

        // Dev identifier format
        let dev_id = &plugin.dev_identifier;
        assert!(
            dev_id.starts_with("device:vst:") || dev_id.starts_with("device:vst3:"),
            "Dev identifier should start with device:vst: or device:vst3: but was {}",
            dev_id
        );

        // Optional fields
        assert!(plugin.vendor.is_some(), "Plugin should have a vendor");
        assert!(plugin.version.is_some(), "Plugin should have a version");
        assert!(
            plugin.sdk_version.is_some(),
            "Plugin should have an SDK version"
        );
        assert!(plugin.flags.is_some(), "Plugin should have flags");
        assert!(plugin.scanstate.is_some(), "Plugin should have scanstate");
        assert!(plugin.enabled.is_some(), "Plugin should have enabled state");
    }

    #[test]
    fn test_generate_mock_live_set() {
        setup("error");
        let live_set = generate_mock_live_set(1);

        // Basic fields
        assert!(!live_set.name.is_empty(), "LiveSet should have a file name");
        assert!(
            live_set.file_path.to_string_lossy().ends_with(".als"),
            "File path should end with .als"
        );
        assert!(
            !live_set.file_hash.is_empty(),
            "LiveSet should have a file hash"
        );

        // Plugins
        assert!(!live_set.plugins.is_empty(), "LiveSet should have plugins");
        assert_eq!(
            live_set.plugins.len(),
            3,
            "LiveSet should have exactly 3 plugins"
        );

        // Verify plugin uniqueness
        let plugin_ids: HashSet<_> = live_set.plugins.iter().map(|p| p.id).collect();
        assert_eq!(
            plugin_ids.len(),
            live_set.plugins.len(),
            "All plugins should have unique IDs"
        );

        let plugin_names: HashSet<_> = live_set.plugins.iter().map(|p| &p.name).collect();
        assert_eq!(
            plugin_names.len(),
            live_set.plugins.len(),
            "All plugins should have unique names"
        );

        // Samples
        assert!(!live_set.samples.is_empty(), "LiveSet should have samples");
        assert_eq!(
            live_set.samples.len(),
            3,
            "LiveSet should have exactly 3 samples"
        );

        // Verify sample uniqueness
        let sample_ids: HashSet<_> = live_set.samples.iter().map(|s| s.id).collect();
        assert_eq!(
            sample_ids.len(),
            live_set.samples.len(),
            "All samples should have unique IDs"
        );

        let sample_paths: HashSet<_> = live_set.samples.iter().map(|s| &s.path).collect();
        assert_eq!(
            sample_paths.len(),
            live_set.samples.len(),
            "All samples should have unique paths"
        );

        // Ableton version
        assert!(
            live_set.ableton_version.major > 0,
            "Should have valid major version"
        );
        assert!(
            !live_set.ableton_version.beta,
            "Should not be beta by default"
        );

        // Musical properties
        assert!(live_set.tempo > 0.0, "Should have positive tempo");
        assert!(
            live_set.time_signature.numerator > 0,
            "Should have valid time signature numerator"
        );
        assert!(
            live_set.time_signature.denominator > 0,
            "Should have valid time signature denominator"
        );
        assert!(live_set.furthest_bar.is_some(), "Should have furthest bar");
        assert!(
            live_set.estimated_duration.is_some(),
            "Should have estimated duration"
        );
    }

    #[test]
    fn test_generate_test_live_sets_arc() {
        setup("error");
        let count = 5;
        let live_sets = generate_test_live_sets_arc(count);

        // Basic count
        assert_eq!(
            live_sets.len(),
            count,
            "Should generate requested number of LiveSets"
        );

        // Verify uniqueness across all LiveSets
        let all_project_ids: HashSet<_> = live_sets.iter().map(|ls| ls.id).collect();
        assert_eq!(
            all_project_ids.len(),
            count,
            "All LiveSets should have unique IDs"
        );

        let all_file_paths: HashSet<_> = live_sets.iter().map(|ls| &ls.file_path).collect();
        assert_eq!(
            all_file_paths.len(),
            count,
            "All LiveSets should have unique paths"
        );

        // Verify all plugins and samples across LiveSets
        let all_plugin_ids: HashSet<_> = live_sets
            .iter()
            .flat_map(|ls| ls.plugins.iter())
            .map(|p| p.id)
            .collect();
        assert_eq!(
            all_plugin_ids.len(),
            live_sets.iter().map(|ls| ls.plugins.len()).sum::<usize>(),
            "All plugins across LiveSets should have unique IDs"
        );

        let all_sample_ids: HashSet<_> = live_sets
            .iter()
            .flat_map(|ls| ls.samples.iter())
            .map(|s| s.id)
            .collect();
        assert_eq!(
            all_sample_ids.len(),
            live_sets.iter().map(|ls| ls.samples.len()).sum::<usize>(),
            "All samples across LiveSets should have unique IDs"
        );
    }
}
