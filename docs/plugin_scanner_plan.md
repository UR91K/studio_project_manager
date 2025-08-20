CLAUDE4

The current Ableton database dependency is a major coupling issue, and the VST scanner approach is much more robust. Let me design a comprehensive plugin discovery system that replaces the Ableton database dependency.
Ran tool
Search files...
Ran tool
Perfect! Now I understand the current plugin validation system. Let me design a comprehensive plugin discovery system that replaces the Ableton database dependency.
Ran tool
## Current Plugin System Analysis

The current system has these key dependencies on Ableton's database:
1. **Plugin validation**: Checking if plugins are installed via `AbletonDatabase::get_plugin_by_dev_identifier`
2. **Plugin metadata**: Getting name, vendor, version from Ableton's database
3. **Installation status**: Determining if plugins are available
4. **Plugin enumeration**: Getting list of all installed plugins

## Proposed Architecture: Independent VST Scanner System
Ran tool
### 1. Plugin Data Harvester (C++ Binary)

Here's the design for the standalone C++ binary:

```cpp
// plugin_data_harvester.cpp
#include <iostream>
#include <string>
#include <memory>
#include <chrono>
#include <exception>
#include <nlohmann/json.hpp>

#ifdef _WIN32
#include <windows.h>
#else
#include <dlfcn.h>
#endif

// VST2 and VST3 SDK includes
#include "pluginterfaces/vst/ivstcomponent.h"
#include "pluginterfaces/vst/ivstaudioprocessor.h"
#include "public.sdk/source/vst/vst2wrapper/vst2wrapper.h"

struct PluginInfo {
    std::string file_path;
    std::string name;
    std::string vendor;
    std::string version;
    std::string unique_id;
    std::string category;
    std::string format; // "VST2" or "VST3"
    bool is_instrument;
    bool is_effect;
    int parameter_count;
    std::vector<std::string> parameter_names;
    std::string sdk_version;
    std::string error_message;
    bool scan_successful;
};

class VSTScanner {
public:
    PluginInfo scanPlugin(const std::string& pluginPath) {
        PluginInfo info;
        info.file_path = pluginPath;
        info.scan_successful = false;
        
        try {
            // Set timeout alarm (5 seconds max per plugin)
            setupTimeout(5);
            
            if (isVST3Plugin(pluginPath)) {
                info = scanVST3Plugin(pluginPath);
            } else if (isVST2Plugin(pluginPath)) {
                info = scanVST2Plugin(pluginPath);
            } else {
                info.error_message = "Unsupported plugin format";
                return info;
            }
            
            info.scan_successful = true;
            
        } catch (const std::exception& e) {
            info.error_message = e.what();
            info.scan_successful = false;
        } catch (...) {
            info.error_message = "Unknown error during plugin scan";
            info.scan_successful = false;
        }
        
        return info;
    }

private:
    bool isVST3Plugin(const std::string& path) {
        return path.find(".vst3") != std::string::npos;
    }
    
    bool isVST2Plugin(const std::string& path) {
        return path.find(".dll") != std::string::npos || 
               path.find(".vst") != std::string::npos;
    }
    
    PluginInfo scanVST3Plugin(const std::string& path) {
        PluginInfo info;
        
        // Load VST3 plugin using VST3 SDK
        // This is a simplified version - real implementation would be more complex
        
#ifdef _WIN32
        HMODULE module = LoadLibraryA(path.c_str());
        if (!module) {
            throw std::runtime_error("Failed to load VST3 plugin");
        }
        
        // Get factory function
        auto GetPluginFactory = (GetFactoryProc)GetProcAddress(module, "GetPluginFactory");
        if (!GetPluginFactory) {
            FreeLibrary(module);
            throw std::runtime_error("Invalid VST3 plugin - no factory function");
        }
        
        // Get factory and enumerate plugins
        IPluginFactory* factory = GetPluginFactory();
        if (!factory) {
            FreeLibrary(module);
            throw std::runtime_error("Failed to get plugin factory");
        }
        
        // Extract plugin information
        PFactoryInfo factoryInfo;
        factory->getFactoryInfo(&factoryInfo);
        info.vendor = factoryInfo.vendor;
        
        // Get first plugin class info
        if (factory->countClasses() > 0) {
            PClassInfo classInfo;
            factory->getClassInfo(0, &classInfo);
            info.name = classInfo.name;
            info.category = classInfo.category;
            info.unique_id = std::string(classInfo.cid, 16); // Convert TUID to string
        }
        
        FreeLibrary(module);
#endif
        
        info.format = "VST3";
        return info;
    }
    
    PluginInfo scanVST2Plugin(const std::string& path) {
        PluginInfo info;
        
        // Load VST2 plugin
#ifdef _WIN32
        HMODULE module = LoadLibraryA(path.c_str());
        if (!module) {
            throw std::runtime_error("Failed to load VST2 plugin");
        }
        
        // Get main function
        auto vstMain = (VstIntPtr (*)(audioMasterCallback))GetProcAddress(module, "VSTPluginMain");
        if (!vstMain) {
            vstMain = (VstIntPtr (*)(audioMasterCallback))GetProcAddress(module, "main");
        }
        
        if (!vstMain) {
            FreeLibrary(module);
            throw std::runtime_error("Invalid VST2 plugin - no main function");
        }
        
        // Create plugin instance with dummy host callback
        AEffect* effect = (AEffect*)vstMain(hostCallback);
        if (!effect) {
            FreeLibrary(module);
            throw std::runtime_error("Failed to create VST2 plugin instance");
        }
        
        // Extract plugin information
        info.unique_id = std::to_string(effect->uniqueID);
        info.is_instrument = (effect->flags & effFlagsIsSynth) != 0;
        info.is_effect = !info.is_instrument;
        info.parameter_count = effect->numParams;
        
        // Get plugin name
        char pluginName[kVstMaxProductStrLen] = {0};
        effect->dispatcher(effect, effGetEffectName, 0, 0, pluginName, 0);
        info.name = pluginName;
        
        // Get vendor name
        char vendorName[kVstMaxVendorStrLen] = {0};
        effect->dispatcher(effect, effGetVendorString, 0, 0, vendorName, 0);
        info.vendor = vendorName;
        
        // Get version
        VstInt32 version = effect->dispatcher(effect, effGetVendorVersion, 0, 0, nullptr, 0);
        info.version = std::to_string(version);
        
        // Get parameter names
        for (int i = 0; i < effect->numParams && i < 100; ++i) { // Limit to 100 params
            char paramName[kVstMaxParamStrLen] = {0};
            effect->dispatcher(effect, effGetParamName, i, 0, paramName, 0);
            info.parameter_names.push_back(paramName);
        }
        
        FreeLibrary(module);
#endif
        
        info.format = "VST2";
        return info;
    }
    
    void setupTimeout(int seconds) {
        // Set up timeout mechanism to prevent hanging
        #ifdef _WIN32
        // Use Windows timer or thread with timeout
        #else
        alarm(seconds);
        #endif
    }
    
    static VstIntPtr VSTCALLBACK hostCallback(AEffect* effect, VstInt32 opcode, 
                                            VstInt32 index, VstIntPtr value, 
                                            void* ptr, float opt) {
        // Minimal host callback - just return what's needed for scanning
        switch (opcode) {
            case audioMasterVersion:
                return kVstVersion;
            case audioMasterCurrentId:
                return 0;
            default:
                return 0;
        }
    }
};

int main(int argc, char* argv[]) {
    if (argc != 3 || std::string(argv[1]) != "-path") {
        std::cerr << "Usage: plugin_data_harvester -path <plugin_file_path>" << std::endl;
        return 1;
    }
    
    std::string pluginPath = argv[2];
    
    VSTScanner scanner;
    PluginInfo info = scanner.scanPlugin(pluginPath);
    
    // Output JSON to stdout
    nlohmann::json output;
    output["file_path"] = info.file_path;
    output["name"] = info.name;
    output["vendor"] = info.vendor;
    output["version"] = info.version;
    output["unique_id"] = info.unique_id;
    output["category"] = info.category;
    output["format"] = info.format;
    output["is_instrument"] = info.is_instrument;
    output["is_effect"] = info.is_effect;
    output["parameter_count"] = info.parameter_count;
    output["parameter_names"] = info.parameter_names;
    output["sdk_version"] = info.sdk_version;
    output["scan_successful"] = info.scan_successful;
    output["error_message"] = info.error_message;
    
    std::cout << output.dump() << std::endl;
    
    return info.scan_successful ? 0 : 1;
}
```

