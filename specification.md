# Ableton Live Project/Set Manager Specification

## 1. Overview
The Studio Project Manager is a comprehensive tool designed to help music producers organize, search, and manage their Ableton Live projects efficiently. It combines powerful backend functionality with an intuitive user interface, catering to both casual users and power users.

## 2. Core Features

### 2.1 Project Scanning and Database Management
- Scan folders containing Ableton Live sets
- Extract and store comprehensive data in a 5NF SQLite database
- Implement efficient scanning to avoid redundant processing of unchanged files

### 2.2 Fuzzy Search Functionality
- Implement a powerful fuzzy search algorithm
- Enable searching based on project contents and metadata
- Provide a CLI tool (`alsm`/`alpm`) for quick queries and folder management

### 2.3 File Watching and Auto-Update
- Implement a toggleable continuous file watcher for added project folders
- Automatically update the database when changes are detected

## Secondary features

## 3. User Interface

### 3.1 General Layout
- Sidebar for main categories (Projects, Plugins, Settings)
- Main content area for search results and detailed views
- Top bar for global actions and search
- Status bar for system messages and file watcher status

### 3.2 Project Management
- Drag-and-drop interface for adding project folders
- Visual feedback for successful additions and scans
- Expandable lists for project details (plugins, samples, etc.)

### 3.3 Search and Filter
- Real-time search results as the user types
- Advanced filtering options (date, plugin, file type, etc.)
- Toggle switches for regex and tag-based searching
- Percentage match display with color coding (0% red to 100% green)

### 3.4 Plugin and Sample Management
- Status indicators for installed/missing plugins and samples
- Quick actions for locating missing items or visiting plugin websites
- Rescanning functionality for installed plugins

### 3.5 Project-specific Features
- To-do lists with prioritization and due dates
- Version control and backup management
- Demo audio file management with playback functionality

### 3.6 Accessibility and Theming
- Keyboard shortcuts for common actions
- High-contrast mode and scalable UI elements
- Light and dark mode options with customizable color schemes

## 4. Advanced Search Features

### 4.1 Regex-Style Field Searching
- Implement field-specific searches (e.g., `plugin:Omnisphere`)
- Design an intuitive syntax for quick, targeted searches

### 4.2 Boolean Operators
- Support OR operators (|) for multi-criteria searches
- Allow grouping of search terms (e.g., `plugin:(Omnisphere | Nexus)`)

### 4.3 Exact Matching
- Enable non-fuzzy, exact matching using double quotation marks

## 5. Extended Features

### 5.1 Version Control System
- Track modifications in projects (location, time, nature of changes)
- Implement rollback functionality

### 5.2 Tagging System
- Allow custom tags (collaborators, genre, etc.)
- Enable filtering and searching by tags

### 5.3 Album Management
- Create and edit album entries
- Manage track lists, cover art, and album notes

### 5.4 Analytics and Visualization
- Implement a "Spotify Wrapped" style statistics tab
- Visualize data on plugin usage, sample frequency, project trends, etc.

### 5.5 Import/Export Functionality
- Support exporting project data in various formats (CSV, JSON)
- Enable importing from other DAWs or project management tools

## 6. Technical Specifications

### 6.1 Main Dependencies
- quick-xml 0.31.0
- thiserror
- log
- rusqlite
- serde
- toml
- once-cell
- regex

### 6.2 Data Structures

#### 6.2.1 LiveSet Struct
- Represents an Ableton Live set
- Methods for data extraction and processing
- Translation functions for database interactions

#### 6.2.2 Database Schema (5NF SQLite)

1. Live Sets Table
    - ID (Primary Key)
    - File path
    - Hash (SHA-1)
    - Last scanned timestamp
    - Project alias (user-editable)
    - Notes
    - File metadata (name, creation time, modification time)
    - Extracted data (Ableton version, key, tempo, time signature, duration, etc.)

2. Plugins Table
    - ID (Primary Key)
    - Plugin name
    - VST version
    - Installation status

3. Samples Table
    - ID (Primary Key)
    - File name
    - Path
    - Extension
    - Presence status

4. LiveSet-Plugins Junction Table
    - Live set ID (Foreign Key)
    - Plugin ID (Foreign Key)

5. LiveSet-Samples Junction Table
    - Live set ID (Foreign Key)
    - Sample ID (Foreign Key)

6. Albums Table
    - ID (Primary Key)
    - Name
    - Cover art file name - stored in relative folder
    - Creation date
    - Last modified date
    - Description

7. Album_LiveSets Table
    - Album ID (Foreign Key)
    - LiveSet ID (Foreign Key)
    - Added date
    - Track number

### 6.3 Key Algorithms
- Fuzzy search implementation
- File hashing for change detection
- Levenshtein distance calculation for match percentages

## 7. Future Enhancements
- macOS support
- Cloud integration for project backup
- Collaborative features for shared projects
- Mobile companion app

## 8. Development Roadmap
1. Core backend development (scanning, database, basic search)
2. Basic CLI tool implementation for proof of concept
3. Basic GUI development
4. Advanced search features
5. Extended features (version control, tagging, album management)
6. Analytics and visualization
7. Import/Export functionality
8. Refinement and optimization
9. macOS port