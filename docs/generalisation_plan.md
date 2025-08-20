CLAUDE4:
## Current Coupling Analysis

1. **`LiveSet` struct** - Hardcoded for Ableton with `ableton_version`, Ableton-specific parsing
2. **Database schema** - Tables named with Ableton-specific fields like `ableton_version_*`
3. **Parser** - Hardcoded XML parser for `.als` files only
4. **File validation** - Only validates `.als` extensions
5. **Ableton database integration** - Plugin validation against Ableton's database

Now let me design the generalized architecture:
Ran tool
Ran tool
## Generalized Architecture Design

### 1. Generic Project Model

Here's how I would design the generic `Project` struct to replace `LiveSet`:

```rust
// Core generic project model
#[derive(Debug, Clone)]
pub struct Project {
    // Universal metadata
    pub id: Uuid,
    pub file_path: PathBuf,
    pub name: String,
    pub file_hash: String,
    pub created_time: DateTime<Local>,
    pub modified_time: DateTime<Local>,
    pub last_parsed_timestamp: DateTime<Local>,
    pub is_active: bool,
    
    // Musical properties (common across DAWs)
    pub tempo: Option<f64>,
    pub time_signature: Option<TimeSignature>,
    pub key_signature: Option<KeySignature>,
    pub estimated_duration: Option<chrono::Duration>,
    pub furthest_bar: Option<f64>,
    
    // Asset collections
    pub plugins: HashSet<Plugin>,
    pub samples: HashSet<Sample>,
    pub tags: HashSet<String>,
    
    // DAW identification
    pub daw_type: DawType,
    pub daw_version: DawVersion,
    
    // DAW-specific data (stored as JSON)
    pub daw_specific_data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DawType {
    AbletonLive,
    Reaper,
    FLStudio,
    LogicPro,
    Cubase,
    StudioOne,
    ProTools,
    Custom(String), // For plugin parsers
}

#[derive(Debug, Clone)]
pub enum DawVersion {
    Ableton(AbletonVersion),
    Reaper { major: u32, minor: u32, patch: u32 },
    FLStudio { major: u32, minor: u32, patch: u32 },
    Logic { major: u32, minor: u32, patch: u32 },
    Custom { version_string: String },
}
```

### 2. Parser Architecture

```rust
// Core parser trait that all DAW parsers must implement
pub trait DawParser: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;
    
    fn can_parse(&self, file_path: &Path) -> bool;
    fn parse(&self, file_path: &Path) -> Result<Project, Self::Error>;
    fn get_daw_type(&self) -> DawType;
    fn get_supported_extensions(&self) -> Vec<&'static str>;
}

// Parser registry for managing all parsers
pub struct ParserRegistry {
    parsers: Vec<Box<dyn DawParser<Error = Box<dyn std::error::Error + Send + Sync>>>>,
}

impl ParserRegistry {
    pub fn new() -> Self {
        let mut registry = Self { parsers: Vec::new() };
        
        // Register built-in parsers
        registry.register(Box::new(AbletonParser::new()));
        // Future parsers would be registered here
        // registry.register(Box::new(ReaperParser::new()));
        // registry.register(Box::new(FLStudioParser::new()));
        
        registry
    }
    
    pub fn register(&mut self, parser: Box<dyn DawParser<Error = Box<dyn std::error::Error + Send + Sync>>>) {
        self.parsers.push(parser);
    }
    
    pub fn parse_file(&self, file_path: &Path) -> Result<Project, ParseError> {
        for parser in &self.parsers {
            if parser.can_parse(file_path) {
                return parser.parse(file_path)
                    .map_err(|e| ParseError::ParserError(e));
            }
        }
        Err(ParseError::UnsupportedFileType(file_path.to_path_buf()))
    }
}

// File type detection
pub struct FileTypeDetector;

impl FileTypeDetector {
    pub fn detect_daw_type(file_path: &Path) -> Option<DawType> {
        match file_path.extension()?.to_str()? {
            "als" => Some(DawType::AbletonLive),
            "rpp" => Some(DawType::Reaper),
            "flp" => Some(DawType::FLStudio),
            "logicx" => Some(DawType::LogicPro),
            "cpr" => Some(DawType::Cubase),
            "song" => Some(DawType::StudioOne),
            "ptx" => Some(DawType::ProTools),
            _ => None,
        }
    }
    
    pub fn is_supported_project_file(file_path: &Path) -> bool {
        Self::detect_daw_type(file_path).is_some()
    }
}
```

### 3. Ableton Parser Implementation