### 2. Rust Integration Layer

```rust
// src/plugins/scanner.rs
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tokio::process::Command as AsyncCommand;
use tokio::time::timeout;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannedPlugin {
    pub file_path: PathBuf,
    pub name: String,
    pub vendor: Option<String>,
    pub version: Option<String>,
    pub unique_id: String,
    pub category: Option<String>,
    pub format: PluginFormat,
    pub is_instrument: bool,
    pub is_effect: bool,
    pub parameter_count: i32,
    pub parameter_names: Vec<String>,
    pub sdk_version: Option<String>,
    pub scan_successful: bool,
    pub error_message: Option<String>,
    pub file_hash: String,
    pub file_size: u64,
    pub last_scanned: chrono::DateTime<chrono::Local>,
}

pub struct PluginScanner {
    harvester_path: PathBuf,
    max_concurrent_scans: usize,
    scan_timeout: Duration,
}

impl PluginScanner {
    pub fn new(harvester_path: PathBuf) -> Self {
        Self {
            harvester_path,
            max_concurrent_scans: num_cpus::get().min(8), // Don't overwhelm system
            scan_timeout: Duration::from_secs(10), // 10 second timeout per plugin
        }
    }
    
    /// Discover all VST/VST3 plugins on the system
    pub async fn discover_plugins(&self) -> Result<Vec<ScannedPlugin>, PluginScanError> {
        let plugin_paths = self.find_plugin_files().await?;
        log::info!("Found {} potential plugin files", plugin_paths.len());
        
        self.scan_plugins_batch(plugin_paths).await
    }
    
    /// Scan plugins in batches with process isolation
    pub async fn scan_plugins_batch(&self, plugin_paths: Vec<PathBuf>) -> Result<Vec<ScannedPlugin>, PluginScanError> {
        use futures::stream::{self, StreamExt};
        
        let results = stream::iter(plugin_paths)
            .map(|path| self.scan_single_plugin(path))
            .buffer_unordered(self.max_concurrent_scans)
            .collect::<Vec<_>>()
            .await;
        
        let mut scanned_plugins = Vec::new();
        let mut failed_count = 0;
        
        for result in results {
            match result {
                Ok(plugin) => scanned_plugins.push(plugin),
                Err(e) => {
                    log::warn!("Plugin scan failed: {:?}", e);
                    failed_count += 1;
                }
            }
        }
        
        log::info!("Scanned {} plugins successfully, {} failed", scanned_plugins.len(), failed_count);
        Ok(scanned_plugins)
    }
    
    /// Scan a single plugin using the C++ harvester
    async fn scan_single_plugin(&self, plugin_path: PathBuf) -> Result<ScannedPlugin, PluginScanError> {
        let start_time = Instant::now();
        
        // Calculate file hash and size
        let file_hash = calculate_file_hash(&plugin_path)?;
        let file_size = std::fs::metadata(&plugin_path)?.len();
        
        log::debug!("Scanning plugin: {}", plugin_path.display());
        
        // Execute harvester with timeout
        let output = timeout(
            self.scan_timeout,
            AsyncCommand::new(&self.harvester_path)
                .arg("-path")
                .arg(&plugin_path)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true) // Important: kill child process on timeout
                .output()
        ).await;
        
        let output = match output {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => return Err(PluginScanError::ProcessError(e.to_string())),
            Err(_) => return Err(PluginScanError::Timeout(plugin_path.clone())),
        };
        
        let scan_duration = start_time.elapsed();
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            log::warn!("Plugin scan failed for {}: {}", plugin_path.display(), stderr);
            
            return Ok(ScannedPlugin {
                file_path: plugin_path,
                name: "Unknown".to_string(),
                vendor: None,
                version: None,
                unique_id: file_hash.clone(),
                category: None,
                format: PluginFormat::VST2AudioFx, // Default
                is_instrument: false,
                is_effect: true,
                parameter_count: 0,
                parameter_names: Vec::new(),
                sdk_version: None,
                scan_successful: false,
                error_message: Some(stderr.to_string()),
                file_hash,
                file_size,
                last_scanned: chrono::Local::now(),
            });
        }
        
        // Parse JSON output
        let stdout = String::from_utf8_lossy(&output.stdout);
        let harvester_result: HarvesterResult = serde_json::from_str(&stdout)
            .map_err(|e| PluginScanError::JsonParseError(e.to_string()))?;
        
        log::debug!("Scanned {} in {:?}", harvester_result.name, scan_duration);
        
        Ok(ScannedPlugin {
            file_path: PathBuf::from(harvester_result.file_path),
            name: harvester_result.name,
            vendor: if harvester_result.vendor.is_empty() { None } else { Some(harvester_result.vendor) },
            version: if harvester_result.version.is_empty() { None } else { Some(harvester_result.version) },
            unique_id: harvester_result.unique_id,
            category: if harvester_result.category.is_empty() { None } else { Some(harvester_result.category) },
            format: match harvester_result.format.as_str() {
                "VST3" => if harvester_result.is_instrument { PluginFormat::VST3Instrument } else { PluginFormat::VST3AudioFx },
                _ => if harvester_result.is_instrument { PluginFormat::VST2Instrument } else { PluginFormat::VST2AudioFx },
            },
            is_instrument: harvester_result.is_instrument,
            is_effect: harvester_result.is_effect,
            parameter_count: harvester_result.parameter_count,
            parameter_names: harvester_result.parameter_names,
            sdk_version: if harvester_result.sdk_version.is_empty() { None } else { Some(harvester_result.sdk_version) },
            scan_successful: harvester_result.scan_successful,
            error_message: if harvester_result.error_message.is_empty() { None } else { Some(harvester_result.error_message) },
            file_hash,
            file_size,
            last_scanned: chrono::Local::now(),
        })
    }
    
    /// Find all VST/VST3 files on the system
    async fn find_plugin_files(&self) -> Result<Vec<PathBuf>, PluginScanError> {
        let mut plugin_paths = Vec::new();
        
        // Common VST paths on Windows
        let search_paths = vec![
            "C:/Program Files/VSTPlugins",
            "C:/Program Files/Steinberg/VSTPlugins", 
            "C:/Program Files/Common Files/VST2",
            "C:/Program Files/Common Files/VST3",
            "C:/Program Files (x86)/VSTPlugins",
            "C:/Program Files (x86)/Steinberg/VSTPlugins",
            "C:/Program Files (x86)/Common Files/VST2",
            "C:/Program Files (x86)/Common Files/VST3",
        ];
        
        for search_path in search_paths {
            let path = PathBuf::from(search_path);
            if path.exists() {
                self.scan_directory_for_plugins(&path, &mut plugin_paths).await?;
            }
        }
        
        // Also check user-specific paths
        if let Ok(user_profile) = std::env::var("USERPROFILE") {
            let user_vst_paths = vec![
                format!("{}/AppData/Roaming/VST3", user_profile),
                format!("{}/Documents/VST3 Presets", user_profile),
            ];
            
            for user_path in user_vst_paths {
                let path = PathBuf::from(user_path);
                if path.exists() {
                    self.scan_directory_for_plugins(&path, &mut plugin_paths).await?;
                }
            }
        }
        
        Ok(plugin_paths)
    }
    
    async fn scan_directory_for_plugins(&self, dir: &Path, plugin_paths: &mut Vec<PathBuf>) -> Result<(), PluginScanError> {
        use tokio::fs;
        
        let mut entries = fs::read_dir(dir).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            
            if path.is_dir() {
                // Recurse into subdirectories
                self.scan_directory_for_plugins(&path, plugin_paths).await?;
            } else if let Some(extension) = path.extension() {
                let ext = extension.to_string_lossy().to_lowercase();
                if ext == "dll" || ext == "vst" || ext == "vst3" {
                    plugin_paths.push(path);
                }
            }
        }
        
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct HarvesterResult {
    file_path: String,
    name: String,
    vendor: String,
    version: String,
    unique_id: String,
    category: String,
    format: String,
    is_instrument: bool,
    is_effect: bool,
    parameter_count: i32,
    parameter_names: Vec<String>,
    sdk_version: String,
    scan_successful: bool,
    error_message: String,
}

#[derive(Debug, thiserror::Error)]
pub enum PluginScanError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Process error: {0}")]
    ProcessError(String),
    #[error("Timeout scanning plugin: {0}")]
    Timeout(PathBuf),
    #[error("JSON parse error: {0}")]
    JsonParseError(String),
    #[error("File hash error: {0}")]
    HashError(String),
}

fn calculate_file_hash(path: &Path) -> Result<String, PluginScanError> {
    use sha2::{Sha256, Digest};
    use std::fs::File;
    use std::io::Read;
    
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 8192];
    
    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    
    Ok(format!("{:x}", hasher.finalize()))
}
```

