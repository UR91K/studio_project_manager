# Ableton Live Project Manager

## Overview

A high-performance gRPC server for indexing and searching Ableton Live projects. This tool provides the fastest Ableton Live set parser available, offering comprehensive project analysis, search, and organization capabilities through a clean gRPC API.

The application is now feature-complete. It runs as a system tray application by default for seamless background operation.

## Features

### Core Features

- **Extremely fast scanning and parsing** of Ableton Live Set (.als) files (~160-270 MB/s)
- **System tray application** - runs silently in the background with minimal system impact
- **gRPC API** for remote access and integration with any client application
- **Comprehensive project data extraction**:
    - Tempo
    - Ableton version
    - Time signature
    - Length (bars)
    - Plugins used
    - Samples used (paths)
    - Key + scale
    - Estimated duration
- **Plugin + Sample validation** - per project, check which samples/plugins are present on the system
- **5NF SQLite database** for storing project information
- **FTS5 based search engine** with operators:
    - `plugin:serum` - search by plugin name
    - `bpm:128` - search by tempo
    - `key:Cmaj` - search by key signature
    - `missing:true` - find projects with missing plugins
    - And more fuzzy search capabilities across all project data
- **Real-time file watching** with gRPC streaming integration
- **Notes** - descriptions for each project
- **Tags** - tag projects for categorization (e.g., artists, genres)
- **Collections** - for making tracklists; collects to-do lists of contained projects, support for cover art
- **Tasks/To-do lists** per project for mix notes, reminders, and project management
- **Batch operations** - perform bulk actions on multiple projects, tags, collections, and tasks for efficient project management
- **Media management** - upload/download cover art and audio files with storage statistics and cleanup
- **Advanced analytics** - collection-level statistics, task completion trends, and historical analytics
- **Data export** - CSV export of statistics and analytics data
- **Database statistics** with enhanced filtering (date ranges, collections, tags, Ableton versions)
- **Configurable settings** via `config.toml`

### gRPC API Endpoints

**Project Management**

- `GetProjects` - List all projects with pagination
- `GetProject` - Get detailed information about a specific project
- `UpdateProjectNotes` - Add or update project notes
- `UpdateProjectName` - Update a project's name

**Batch Operations**

- `BatchArchiveProjects` - Mark multiple projects as archived
- `BatchDeleteProjects` - Delete multiple projects permanently
- `BatchTagProjects` - Add tags to multiple projects
- `BatchUntagProjects` - Remove tags from multiple projects
- `BatchAddProjectsToCollection` - Add multiple projects to a collection
- `BatchRemoveProjectsFromCollection` - Remove multiple projects from a collection
- `BatchCreateCollectionFromProjects` - Create a new collection from multiple projects
- `BatchUpdateTaskStatus` - Update status of multiple tasks
- `BatchDeleteTasks` - Delete multiple tasks

**Search**

- `Search` - Advanced search with FTS5 and specialized operators

**Scanning**

- `ScanDirectories` - Trigger project scanning with progress streaming
- `GetScanStatus` - Get the current status of the scan

**Collections**

- `GetCollections` - Get all collections with aggregated statistics (duration, project count)
- `CreateCollection` - Create a new collection
- `UpdateCollection` - Update an existing collection
- `AddProjectToCollection` - Add a project to a collection
- `RemoveProjectFromCollection` - Remove a project from a collection
- `GetCollectionTasks` - Get consolidated task view for all projects in a collection

**Tags**

- `GetTags` - Get all available tags
- `CreateTag` - Create a new tag
- `TagProject` - Add a tag to a project
- `UntagProject` - Remove a tag from a project

**Tasks**

- `GetProjectTasks` - Get all tasks for a specific project
- `CreateTask` - Create a new task for a project
- `UpdateTask` - Update an existing task
- `DeleteTask` - Delete a task

**File Watching**

- `StartWatcher` - Start watching specified directories for changes
- `StopWatcher` - Stop the file watcher
- `GetWatcherEvents` - Stream file system events (returns streaming responses)

**System Info**

- `GetSystemInfo` - Get system and database information
- `GetStatistics` - Get database statistics with advanced filtering (date ranges, collections, tags, Ableton versions)
- `ExportStatistics` - Export statistics and analytics data to CSV format

**Media Management**

