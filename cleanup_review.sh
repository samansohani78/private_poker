#!/bin/bash
# Private Poker - Optional Cleanup (Review Required)
# Removes directories that may or may not be needed
# MANUAL REVIEW REQUIRED

set -e

echo "================================================"
echo "Private Poker - Optional Cleanup"
echo "================================================"
echo ""
echo "⚠️  WARNING: Manual review required!"
echo ""
echo "This script will prompt you to remove:"
echo "  1. web_client/ - Standalone web client (not in build)"
echo "  2. pp_admin/ - Admin shell scripts"
echo "  3. assets/ - Demo GIFs and recording scripts"
echo ""
echo "Review PROJECT_CLEANUP_ANALYSIS.md before proceeding."
echo ""
read -p "Continue? (y/N): " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Cancelled."
    exit 0
fi

echo ""
CHANGES_MADE=false

# Option A: Remove web_client
if [ -d "web_client" ]; then
    echo ""
    echo "web_client/ contains:"
    echo "  - Standalone HTML/JS/CSS web interface"
    echo "  - Not integrated into Cargo build"
    echo "  - Referenced in test_full_system.sh only"
    echo ""
    read -p "Remove web_client/? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        git rm -r web_client/
        echo "✓ Removed web_client/"
        CHANGES_MADE=true
    else
        echo "⊘ Kept web_client/"
    fi
fi

# Option B: Remove pp_admin
if [ -d "pp_admin" ]; then
    echo ""
    echo "pp_admin/ contains:"
    echo "  - Shell scripts for user management"
    echo "  - Direct database manipulation scripts"
    echo "  - SSH configuration"
    echo ""
    read -p "Remove pp_admin/? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        git rm -r pp_admin/
        echo "✓ Removed pp_admin/"
        CHANGES_MADE=true
    else
        echo "⊘ Kept pp_admin/"
    fi
fi

# Option C: Remove assets
if [ -d "assets" ]; then
    echo ""
    echo "assets/ contains:"
    echo "  - demo.gif (1 MB)"
    echo "  - VHS tape recording scripts"
    echo "  - Not linked in documentation"
    echo ""
    read -p "Remove assets/? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        git rm -r assets/
        echo "✓ Removed assets/"
        CHANGES_MADE=true
    else
        echo "⊘ Kept assets/"
    fi
fi

# Commit if changes were made
if [ "$CHANGES_MADE" = true ]; then
    echo ""
    echo "Staging changes for commit..."
    git status --short

    echo ""
    read -p "Commit these changes? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        git commit -m "chore: Remove unused directories

$(git status --short | grep "^D" | sed 's/^D /- /')

Removed directories not integrated into main build system.
See PROJECT_CLEANUP_ANALYSIS.md for details.
"
        echo ""
        echo "✓ Changes committed"
    else
        echo ""
        echo "Changes staged but not committed."
        echo "To commit later: git commit"
        echo "To undo: git reset HEAD"
    fi
else
    echo ""
    echo "No changes made."
fi

echo ""
echo "================================================"
echo "Optional cleanup complete!"
echo "================================================"
echo ""
echo "Next steps:"
echo "  1. Verify build: cargo build --workspace"
echo "  2. Verify tests: cargo test --workspace"
echo "  3. Review remaining structure"