### 3. Internal Plugin Database Schema
Ran tool
```sql
-- Internal plugin database schema (replaces Ableton DB dependency)
CREATE TABLE IF NOT EXISTS scanned_plugins (
    id TEXT PRIMARY KEY,
    
    -- File information
    file_path TEXT NOT NULL UNIQUE,
    file_hash TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    last_scanned DATETIME NOT NULL,
    
    -- Plugin metadata
    name TEXT NOT NULL,
    vendor TEXT,
    version TEXT,
    unique_id TEXT NOT NULL, -- VST unique ID or generated hash
    category TEXT,
    
    -- Plugin type
    format TEXT NOT NULL, -- VST2Instrument, VST2AudioFx, VST3Instrument, VST3AudioFx
    is_instrument BOOLEAN NOT NULL,
    is_effect BOOLEAN NOT NULL,
    
    -- Technical details
    parameter_count INTEGER NOT NULL DEFAULT 0,
    parameter_names TEXT, -- JSON array of parameter names
    sdk_version TEXT,
    
    -- Scan status
    scan_successful BOOLEAN NOT NULL,
    error_message TEXT,
    
    -- Installation status
    is_available BOOLEAN NOT NULL DEFAULT true, -- File still exists
    last_availability_check DATETIME,
    
    UNIQUE(unique_id, format)
);

-- Updated plugins table (remove Ableton-specific fields)
CREATE TABLE IF NOT EXISTS plugins (
    id TEXT PRIMARY KEY,
    
    -- Core identification (from project files)
    dev_identifier TEXT NOT NULL,
    name TEXT NOT NULL,
    format TEXT NOT NULL,
    
    -- Link to scanned plugin data
    scanned_plugin_id TEXT,
    
    -- Status
    installed BOOLEAN NOT NULL DEFAULT false,
    
    UNIQUE(dev_identifier),
    FOREIGN KEY (scanned_plugin_id) REFERENCES scanned_plugins(id) ON DELETE SET NULL
);

-- Plugin scan history for tracking changes
CREATE TABLE IF NOT EXISTS plugin_scan_history (
    id TEXT PRIMARY KEY,
    scan_started DATETIME NOT NULL,
    scan_completed DATETIME,
    plugins_found INTEGER NOT NULL DEFAULT 0,
    plugins_scanned INTEGER NOT NULL DEFAULT 0,
    plugins_failed INTEGER NOT NULL DEFAULT 0,
    scan_duration_ms INTEGER,
    error_message TEXT
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_scanned_plugins_unique_id ON scanned_plugins(unique_id);
CREATE INDEX IF NOT EXISTS idx_scanned_plugins_name ON scanned_plugins(name);
CREATE INDEX IF NOT EXISTS idx_scanned_plugins_vendor ON scanned_plugins(vendor);
CREATE INDEX IF NOT EXISTS idx_scanned_plugins_format ON scanned_plugins(format);
CREATE INDEX IF NOT EXISTS idx_scanned_plugins_available ON scanned_plugins(is_available);
CREATE INDEX IF NOT EXISTS idx_plugins_dev_identifier ON plugins(dev_identifier);
```

