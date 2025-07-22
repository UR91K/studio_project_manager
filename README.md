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

Use CLI mode for debugging or when you want to see log output directly. The log level is configurable in `config.toml`

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

You could then create or use any frontend you would like, or manually interact with the server using grpcurl.
I am currently working on a first party frontend.

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
