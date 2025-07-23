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