### 4. Plugin Matching System

```rust
// src/plugins/matcher.rs
use crate::models::{Plugin, PluginFormat};
use crate::plugins::scanner::ScannedPlugin;

pub struct PluginMatcher;

impl PluginMatcher {
    /// Match a plugin from project file to scanned plugin database
    pub fn match_plugin(&self, 
                       dev_identifier: &str, 
                       plugin_name: &str,
                       format: PluginFormat,
                       scanned_plugins: &[ScannedPlugin]) -> Option<&ScannedPlugin> {
        
        // Strategy 1: Direct unique ID match (most reliable)
        if let Some(matched) = self.match_by_unique_id(dev_identifier, format, scanned_plugins) {
            return Some(matched);
        }
        
        // Strategy 2: Name + vendor fuzzy match
        if let Some(matched) = self.match_by_name_and_vendor(plugin_name, format, scanned_plugins) {
            return Some(matched);
        }
        
        // Strategy 3: Name similarity match (fallback)
        self.match_by_name_similarity(plugin_name, format, scanned_plugins)
    }
    
    fn match_by_unique_id(&self, dev_identifier: &str, format: PluginFormat, scanned_plugins: &[ScannedPlugin]) -> Option<&ScannedPlugin> {
        scanned_plugins.iter().find(|plugin| {
            plugin.unique_id == dev_identifier && plugin.format == format
        })
    }
    
    fn match_by_name_and_vendor(&self, plugin_name: &str, format: PluginFormat, scanned_plugins: &[ScannedPlugin]) -> Option<&ScannedPlugin> {
        scanned_plugins.iter().find(|plugin| {
            plugin.format == format && 
            self.names_match(&plugin.name, plugin_name)
        })
    }
    
    fn match_by_name_similarity(&self, plugin_name: &str, format: PluginFormat, scanned_plugins: &[ScannedPlugin]) -> Option<&ScannedPlugin> {
        let mut best_match: Option<&ScannedPlugin> = None;
        let mut best_score = 0.0;
        
        for plugin in scanned_plugins.iter().filter(|p| p.format == format) {
            let score = self.calculate_name_similarity(&plugin.name, plugin_name);
            if score > best_score && score > 0.7 { // 70% similarity threshold
                best_score = score;
                best_match = Some(plugin);
            }
        }
        
        best_match
    }
    
    fn names_match(&self, name1: &str, name2: &str) -> bool {
        let normalized1 = self.normalize_plugin_name(name1);
        let normalized2 = self.normalize_plugin_name(name2);
        normalized1 == normalized2
    }
    
    fn normalize_plugin_name(&self, name: &str) -> String {
        name.to_lowercase()
            .replace(" ", "")
            .replace("-", "")
            .replace("_", "")
            .replace("vst", "")
            .replace("vst3", "")
    }
    
    fn calculate_name_similarity(&self, name1: &str, name2: &str) -> f64 {
        // Use Levenshtein distance for fuzzy matching
        let normalized1 = self.normalize_plugin_name(name1);
        let normalized2 = self.normalize_plugin_name(name2);
        
        let distance = levenshtein_distance(&normalized1, &normalized2);
        let max_len = normalized1.len().max(normalized2.len());
        
        if max_len == 0 {
            1.0
        } else {
            1.0 - (distance as f64 / max_len as f64)
        }
    }
}

fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let len1 = s1.len();
    let len2 = s2.len();
    let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];
    
    for i in 0..=len1 {
        matrix[i][0] = i;
    }
    for j in 0..=len2 {
        matrix[0][j] = j;
    }
    
    for (i, c1) in s1.chars().enumerate() {
        for (j, c2) in s2.chars().enumerate() {
            let cost = if c1 == c2 { 0 } else { 1 };
            matrix[i + 1][j + 1] = (matrix[i][j + 1] + 1)
                .min(matrix[i + 1][j] + 1)
                .min(matrix[i][j] + cost);
        }
    }
    
    matrix[len1][len2]
}
```