```rust
// Refactored Ableton parser implementing the generic trait
pub struct AbletonParser {
    ableton_db: Option<AbletonDatabase>,
}

impl DawParser for AbletonParser {
    type Error = LiveSetError;
    
    fn can_parse(&self, file_path: &Path) -> bool {
        file_path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext == "als")
            .unwrap_or(false)
    }
    
    fn parse(&self, file_path: &Path) -> Result<Project, Self::Error> {
        // Use existing LiveSet parsing logic
        let live_set = LiveSet::new(file_path.to_path_buf())?;
        
        // Convert to generic Project
        Ok(Project {
            id: live_set.id,
            file_path: live_set.file_path,
            name: live_set.name,
            file_hash: live_set.file_hash,
            created_time: live_set.created_time,
            modified_time: live_set.modified_time,
            last_parsed_timestamp: live_set.last_parsed_timestamp,
            is_active: live_set.is_active,
            
            tempo: Some(live_set.tempo),
            time_signature: Some(live_set.time_signature),
            key_signature: live_set.key_signature,
            estimated_duration: live_set.estimated_duration,
            furthest_bar: live_set.furthest_bar,
            
            plugins: live_set.plugins,
            samples: live_set.samples,
            tags: live_set.tags,
            
            daw_type: DawType::AbletonLive,
            daw_version: DawVersion::Ableton(live_set.ableton_version),
            daw_specific_data: None, // Could store Ableton-specific metadata here
        })
    }
    
    fn get_daw_type(&self) -> DawType {
        DawType::AbletonLive
    }
    
    fn get_supported_extensions(&self) -> Vec<&'static str> {
        vec!["als"]
    }
}
```

### 4. Database Schema Changes

```sql
-- Updated projects table to support multiple DAWs
CREATE TABLE IF NOT EXISTS projects (
    is_active BOOLEAN NOT NULL DEFAULT true,
    
    id TEXT PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    hash TEXT NOT NULL,
    notes TEXT,
    created_at DATETIME NOT NULL,
    modified_at DATETIME NOT NULL,
    last_parsed_at DATETIME NOT NULL,
    
    -- Musical properties (nullable for DAWs that don't support them)
    tempo REAL,
    time_signature_numerator INTEGER,
    time_signature_denominator INTEGER,
    key_signature_tonic TEXT,
    key_signature_scale TEXT,
    duration_seconds INTEGER,
    furthest_bar REAL,
    
    -- DAW identification
    daw_type TEXT NOT NULL,
    daw_version_major INTEGER NOT NULL,
    daw_version_minor INTEGER NOT NULL,
    daw_version_patch INTEGER NOT NULL,
    daw_version_data TEXT, -- JSON for complex version data
    
    -- DAW-specific data stored as JSON
    daw_specific_data TEXT, -- JSON blob for DAW-specific metadata
    
    audio_file_id TEXT,
    FOREIGN KEY (audio_file_id) REFERENCES media_files(id) ON DELETE SET NULL
);

-- Add indexes for DAW type queries
CREATE INDEX IF NOT EXISTS idx_projects_daw_type ON projects(daw_type);
CREATE INDEX IF NOT EXISTS idx_projects_daw_version ON projects(daw_type, daw_version_major, daw_version_minor);
```

### 5. Plugin Parser System

For extensibility, you could allow users to add custom parsers:

```rust
// Plugin system for custom parsers
pub trait PluginParser {
    fn parser_info(&self) -> ParserInfo;
    fn create_parser(&self) -> Box<dyn DawParser<Error = Box<dyn std::error::Error + Send + Sync>>>;
}

#[derive(Debug)]
pub struct ParserInfo {
    pub name: String,
    pub version: String,
    pub supported_extensions: Vec<String>,
    pub daw_type: DawType,
}

// Example custom parser
pub struct ReaperParser;

impl DawParser for ReaperParser {
    type Error = Box<dyn std::error::Error + Send + Sync>;
    
    fn can_parse(&self, file_path: &Path) -> bool {
        file_path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext == "rpp")
            .unwrap_or(false)
    }
    
    fn parse(&self, file_path: &Path) -> Result<Project, Self::Error> {
        // Parse Reaper project file (.rpp is text-based)
        let content = std::fs::read_to_string(file_path)?;
        
        let mut project = Project {
            id: Uuid::new_v4(),
            file_path: file_path.to_path_buf(),
            name: file_path.file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            file_hash: calculate_file_hash(file_path)?,
            created_time: get_file_created_time(file_path)?,
            modified_time: get_file_modified_time(file_path)?,
            last_parsed_timestamp: Local::now(),
            is_active: true,
            
            tempo: None,
            time_signature: None,
            key_signature: None,
            estimated_duration: None,
            furthest_bar: None,
            
            plugins: HashSet::new(),
            samples: HashSet::new(),
            tags: HashSet::new(),
            
            daw_type: DawType::Reaper,
            daw_version: DawVersion::Reaper { major: 6, minor: 0, patch: 0 }, // Parse from file
            daw_specific_data: None,
        };
        
        // Parse Reaper-specific content
        self.parse_reaper_content(&content, &mut project)?;
        
        Ok(project)
    }
    
    fn get_daw_type(&self) -> DawType {
        DawType::Reaper
    }
    
    fn get_supported_extensions(&self) -> Vec<&'static str> {
        vec!["rpp"]
    }
}

impl ReaperParser {
    fn parse_reaper_content(&self, content: &str, project: &mut Project) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Parse Reaper project format
        // This would be specific to Reaper's text-based format
        for line in content.lines() {
            if line.trim().starts_with("TEMPO") {
                // Parse tempo: TEMPO 120 4 4
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    project.tempo = parts[1].parse().ok();
                }
            }
            // Parse other Reaper-specific elements...
        }
        Ok(())
    }
}
```

