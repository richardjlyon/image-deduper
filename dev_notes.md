Architecture:

- File discovery
- Image processing
- Deduplication engine
- Action manager

File discovery:

Filesystem → Image File Catalog → Processing Queue

- Recursive directory traversal with configurable depth
- Multi-threaded file scanning for performance
- Support for all required formats (JPEG, PNG, TIFF, HEIC)
- Metadata extraction (creation date, dimensions, etc.)
- Exclusion patterns for directories/files to skip

Image processing:

  Raw Image → Hash Generator → Comparison Engine

- Primary hash method: Perceptual hashing (pHash) for near-duplicate detection
- Secondary verification: Pixel-by-pixel comparison for confirmed duplicates
- HEIC format support via third-party library (libheif bindings)
- Thumbnail generation for visual verification
- Parallel processing with work stealing for efficiency

Deduplication Engine

  Hash Database → Duplicate Groups → Decision Engine

- Persistence layer to maintain state between runs
- Grouping of similar/identical images
- Configurable prioritization rules:
  - Highest resolution
  - Original creation date
  - Best q11uality (based on compression artifacts)
  - Directory preferences (originals vs. backups)


Action Manager

  Decision → Safety Check → Execution → Verification

- Safety-first approach: Default to moving files to a "duplicates" directory rather than deletion
- Dry-run mode to preview actions without making changes
- Transaction-like operations with rollback capability
- Logging of all actions for audit trail
- Post-action verification to ensure originals are intact

Implementation Roadmap
----------------------
Start with a minimal version that only handles exact duplicates
Add HEIC support and test thoroughly
Implement the perceptual hashing and similarity detection
Build the safety features and verification systems
Add the configurable prioritization rules
Improve performance with parallel processing