### 5. Integration with Existing System
Ran tool
```rust
// src/plugins/mod.rs - New plugin management system
pub mod scanner;
pub mod matcher;
pub mod database;

use crate::database::LiveSetDatabase;
use crate::models::{Plugin, PluginFormat};
use scanner::{PluginScanner, ScannedPlugin};
use matcher::PluginMatcher;

pub struct PluginManager {
    scanner: PluginScanner,
    matcher: PluginMatcher,
    db: LiveSetDatabase,
}

impl PluginManager {
    pub fn new(harvester_path: PathBuf, db: LiveSetDatabase) -> Self {
        Self {
            scanner: PluginScanner::new(harvester_path),
            matcher: PluginMatcher,
            db,
        }
    }
    
    /// Full system plugin scan (run on startup or manually)
    pub async fn full_plugin_scan(&mut self) -> Result<PluginScanResult, PluginScanError> {
        log::info!("Starting full plugin system scan...");
        
        let scan_start = chrono::Local::now();
        let scanned_plugins = self.scanner.discover_plugins().await?;
        
        // Update database with scanned plugins
        self.update_plugin_database(&scanned_plugins).await?;
        
        // Update installation status for existing project plugins
        let updated_count = self.refresh_project_plugin_status(&scanned_plugins).await?;
        
        let scan_result = PluginScanResult {
            plugins_found: scanned_plugins.len(),
            plugins_successful: scanned_plugins.iter().filter(|p| p.scan_successful).count(),
            plugins_failed: scanned_plugins.iter().filter(|p| !p.scan_successful).count(),
            project_plugins_updated: updated_count,
            scan_duration: chrono::Local::now() - scan_start,
        };
        
        log::info!("Plugin scan completed: {:?}", scan_result);
        Ok(scan_result)
    }
    
    /// Update plugin installation status for a specific plugin
    pub async fn check_plugin_status(&mut self, plugin: &mut Plugin) -> Result<(), PluginScanError> {
        // Get current scanned plugins
        let scanned_plugins = self.get_scanned_plugins_from_db().await?;
        
        // Try to match this plugin
        if let Some(scanned_plugin) = self.matcher.match_plugin(
            &plugin.dev_identifier,
            &plugin.name,
            plugin.plugin_format,
            &scanned_plugins
        ) {
            // Update plugin with scanned data
            plugin.installed = scanned_plugin.scan_successful && scanned_plugin.is_available;
            plugin.name = scanned_plugin.name.clone();
            plugin.vendor = scanned_plugin.vendor.clone();
            plugin.version = scanned_plugin.version.clone();
        } else {
            plugin.installed = false;
        }
        
        Ok(())
    }
    
    async fn update_plugin_database(&mut self, scanned_plugins: &[ScannedPlugin]) -> Result<(), PluginScanError> {
        // Insert/update scanned plugins in database
        for plugin in scanned_plugins {
            self.db.upsert_scanned_plugin(plugin).await?;
        }
        
        // Mark plugins as unavailable if their files no longer exist
        self.db.mark_missing_plugins_unavailable().await?;
        
        Ok(())
    }
    
    async fn refresh_project_plugin_status(&mut self, scanned_plugins: &[ScannedPlugin]) -> Result<usize, PluginScanError> {
        let mut updated_count = 0;
        
        // Get all plugins referenced in projects
        let project_plugins = self.db.get_all_project_plugins().await?;
        
        for mut plugin in project_plugins {
            let old_status = plugin.installed;
            
            if let Some(scanned_plugin) = self.matcher.match_plugin(
                &plugin.dev_identifier,
                &plugin.name,
                plugin.plugin_format,
                scanned_plugins
            ) {
                plugin.installed = scanned_plugin.scan_successful && scanned_plugin.is_available;
                plugin.name = scanned_plugin.name.clone();
                plugin.vendor = scanned_plugin.vendor.clone();
                plugin.version = scanned_plugin.version.clone();
            } else {
                plugin.installed = false;
            }
            
            if plugin.installed != old_status {
                updated_count += 1;
                self.db.update_plugin_status(&plugin).await?;
            }
        }
        
        Ok(updated_count)
    }
}

#[derive(Debug)]
pub struct PluginScanResult {
    pub plugins_found: usize,
    pub plugins_successful: usize,
    pub plugins_failed: usize,
    pub project_plugins_updated: usize,
    pub scan_duration: chrono::Duration,
}
```