### 6. Migration Strategy
Ran tool
## Step-by-Step Implementation Plan

Here's how I would approach implementing this generalized architecture:

### Phase 1: Foundation (2-3 weeks)
1. **Create generic models module** (`src/models/project.rs`)
   - Define `Project`, `DawType`, `DawVersion` structs
   - Keep existing `LiveSet` for backward compatibility
   
2. **Create parser traits** (`src/parsers/mod.rs`)
   - Define `DawParser` trait
   - Create `ParserRegistry`
   - Create `FileTypeDetector`

3. **Database migration**
   - Add migration script to update schema
   - Add DAW-agnostic columns
   - Migrate existing Ableton projects

### Phase 2: Ableton Integration (1-2 weeks)
4. **Refactor Ableton parser**
   - Implement `DawParser` trait for `AbletonParser`
   - Create conversion from `LiveSet` to `Project`
   - Update database operations to use generic `Project`

5. **Update scanning system**
   - Modify file scanner to use `ParserRegistry`
   - Update batch operations for generic projects
   - Test with existing Ableton projects

### Phase 3: New DAW Support (2-4 weeks each)
6. **Implement Reaper parser**
   - Create `ReaperParser` implementing `DawParser`
   - Parse `.rpp` text format
   - Extract plugins, samples, tempo, etc.

7. **Implement FL Studio parser**
   - Create `FLStudioParser` for `.flp` files
   - Handle binary format parsing
   - Map FL Studio concepts to generic model

### Phase 4: Plugin System (2-3 weeks)
8. **Create plugin architecture**
   - Design plugin loading system
   - Create plugin API documentation
   - Implement runtime parser registration

9. **Configuration and UI**
   - Add DAW type filtering to UI
   - Update search to work across DAW types
   - Add DAW-specific views and statistics

### Phase 5: Testing and Polish (1-2 weeks)
10. **Comprehensive testing**
    - Unit tests for all parsers
    - Integration tests with real project files
    - Performance testing with mixed DAW libraries

## Key Benefits of This Architecture

1. **Extensibility**: Easy to add new DAW support
2. **Plugin System**: Users can add custom parsers
3. **Backward Compatibility**: Existing Ableton projects continue to work
4. **Performance**: Efficient file type detection and parsing
5. **Database Flexibility**: Schema supports any DAW type
6. **Type Safety**: Strong typing prevents DAW-specific data mixing

## Migration Considerations

1. **Existing Data**: All existing Ableton projects would be migrated automatically
2. **API Compatibility**: gRPC API could remain largely unchanged
3. **Configuration**: Add DAW-specific settings to config
4. **Performance**: Parser registry lookup is O(n) but n is small
5. **Memory**: Generic model slightly larger than `LiveSet` but not significantly

This architecture provides a solid foundation for supporting multiple DAW formats while maintaining the existing functionality and performance characteristics of your Ableton Live project scanner.
Ran tool
## Summary

I've designed a comprehensive architecture to generalize your Seula project beyond Ableton Live. Here are the key components:

### **Core Architecture Changes**

1. **Generic `Project` Model**: Replaces `LiveSet` with a DAW-agnostic structure that includes:
   - Universal metadata (file info, timestamps, etc.)
   - Common musical properties (tempo, key, time signature)
   - Asset collections (plugins, samples, tags)
   - DAW identification and version info
   - Optional DAW-specific data storage (JSON)

2. **Parser System**: 
   - `DawParser` trait that all parsers implement
   - `ParserRegistry` for managing and discovering parsers
   - `FileTypeDetector` for automatic file type detection
   - Plugin system for user-contributed parsers

3. **Database Schema**: Updated to support multiple DAWs while maintaining performance and existing data

### **Implementation Strategy**

The 5-phase approach ensures:
- **Backward compatibility** with existing Ableton projects
- **Incremental rollout** with each DAW as a separate phase
- **Extensible plugin system** for community contributions
- **Comprehensive testing** throughout

### **Key Benefits**

- **Future-proof**: Easy to add support for Logic Pro, Cubase, Studio One, etc.
- **Community-driven**: Plugin system allows users to contribute parsers
- **Performance**: Efficient file detection and parsing
- **Type-safe**: Strong Rust typing prevents data corruption
- **Flexible**: DAW-specific data can be stored without affecting common operations

This architecture transforms Seula from an Ableton-specific tool into a universal DAW project manager, while preserving all existing functionality and maintaining the high performance standards of your current implementation.

The most critical first step would be implementing the generic `Project` model and parser traits, then migrating the existing Ableton parser to use this new system. This provides the foundation for all future DAW support.

---

**Suggested git commit message for this architectural planning:**
```
feat: Design multi-DAW architecture for universal project support

- Analyzed current Ableton Live coupling in codebase
- Designed generic Project model to replace LiveSet
- Created pluggable DawParser trait system with registry
- Planned database schema changes for DAW-agnostic storage
- Outlined 5-phase implementation strategy with plugin system
- Maintains backward compatibility with existing Ableton projects
```