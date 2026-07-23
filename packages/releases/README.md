# Immutable Model Package Releases

Each leaf directory is one complete author-source authority identified by the
canonical package name and exact version in its `package.json`. A new release
gets a new directory. Existing release bytes are never edited to add a
declaration, revise documentation, or update a dependency.

The directory hierarchy is for source stewardship only. Resolution remains
content-addressed and accepts only exact name, version, semantic digest, source
digest, and manifest dependency edges. No runtime search path or compatibility
selection is inferred from this tree.