### 6. Migration Plan

```rust
// src/plugins/migration.rs
use crate::ableton_db::AbletonDatabase;
use crate::plugins::PluginManager;

pub struct AbletonPluginMigration;

impl AbletonPluginMigration {
    /// Migrate from Ableton database to internal plugin system
    pub async fn migrate_from_ableton_db(&self, 
                                       ableton_db_path: PathBuf,
                                       plugin_manager: &mut PluginManager) -> Result<MigrationResult, MigrationError> {
        
        log::info!("Starting migration from Ableton database...");
        
        // 1. Read existing Ableton database
        let ableton_db = AbletonDatabase::new(ableton_db_path)?;
        let ableton_plugins = ableton_db.get_database_plugins()?;
        
        log::info!("Found {} plugins in Ableton database", ableton_plugins.len());
        
        // 2. Perform full plugin scan with our system
        let scan_result = plugin_manager.full_plugin_scan().await?;
        
        // 3. Create mapping between old and new systems
        let mapping = self.create_plugin_mapping(&ableton_plugins, &plugin_manager).await?;
        
        // 4. Update existing projects to use new plugin IDs
        let updated_projects = self.update_project_plugin_references(&mapping, &plugin_manager.db).await?;
        
        Ok(MigrationResult {
            ableton_plugins_found: ableton_plugins.len(),
            seula_plugins_scanned: scan_result.plugins_successful,
            plugins_mapped: mapping.len(),
            projects_updated: updated_projects,
            scan_result,
        })
    }
}

#[derive(Debug)]
pub struct MigrationResult {
    pub ableton_plugins_found: usize,
    pub seula_plugins_scanned: usize,
    pub plugins_mapped: usize,
    pub projects_updated: usize,
    pub scan_result: PluginScanResult,
}
```

