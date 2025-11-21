#!/bin/bash
# Private Poker - Safe Cleanup Script
# Removes confirmed duplicate/obsolete files
# SAFE TO EXECUTE - Very low risk

set -e

echo "================================================"
echo "Private Poker - Safe Cleanup"
echo "================================================"
echo ""
echo "This script will remove:"
echo "  1. private_poker/src/db/migrations/ (duplicate)"
echo "  2. README.md.old (backup)"
echo ""
read -p "Continue? (y/N): " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Cancelled."
    exit 0
fi

echo ""
echo "Starting cleanup..."
echo ""

# Remove duplicate migration directory
if [ -d "private_poker/src/db/migrations" ]; then
    echo "Removing private_poker/src/db/migrations/..."
    git rm -r private_poker/src/db/migrations/
    echo "✓ Removed duplicate migration directory"
else
    echo "⊘ private_poker/src/db/migrations/ not found"
fi

# Remove backup README
if [ -f "README.md.old" ]; then
    echo "Removing README.md.old..."
    git rm README.md.old
    echo "✓ Removed README.md.old backup"
else
    echo "⊘ README.md.old not found"
fi

echo ""
echo "Staging changes for commit..."
git status --short

echo ""
read -p "Commit these changes? (y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    git commit -m "chore: Remove duplicate migrations and backup files

- Remove private_poker/src/db/migrations/ (duplicate of root migrations/)
- Remove README.md.old (temporary backup)
- Root migrations/ directory is the authoritative source
"
    echo ""
    echo "✓ Changes committed"
else
    echo ""
    echo "Changes staged but not committed."
    echo "To commit later: git commit -m 'chore: Remove duplicates'"
    echo "To undo: git reset HEAD"
fi

echo ""
echo "================================================"
echo "Safe cleanup complete!"
echo "================================================"
echo ""
echo "Next steps:"
echo "  1. Review PROJECT_CLEANUP_ANALYSIS.md"
echo "  2. Run ./cleanup_review.sh for optional cleanup"
echo "  3. Verify build: cargo build --workspace"
echo "  4. Verify tests: cargo test --workspace"
