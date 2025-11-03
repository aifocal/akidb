#!/usr/bin/env bash
set -euo pipefail

# sync-github.sh
# Sync local repository with GitHub (https://github.com/aifocal/akidb)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${PROJECT_ROOT}"

echo "🔄 Syncing AkiDB with GitHub..."
echo ""

# Color codes
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Check if we're on main branch
CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [ "$CURRENT_BRANCH" != "main" ]; then
    echo -e "${YELLOW}⚠️  Warning: You're on branch '$CURRENT_BRANCH', not 'main'${NC}"
    read -p "Continue anyway? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Aborted."
        exit 1
    fi
fi

# Check for uncommitted changes
if [[ -n $(git status --porcelain) ]]; then
    echo -e "${YELLOW}📝 Uncommitted changes detected:${NC}"
    git status --short
    echo ""
    read -p "Commit all changes? (Y/n) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Nn]$ ]]; then
        # Get commit message
        echo "Enter commit message (or press Enter for auto-generated message):"
        read -r COMMIT_MSG

        if [ -z "$COMMIT_MSG" ]; then
            COMMIT_MSG="chore: Auto-sync local changes

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>"
        else
            COMMIT_MSG="${COMMIT_MSG}

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>"
        fi

        git add -A
        git commit -m "$COMMIT_MSG"
        echo -e "${GREEN}✓ Changes committed${NC}"
    else
        echo -e "${YELLOW}⚠️  Skipping uncommitted changes${NC}"
    fi
fi

# Fetch latest from remote
echo ""
echo "📥 Fetching from origin..."
git fetch origin "$CURRENT_BRANCH"

# Check if we're behind
LOCAL=$(git rev-parse @)
REMOTE=$(git rev-parse @{u} 2>/dev/null || echo "")
BASE=$(git merge-base @ @{u} 2>/dev/null || echo "")

if [ -z "$REMOTE" ]; then
    echo -e "${RED}✗ No upstream branch set${NC}"
    exit 1
fi

if [ "$LOCAL" = "$REMOTE" ]; then
    echo -e "${GREEN}✓ Already up to date${NC}"
elif [ "$LOCAL" = "$BASE" ]; then
    echo -e "${YELLOW}📥 Pulling changes from remote...${NC}"
    git pull origin "$CURRENT_BRANCH"
    echo -e "${GREEN}✓ Pulled successfully${NC}"
elif [ "$REMOTE" = "$BASE" ]; then
    echo -e "${YELLOW}📤 Pushing changes to remote...${NC}"
    git push origin "$CURRENT_BRANCH"
    echo -e "${GREEN}✓ Pushed successfully${NC}"
else
    echo -e "${RED}✗ Branches have diverged!${NC}"
    echo "Please resolve manually with:"
    echo "  git pull --rebase origin $CURRENT_BRANCH"
    exit 1
fi

# Final status
echo ""
echo -e "${GREEN}✅ Sync complete!${NC}"
echo ""
git log --oneline -3
echo ""
echo "Remote: $(git config --get remote.origin.url)"
echo "Branch: $CURRENT_BRANCH"
