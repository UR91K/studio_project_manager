# Ableton Live Project Manager

## Overview

A tool which aims to let users create an index of their Ableton Live sets and search them based on their contents

## Features

### Currently implemented

- Fast scanning and analyzing of Ableton Live Set (.als) files
- Extract the following project data:
    - Tempo
    - Ableton version
    - Time signature
    - Length (bars)
    - Plugins used
    - Samples used
- Check which plugins are missing in a project


### Coming very soon - high priority
- Extract Key + scale, time duration,
- Efficient 5NF database for storing project information
- Fast, fuzzy searching across all project data
- GUI with a search bar, user-friendly database viewer, and ability to add folders, etc.

### Planned - lower priority

- Version control system
- Album management system for creating albums of Ableton Live sets, notes, album art, etc.
- Ability to reference and play demo audio files to audition sets.
- Statistics tab, almost like "Ableton Wrapped" with information on things like plugin usage, most common tempos, etc.
- macOS support

## Installation

### Prerequisites

- Rust 1.54 or higher
- SQLite 3.35.0 or higher

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