## Key Benefits of This Architecture

### 1. **Complete Independence**
- No dependency on Ableton database
- Works with any DAW, not just Ableton Live
- Self-contained plugin discovery system

### 2. **Crash Safety**
- Each plugin scanned in separate process
- Timeout protection prevents hanging
- Process isolation prevents crashes from affecting main application

### 3. **Performance**
- Concurrent scanning with configurable limits
- Caching of scan results
- Incremental updates (only scan changed files)

### 4. **Reliability**
- Multiple matching strategies (unique ID, name, fuzzy matching)
- Comprehensive error handling and logging
- Graceful degradation when plugins fail to scan

### 5. **Extensibility**
- Easy to add support for new plugin formats (AU, CLAP, etc.)
- Modular architecture allows for different scanning strategies
- JSON output makes it easy to extend harvester capabilities

## Implementation Timeline

1. **Week 1-2**: Build C++ Plugin Data Harvester
2. **Week 3**: Implement Rust scanner integration  
3. **Week 4**: Create plugin database and matching system
4. **Week 5**: Integration with existing project parsing
5. **Week 6**: Migration tools and testing

This system completely eliminates the Ableton database dependency while providing superior plugin discovery and crash safety through process isolation.
Ran tool
## Summary

I've designed a comprehensive plugin discovery system that completely replaces the Ableton database dependency with a robust, crash-safe, and extensible architecture:

