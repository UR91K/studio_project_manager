# Required Features

This document outlines the required features and endpoints for the Studio Project Manager backend services.

## Collections Service

### High Priority
- [ ] **SearchCollections** - Search collections by name, description, or notes
- [ ] **Pagination support** for GetCollections (limit, offset, sort_by, sort_desc)
- [ ] **GetCollectionStatistics** - Get detailed statistics for status bars/overviews
- [ ] **DuplicateCollection** - Create a copy of an existing collection
- [ ] **MergeCollections** - Merge two collections with configurable dominant collection

### Medium Priority
- [ ] **CreateCollectionFromData** - Create collection with pre-populated project list (for import scenarios)
- [ ] **RollbackLastOperation** - Rollback the last operation on a collection (if undo system is implemented)

### Not in Scope
- FilterCollections (frontend responsibility)
- ExportCollection (frontend responsibility)
- ImportCollection (frontend responsibility)
- Collection templates
- Collection sharing/collaboration
- ValidateCollection (redundant with project scanning)
- GetCollectionHealth (redundant with project status flags)
- Collection history (frontend responsibility)
- MoveProjectsBetweenCollections (achievable via existing endpoints)
- Additional metadata management (already covered by existing endpoints)

## Plugins Service

### High Priority
- [ ] **GetPlugin** - Get individual plugin details by ID
- [ ] **GetPluginAnalytics** - Get advanced plugin statistics for status bars/overviews

### Medium Priority
- [ ] **Enhanced GetAllPlugins filtering** - Add vendor_filter, format_filter, installed_only, min_usage_count to existing endpoint
- [ ] **GetPluginVendors** - Get list of all vendors with statistics (if dedicated vendor management UI needed)
- [ ] **GetPluginFormats** - Get list of all formats with statistics (if dedicated format management UI needed)

### Not in Scope
- Plugin CRUD operations (plugins are discovered from .als files)
- Plugin installation management (handled by Ableton)
- Plugin configuration
- Plugin export/import (frontend responsibility)
- Plugin templates
- Plugin sharing (frontend responsibility)
- BatchGetPlugins (not needed at this time)

## Projects Service

### High Priority
- [ ] **Enhanced GetProjects filtering** - Add tempo range, key signature, time signature, Ableton version, date ranges, has_audio_file filters
- [ ] **GetProjectStatistics** - Get project statistics for dashboards/overviews (counts, distributions, averages)

### Medium Priority
- [ ] **RescanProject** - Rescan individual project for validation/updates

### Not in Scope
- Project creation (projects are discovered from .als files)
- Project import (projects must be created via scanning)
- Project export (frontend responsibility)
- Project duplication (would violate scanning-only constraint)
- Project validation (handled by rescanning)
- Plugin CRUD operations (would be pointless given scanning architecture)

## Samples Service

### High Priority
- [ ] **GetSample** - Get individual sample details by ID
- [ ] **Enhanced GetAllSamples filtering** - Add present_only, missing_only, extension_filter, usage_count range filters
- [ ] **GetSampleAnalytics** - Get advanced sample statistics for status bars/overviews

### Medium Priority
- [ ] **GetSampleExtensions** - Get extension statistics for filtering UI and storage analysis

### Not in Scope
- Sample CRUD operations (samples are discovered from .als files)
- Sample file management (handled by scanning system)
- Sample upload/download (frontend responsibility)
- Sample conversion (frontend responsibility)
- Sample sharing (frontend responsibility)
- BatchGetSamples (no clear use case identified)

## Services to Review

The following services need similar analysis and endpoint review:

- [ ] Projects Service
- [ ] Media Service
- [ ] Plugins Service
- [ ] Samples Service
- [ ] Tags Service
- [ ] Tasks Service
- [ ] Search Service
- [ ] System Service
- [ ] Scanning Service
- [ ] Watcher Service

## Process

For each service:
1. Analyze existing endpoints in proto files
2. Identify missing frontend requirements
3. Review and refine scope based on architectural decisions
4. Document required features with priority levels
5. Implement endpoints following established patterns
