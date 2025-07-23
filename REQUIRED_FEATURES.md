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
