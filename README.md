# Ableton Live Project Manager

## Overview

A tool which aims to let users create an index of their Ableton Live sets and search them based on their contents. As far as I know, this has the fastest ableton live set parser out there.

## Features

### Currently implemented

- Efficient parsing of Ableton Live Set (.als) files (~130-270 MB/s)
- Extract the following project data:
    - Tempo
    - Ableton version
    - Time signature
    - Length (bars)
    - Plugins used
    - Samples used
    - Key + scale
    - Estimated duration
- Check which of the plugins in a project are installed
- Efficient 5NF database for storing project information
- FTS5 fuzzy searching across all project data
- Tags
- Collections for planning albums and EPs
- Clean Tauri GUI (work in progress)

### Planned - lower priority

- Version control system
- Album management system for creating albums of Ableton Live sets, notes, album art, etc.
- Ability to reference and play demo audio files to audition sets.
- Statistics tab, almost like "Ableton Wrapped" with information on things like plugin usage, most common tempos, etc.
- macOS support

## Installation

### Prerequisites

- Rust 1.84 or higher (for building the project)
- Ableton Live - any version, 8-12 are officially supported
  - the Ableton Live database is used to check which plugins are installed.

### Building from source

Warning: the project is not ready to be run on other people's machines and currently won't do anything unless you change the hard-coded file paths in the main function.

Just clone, then build and run with cargo.

## Configuration

Edit the `config.toml` file to set up your project directories and database location:

```toml
paths = [
    '{USER_HOME}\Documents\Ableton Live Projects'
]

database_path = '{USER_HOME}\Documents\ableton_manager\ableton_live_sets.db'

live_database_dir = '{USER_HOME}\AppData\Local\Ableton\Live Database'
```

## Usage

Usage will be provided once the app is functional.

## Contributing

Contributions are welcome! Feel free to submit a PR, but please rather open an issue for large contributions.