## What's Changed

- Changed the Files panel to a tree view with expandable folders and indentation.
- Improved multi-selection actions in the left pane. Stage, Unstage, Revert, and Delete now apply to all selected items, and the deletion confirmation dialog lists the affected items.
- Added filtering and Select All to Changes, along with file and folder icons and ACTION / STATE columns that distinguish each change from its staging state.
- Updated the Changes context menu to show only actions available for the current state.

## Installation

Download `LoreLens-0.1.5.msi` for Windows x64. Install the Lore CLI separately using the in-app installation guide, or select an existing installation with Locate CLI.

## Validation and Known Limitations

- All 40 tests passed; one Lore CLI integration test was excluded.
- The Windows release build and MSI package were generated. Manual UI testing and MSI installation and upgrade testing were not performed.
- This release does not address missing content or `Address not found` errors from the Lore CLI.
