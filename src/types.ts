export interface Project {
  id: string
  name: string
  filename: string
  modified: string
  created: string
  last_scanned: string
  time_signature: string
  key_scale: string | null
  duration: string | null
  ableton_version: string
  plugins: Plugin[]
  samples: Sample[]
  // Future features
  demo?: boolean  // For audio preview feature
}

export interface Plugin {
  id: string
  name: string
  vendor: string | null
  version: string | null
  format: string
  installed: boolean
}

export interface Sample {
  id: string
  name: string
  path: string
  is_present: boolean
}

export interface SearchResult {
  project: Project
  relevance: number
} 