- `UploadCoverArt` - Upload cover art for collections (streaming)
- `UploadAudioFile` - Upload audio files for projects (streaming)
- `DownloadMedia` - Download media files (streaming)
- `DeleteMedia` - Delete media files
- `SetCollectionCoverArt` - Associate cover art with a collection
- `RemoveCollectionCoverArt` - Remove cover art from a collection
- `SetProjectAudioFile` - Associate audio file with a project
- `RemoveProjectAudioFile` - Remove audio file from a project
- `ListMediaFiles` - List all media files with pagination
- `GetMediaFilesByType` - Get media files filtered by type
- `GetOrphanedMediaFiles` - Find media files not associated with any project/collection
- `GetMediaStatistics` - Get media storage statistics
- `CleanupOrphanedMedia` - Remove orphaned media files from storage

### Future Enhancements

- **Version control system** - track project changes over time
- **Audio file integration** - reference and play demo audio files for auditioning
- **Analytics dashboard frontend** - visual dashboard for the existing analytics backend ("Ableton Wrapped" style)
- **macOS support** - currently Windows-focused

## Installation

### Prerequisites

- **Rust 1.70 or higher**
- **SQLite 3.35.0 or higher**
- **Protocol Buffers compiler** (`protoc`)
  - Windows: `choco install protoc` (using Chocolatey)

### Building from source

1. Clone the repository:
```bash
git clone <repository-url>
cd studio_project_manager
```

2. Build the project:
```bash
cargo build --release
```

3. Run the application:
```bash
cargo run --release
```

The application will start in system tray mode by default, with the gRPC server running on `localhost:50051`.

## Configuration

The application is configured via `config.toml`. All settings are optional with sensible defaults:

```toml
# Project directories to scan
paths = [
    '{USER_HOME}/Documents/Ableton Live Projects',
    '{USER_HOME}/Music/Ableton Projects'
]

# Database location (optional - defaults to executable directory)
database_path = ""

# Ableton Live database directory for plugin detection
live_database_dir = '{USER_HOME}/AppData/Local/Ableton/Live Database'

# gRPC server port (default: 50051)
grpc_port = 50051

# Log level: error, warn, info, debug, trace (default: info)
log_level = "info"
```

### Configuration Options

- **`paths`** - Array of project directories to scan
- **`database_path`** - SQLite database location (leave empty for executable directory)
- **`live_database_dir`** - Ableton Live's database directory for plugin detection
- **`grpc_port`** - Port for the gRPC server (default: 50051)
- **`log_level`** - Logging verbosity level (default: "info")

The `{USER_HOME}` placeholder will be automatically replaced with your user directory.

## Usage

### System Tray Mode (Default)

```bash
# Start as system tray application
./studio_project_manager.exe

# Or with cargo
cargo run --release
```

The application will:
1. Load configuration from `config.toml`
2. Initialize the SQLite database
3. Start the gRPC server
4. Run silently in the system tray

Right-click the tray icon to quit the application.

### CLI Mode

```bash
# Start in CLI mode (shows logs in terminal)
./studio_project_manager.exe --cli

# Or with cargo
cargo run --release -- --cli
```

Use CLI mode for debugging or when you want to see log output directly.

### Testing with grpcurl

You can test the API using `grpcurl`:

