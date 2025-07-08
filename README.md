# Ableton Live Project Manager

## Overview

A high-performance gRPC server for indexing and searching Ableton Live projects. This tool provides the fastest Ableton Live set parser available. It offers comprehensive project analysis and search, and organisation capabilities through a clean gRPC API.

## Features

### Currently implemented

- **Extremely fast scanning and parsing** of Ableton Live Set (.als) files (~160-270 MB/s)
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
- **Real-time file watching** (planned integration with gRPC streaming)
- **Notes** - a description for each project
- **Database statistics** and system information endpoints

### gRPC API Endpoints

**Project Management**

- `GetProjects` - List all projects with pagination
- `GetProject` - Get detailed information about a specific project
- `UpdateProjectNotes` - Add or update project notes

**Search**

- `Search` - Advanced search with FTS5 and specialized operators

**Scanning**

- `ScanDirectories` - Trigger project scanning with progress streaming
- `GetScanStatus` - Get the current status of the scan

**Collections**

- `GetCollections` - Get all the current collections
- `CreateCollection` - Create a new collection
- `UpdateCollection` - Update an existing collection
- `AddProjectToCollection` - Add a project to a collection
- `RemoveProjectFromCollection` - Remove a project from a collection

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
- `GetStatistics` - Get database statistics

### Coming soon - high priority

- **To-do lists** per project for mix notes, etc. (already implemented internally)
- **Tags** - tag projects for categorisation (e.g., artists, genres) [already implemented internally]
- **Collections** - for making tracklists; collects to-do lists of contained projects, support for cover art [already implemented internally]
- **Real-time file watcher streaming** - get notified of project changes
- **Configuration management** - add/remove project directories via API

### Planned - lower priority

- **Version control system** - track project changes over time
- **Audio file integration** - reference and play demo audio files for auditioning
- **Analytics dashboard** - "Ableton Wrapped" style statistics
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

3. Run the server:
```bash
cargo run --release
```

The gRPC server will start on `localhost:50051` by default.

## Configuration

Edit the `config.toml` file to set up your project directories and database location:

```toml
paths = [
    '{USER_HOME}/Documents/Ableton Live Projects',
    '{USER_HOME}/Music/Ableton Projects'
]

database_path = '{USER_HOME}/Documents/ableton_manager/ableton_live_sets.db'

live_database_dir = '{USER_HOME}/AppData/Local/Ableton/Live Database'
```

`paths` is for your project folders
`database_path` is the path where you want your database to be stored. Leave blank for same directory as the executable.
`live_database_dir` is the directory where Ableton stores it's database files (used during plugin detection.) Usually this will have a bunch of files with names like `Live-files-1218.db` in it.

The `{USER_HOME}` placeholder will be automatically replaced with your user directory.

## Usage

### Starting the server

```bash
cargo run --release
```

The server will:
1. Load configuration from `config.toml`
2. Initialize the SQLite database
3. Start the gRPC server on `localhost:50051`

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
```

### Client Integration

The gRPC service can be integrated with any language that supports gRPC. The protobuf definitions are available in `proto/studio_project_manager.proto`.

## Contributing

Contributions are welcome! Feel free to submit a PR, but please open an issue first for large contributions.

## License

[License information]