### **Core Components**

1. **Plugin Data Harvester (C++)**: Standalone binary that safely loads and inspects VST2/VST3 plugins using the VST SDK, outputting structured JSON data

2. **Rust Scanner Integration**: Asynchronous, concurrent plugin scanning with process isolation, timeouts, and comprehensive error handling

3. **Internal Plugin Database**: Complete schema for storing scanned plugin data with installation status, metadata, and scan history

4. **Smart Plugin Matching**: Multi-strategy matching system (unique ID, name+vendor, fuzzy matching) to link project plugins with scanned plugins

5. **Migration System**: Tools to transition from Ableton database dependency to the new internal system

### **Key Advantages**

- **🛡️ Crash Safety**: Process isolation prevents plugin crashes from affecting main application
- **🚀 Performance**: Concurrent scanning with configurable limits and result caching  
- **🔄 DAW Agnostic**: Works with any DAW, not just Ableton Live
- **🎯 Reliable Matching**: Multiple fallback strategies for plugin identification
- **📈 Scalable**: Easy to extend for new plugin formats (AU, CLAP, etc.)
- **🔍 Comprehensive**: Extracts detailed metadata including parameters, categories, versions

### **Implementation Strategy**

The system uses a separate C++ binary (`plugin_data_harvester.exe`) that Rust spawns as child processes with timeouts. This approach:

- Isolates crashes to individual plugin scans
- Allows proper VST SDK usage without FFI complexity
- Provides clean JSON interface between C++ and Rust
- Enables easy debugging and testing of plugin scanning

This architecture transforms the plugin system from being Ableton-dependent to being truly universal, while providing better crash safety and more detailed plugin information than the original Ableton database approach.