```bash
# Get system information
grpcurl -plaintext -proto proto/studio_project_manager.proto localhost:50051 studio_project_manager.StudioProjectManager/GetSystemInfo

# Get all projects
grpcurl -plaintext -proto proto/studio_project_manager.proto localhost:50051 studio_project_manager.StudioProjectManager/GetProjects

# Search for projects
grpcurl -plaintext -proto proto/studio_project_manager.proto -d '{"query": "plugin:serum"}' localhost:50051 studio_project_manager.StudioProjectManager/Search

# Search by tempo
grpcurl -plaintext -proto proto/studio_project_manager.proto -d '{"query": "bpm:128"}' localhost:50051 studio_project_manager.StudioProjectManager/Search

# Start file watcher
grpcurl -plaintext -proto proto/studio_project_manager.proto -d '{"directories": ["C:/Users/YourName/Documents/Ableton Live Projects"]}' localhost:50051 studio_project_manager.StudioProjectManager/StartWatcher

# Get media statistics
grpcurl -plaintext -proto proto/studio_project_manager.proto localhost:50051 studio_project_manager.StudioProjectManager/GetMediaStatistics

# List media files
grpcurl -plaintext -proto proto/studio_project_manager.proto -d '{"limit": 10}' localhost:50051 studio_project_manager.StudioProjectManager/ListMediaFiles

# Get orphaned media files
grpcurl -plaintext -proto proto/studio_project_manager.proto localhost:50051 studio_project_manager.StudioProjectManager/GetOrphanedMediaFiles

# Get statistics with date range filtering
grpcurl -plaintext -proto proto/studio_project_manager.proto -d '{"date_range": {"start_date": "2024-01-01", "end_date": "2024-12-31"}}' localhost:50051 studio_project_manager.StudioProjectManager/GetStatistics

# Get statistics filtered by collection
grpcurl -plaintext -proto proto/studio_project_manager.proto -d '{"collection_ids": [1, 2, 3]}' localhost:50051 studio_project_manager.StudioProjectManager/GetStatistics

# Get consolidated tasks for a collection
grpcurl -plaintext -proto proto/studio_project_manager.proto -d '{"collection_id": 1}' localhost:50051 studio_project_manager.StudioProjectManager/GetCollectionTasks

# Export statistics to CSV
grpcurl -plaintext -proto proto/studio_project_manager.proto -d '{"format": "EXPORT_CSV"}' localhost:50051 studio_project_manager.StudioProjectManager/ExportStatistics

# Batch operations examples
# Archive multiple projects
grpcurl -plaintext -proto proto/studio_project_manager.proto -d '{"project_ids": [1, 2, 3]}' localhost:50051 studio_project_manager.StudioProjectManager/BatchArchiveProjects

# Tag multiple projects
grpcurl -plaintext -proto proto/studio_project_manager.proto -d '{"project_ids": [1, 2, 3], "tag_ids": [1, 2]}' localhost:50051 studio_project_manager.StudioProjectManager/BatchTagProjects

# Add multiple projects to a collection
grpcurl -plaintext -proto proto/studio_project_manager.proto -d '{"project_ids": [1, 2, 3], "collection_id": 1}' localhost:50051 studio_project_manager.StudioProjectManager/BatchAddProjectsToCollection

# Create collection from projects
grpcurl -plaintext -proto proto/studio_project_manager.proto -d '{"project_ids": [1, 2, 3], "name": "My New Collection", "description": "Collection created from batch operation"}' localhost:50051 studio_project_manager.StudioProjectManager/BatchCreateCollectionFromProjects

# Update multiple task statuses
grpcurl -plaintext -proto proto/studio_project_manager.proto -d '{"task_ids": [1, 2, 3], "status": "COMPLETED"}' localhost:50051 studio_project_manager.StudioProjectManager/BatchUpdateTaskStatus
```

### Client Integration

The gRPC service can be integrated with any language that supports gRPC. The protobuf definitions are available in `proto/studio_project_manager.proto`.

## Deployment

1. **Build the release binary:**
   ```bash
   cargo build --release
   ```

2. **Copy the executable and config:**
   ```
   studio_project_manager.exe
   config.toml
   ```

3. **Run the application:**
   - Double-click the executable for tray mode
   - Or run from command line: `./studio_project_manager.exe`

The application will automatically:
- Create the database if it doesn't exist
- Start the gRPC server
- Run in the system tray
- Begin scanning configured directories

## Performance

### Scanning Performance Benchmarks

**Cold Scan (First Run - 3,570 projects)**
- **Scanning speed**: 38.6 projects/sec (average), 43.7 projects/sec (peak)
- **Total time**: 92.52 seconds
- **Time per project**: 0.026 seconds
- **Throughput**: 2,315 projects/minute

**Warm Scan (Subsequent Runs - 3,570 projects)**
- **Scanning speed**: 860.7 projects/sec (average), 860.8 projects/sec (peak)
- **Total time**: 4.15 seconds
- **Time per project**: 0.001 seconds
- **Throughput**: 51,645 projects/minute

### System Resources

- **Memory usage**: Minimal - designed for long-running operation
- **Database**: SQLite with FTS5 for fast full-text search
- **Concurrency**: Multi-threaded scanning and processing

### Time Estimates

| Projects | Cold Scan | Warm Scan |
|----------|-----------|-----------|
| 100      | 2.6s      | 0.1s      |
| 500      | 13.0s     | 0.6s      |
| 1000     | 25.9s     | 1.2s      |

*Benchmarks conducted on modern hardware with 3,570 Ableton Live projects*

## Contributing

Contributions are welcome! Feel free to submit a PR, but please open an issue first for large contributions.

## License

[License information]
