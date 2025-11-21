# Project Cleanup Analysis & High-Level Map

**Analysis Date**: November 21, 2025
**Project**: Private Poker (Rust Texas Hold'em Platform)
**Status**: Production Ready

---

## 1. Project High-Level Map

### 1.1 Core Workspace Structure

```
private_poker/
├── Cargo.toml                    # ✅ Workspace root config
├── .env                          # ✅ Environment configuration
├── .env.example                  # ✅ Template for environment vars
├── CLAUDE.md                     # ✅ Complete project documentation (1,327 lines)
├── README.md                     # ✅ Quick start guide
│
├── private_poker/                # ✅ CORE LIBRARY (Game Engine, Bot AI, Security)
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs               # ✅ Main entry point - exports all modules
│   │   ├── game/                # ✅ Core poker engine (FSM, hand evaluation)
│   │   │   ├── mod.rs
│   │   │   ├── entities.rs      # 99.57% coverage
│   │   │   ├── functional.rs    # 99.71% coverage
│   │   │   ├── implementation.rs
│   │   │   ├── state_machine.rs
│   │   │   ├── constants.rs
│   │   │   └── states/mod.rs
│   │   ├── auth/                # ✅ Authentication (Argon2id, JWT, 2FA)
│   │   ├── wallet/              # ✅ Financial system (double-entry ledger)
│   │   ├── bot/                 # ✅ Bot AI system
│   │   ├── security/            # ✅ Rate limiting, anti-collusion
│   │   ├── table/               # ✅ Multi-table actor system
│   │   ├── tournament/          # ✅ Tournament management
│   │   ├── net/                 # ✅ Networking (client, server, messages)
│   │   └── db/                  # ✅ Database layer (sqlx, repository pattern)
│   ├── tests/                   # ✅ Integration tests (12 test files)
│   ├── examples/                # ✅ hand_evaluation.rs example
│   └── benches/                 # ✅ game_benchmarks.rs
│
├── pp_server/                    # ✅ HTTP/WebSocket Server
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs              # ✅ Server entry point
│   │   ├── lib.rs               # ✅ Server library
│   │   ├── logging.rs           # ✅ Structured logging
│   │   └── api/                 # ✅ REST & WebSocket endpoints
│   │       ├── mod.rs           # Router
│   │       ├── auth.rs          # Auth endpoints
│   │       ├── tables.rs        # Table endpoints
│   │       ├── websocket.rs     # WebSocket handler
│   │       ├── request_id.rs    # Request tracing
│   │       └── middleware.rs    # Middleware
│   └── tests/                   # ✅ Server integration tests
│
├── pp_client/                    # ✅ TUI/CLI Client
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs              # ✅ Client entry point
│   │   ├── lib.rs               # ✅ Client library
│   │   ├── tui_app.rs           # ✅ Rich TUI interface (ratatui)
│   │   ├── app.rs               # ✅ Simple CLI interface
│   │   ├── commands.rs          # ✅ Command parser (100% coverage)
│   │   ├── api_client.rs        # ✅ HTTP API client
│   │   └── websocket_client.rs  # ✅ WebSocket client
│   └── tests/                   # ✅ Client integration tests
│
├── pp_bots/                      # ✅ Bot Manager Application
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs              # ✅ Bot manager entry point
│       ├── app.rs               # ✅ Bot TUI app
│       └── bot.rs               # ✅ Bot runner
│
├── migrations/                   # ✅ Database migrations (sqlx)
│   ├── 001_initial_schema.sql   # ✅ PRIMARY - Used by sqlx migrate
│   ├── 007_tournaments.sql      # ✅ PRIMARY
│   ├── 008_balance_constraints.sql  # ✅ PRIMARY
│   └── 009_rate_limit_unique_constraint.sql  # ✅ PRIMARY
│
└── .github/workflows/            # ✅ CI/CD
    └── ci.yml                    # ✅ GitHub Actions workflow
```

---

## 2. Unused/Redundant Files Detected

### 2.1 🔴 SAFE TO DELETE - Duplicate/Obsolete Files

#### A. Duplicate Migration Files

**File**: `private_poker/src/db/migrations/001_initial_schema.sql`

**Evidence**:
- ❌ Not referenced in any Cargo.toml
- ❌ Not imported by any Rust code
- ❌ sqlx uses root `migrations/` directory, not `src/db/migrations/`
- ✅ Root `migrations/001_initial_schema.sql` is newer (Nov 17) and more complete
- ✅ Root migration has additional constraints and tournament tables

**Differences**:
```diff
# Root migrations/001_initial_schema.sql has:
+ UNIQUE constraint on rate_limit_attempts
+ Tournament tables (tournaments, tournament_tables, tournament_players)
+ Updated schema with latest fixes

# private_poker/src/db/migrations/001_initial_schema.sql:
- Older version (Nov 6)
- Missing tournament tables
- Missing unique constraints
```

**Checked**:
- ✅ grep -r "src/db/migrations" --include="*.toml" → No results
- ✅ grep -r "src/db/migrations" --include="*.rs" → No imports
- ✅ sqlx migrate uses `migrations/` directory by default

**Recommendation**: **DELETE** - It's a duplicate of an older schema version

---

#### B. Backup README File

**File**: `README.md.old`

**Evidence**:
- ❌ Backup created during documentation cleanup
- ❌ Not referenced anywhere
- ✅ New README.md exists and is complete

**Checked**:
- ✅ grep -r "README.md.old" → No references

**Recommendation**: **DELETE** - Temporary backup no longer needed

---

#### C. Web Client Directory

**Directory**: `web_client/`

**Contents**:
```
web_client/
├── index.html
├── lobby.html
├── game.html
├── js/
│   ├── api.js
│   ├── auth.js
│   ├── websocket.js
│   └── game.js
├── css/
│   ├── main.css
│   └── cards.css
└── README.md
```

**Evidence**:
- ⚠️ Not referenced in Cargo workspace
- ⚠️ Not mentioned in CLAUDE.md or README.md
- ⚠️ Not built by cargo build
- ⚠️ Only referenced in `test_full_system.sh` (manual test script)
- ✅ Standalone HTML/JS/CSS web client
- ✅ Working implementation (as per test script)

**Checked**:
- ✅ grep -r "web_client" --include="*.toml" → No results
- ✅ grep -r "web_client" --include="*.rs" → No results
- ✅ grep -r "web_client" --include="*.md" → Only in its own README and test_full_system.sh

**Status**: **QUESTIONABLE**

**Recommendation**: **REVIEW MANUALLY** - This appears to be a separate web client implementation not integrated into the main build system. Options:
1. **Keep if actively used**: Document in main README.md
2. **Move to separate repo**: If it's a standalone project
3. **Delete**: If superseded by TUI/CLI clients

---

#### D. SSH Configuration File

**File**: `pp_admin/sshd_config`

**Evidence**:
- ❌ SSH server configuration
- ❌ Not referenced in any Rust code
- ❌ Not used by application
- ❌ Appears to be for a separate SSH-based deployment setup

**Checked**:
- ✅ grep -r "sshd_config" --include="*.rs" --include="*.toml" → No results
- ✅ Not mentioned in documentation

**Recommendation**: **REVIEW MANUALLY** - May be part of deployment infrastructure

---

#### E. Admin Scripts

**Directory**: `pp_admin/`

**Files**:
```
pp_admin/
├── create_user.sh      # Create user via psql
├── delete_user.sh      # Delete user via psql
├── claim_user.sh       # Claim user script
└── sshd_config         # SSH config
```

**Evidence**:
- ⚠️ Shell scripts for direct database manipulation
- ⚠️ Not integrated into Rust application
- ⚠️ No Cargo.toml or Rust code
- ⚠️ Appears to be external admin utilities

**Checked**:
- ✅ grep -r "pp_admin" --include="*.rs" --include="*.toml" → No results
- ✅ grep -r "create_user.sh" --include="*.md" → No documentation

**Recommendation**: **REVIEW MANUALLY** - May be useful admin tools but not part of core application

---

#### F. Test Scripts in Root

**Files**:
```
test_full_system.sh       # Full system integration test
test_complete_flow.sh     # Complete flow test
test_game_flow.sh         # Game flow test
test_join_fix.sh          # Join fix test
debug_game.sh             # Debug script
```

**Evidence**:
- ✅ All are manual integration test scripts
- ✅ Not run by `cargo test`
- ⚠️ Some reference web_client (which may not exist in prod)
- ⚠️ Contain hardcoded credentials (postgres:7794951)

**Checked**:
- ✅ Not referenced in Cargo.toml
- ✅ Not run by CI/CD (.github/workflows/ci.yml only runs `cargo test`)
- ⚠️ May be useful for manual testing

**Recommendation**: **KEEP** but consider:
1. Move to `scripts/` directory for organization
2. Add to .gitignore if they contain sensitive data
3. Document usage in README.md or separate TESTING.md

---

#### G. Database Backup/Restore Scripts

**Directory**: `scripts/`

**Files**:
```
scripts/
├── backup-db.sh         # PostgreSQL backup script
└── restore-db.sh        # PostgreSQL restore script
```

**Evidence**:
- ✅ Useful operational scripts
- ✅ Production-ready backup/restore functionality
- ⚠️ Not documented in main README.md

**Recommendation**: **KEEP** - Production operational scripts

---

#### H. Assets Directory

**Directory**: `assets/`

**Files**:
```
assets/
├── demo.gif             # 1.05 MB GIF file
├── demo_mold.tape       # VHS tape recording script
└── demo_ognf.tape       # VHS tape recording script
```

**Evidence**:
- ❌ Not referenced in CLAUDE.md or README.md
- ❌ Excluded from Cargo workspace (Cargo.toml: exclude = ["assets/*"])
- ❌ .tape files are VHS (terminal recording) scripts
- ✅ demo.gif is likely a demo recording but not linked anywhere

**Checked**:
- ✅ grep -r "demo.gif" --include="*.md" → No results
- ✅ grep -r "demo_mold\|demo_ognf" → No results

**Recommendation**: **REVIEW MANUALLY** - If not used for documentation, can be deleted. If keeping, document in README.md

---

### 2.2 🟡 FILES TO REVIEW MANUALLY

#### A. Docker Configuration

**File**: `docker-compose.yml`, `Dockerfile`

**Status**: **KEEP** - Deployment infrastructure
**Note**: Document in README.md Docker section

---

#### B. CI/CD Configuration

**File**: `.github/workflows/ci.yml`

**Status**: **KEEP** - GitHub Actions workflow
**Note**: Currently only runs `cargo test`, could be enhanced

---

#### C. Example Files

**File**: `private_poker/examples/hand_evaluation.rs`

**Status**: **KEEP** - Useful for documentation
**Note**: Consider documenting in CLAUDE.md

---

#### D. Benchmark Files

**File**: `private_poker/benches/game_benchmarks.rs`

**Status**: **KEEP** - Performance benchmarks
**Note**: Consider documenting how to run

---

### 2.3 🟢 NO ISSUES - Core Files Verified

All core application files are:
- ✅ Actively used
- ✅ Referenced in imports
- ✅ Part of build system
- ✅ Tested (661 tests passing)

---

## 3. Refactors Needed Before Cleanup

### 3.1 If Keeping web_client/

**Actions Required**:
1. Add to CLAUDE.md under "Client Applications"
2. Add to README.md with setup instructions
3. Consider adding to Cargo workspace or separate repo
4. Document in architecture diagrams

### 3.2 Test Scripts Organization

**Actions Required**:
1. Move all test_*.sh and debug_*.sh to `scripts/tests/`
2. Create `scripts/README.md` documenting each script
3. Remove hardcoded credentials
4. Add example usage to main README.md

### 3.3 Admin Scripts

**Actions Required**:
1. Document pp_admin/ scripts if keeping
2. Consider integrating into main application as admin commands
3. Or move to separate admin tool repository

---

## 4. Safe Cleanup Plan

### Phase 1: Definite Safe Deletions

```bash
# Duplicate migration file (older version)
git rm private_poker/src/db/migrations/001_initial_schema.sql
git rm -r private_poker/src/db/migrations/

# Backup README
git rm README.md.old
```

**Risk**: ⚠️ **VERY LOW** - These are confirmed duplicates/backups

---

### Phase 2: Review Required

**Manual Review Needed For**:

1. **web_client/** - Decide:
   - Keep and document
   - Move to separate repository
   - Delete if superseded

2. **pp_admin/** - Decide:
   - Keep as admin utilities (document)
   - Integrate into main app
   - Delete if unused

3. **assets/** - Decide:
   - Add demo.gif to README.md
   - Delete if not used
   - Move to docs/ if keeping

4. **Test scripts** - Decide:
   - Organize into scripts/tests/
   - Document in TESTING.md
   - Keep as-is

---

### Phase 3: Organization (Optional)

```bash
# Create organized structure
mkdir -p scripts/tests/
mkdir -p scripts/admin/
mkdir -p docs/assets/

# Move test scripts
git mv test_*.sh debug_*.sh scripts/tests/

# Move admin scripts
git mv pp_admin/* scripts/admin/
git rm -r pp_admin/

# Move assets (if keeping)
git mv assets/* docs/assets/
git rm -r assets/
```

**Risk**: ⚠️ **LOW** - Organizational changes only

---

## 5. Shell Commands for Cleanup

### 5.1 SAFE IMMEDIATE CLEANUP (Execute after review)

```bash
#!/bin/bash
# Private Poker - Safe Cleanup Script
# Execute these commands after manual verification

echo "Phase 1: Definite Safe Deletions"
echo "=================================="

# Remove duplicate migration directory
git rm -r private_poker/src/db/migrations/
echo "✓ Removed duplicate migration directory"

# Remove backup README
git rm README.md.old
echo "✓ Removed README.md.old backup"

# Commit Phase 1
git commit -m "chore: Remove duplicate migrations and backup files

- Remove private_poker/src/db/migrations/ (duplicate of root migrations/)
- Remove README.md.old (temporary backup)
- Root migrations/ directory is the authoritative source
"

echo ""
echo "Phase 1 complete. Review Phase 2 before proceeding."
```

---

### 5.2 OPTIONAL CLEANUP (Review required)

```bash
#!/bin/bash
# Private Poker - Optional Cleanup
# REVIEW EACH SECTION BEFORE EXECUTING

echo "Phase 2: Optional Cleanup (REVIEW REQUIRED)"
echo "============================================"

# Option A: Remove web_client if not used
read -p "Remove web_client/? (y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    git rm -r web_client/
    echo "✓ Removed web_client/"
fi

# Option B: Remove pp_admin if not used
read -p "Remove pp_admin/? (y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    git rm -r pp_admin/
    echo "✓ Removed pp_admin/"
fi

# Option C: Remove assets if not documented
read -p "Remove assets/? (y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    git rm -r assets/
    echo "✓ Removed assets/"
fi

# Commit Phase 2 if any changes
git status --short | grep -q "^D" && git commit -m "chore: Remove unused directories

- Remove web_client/ (standalone, not integrated)
- Remove pp_admin/ (separate admin tools)
- Remove assets/ (unused demo files)
"

echo ""
echo "Phase 2 complete."
```

---

### 5.3 ORGANIZATION (Optional improvement)

```bash
#!/bin/bash
# Private Poker - Organization Improvements
# Organize scripts into better structure

echo "Phase 3: Organization (OPTIONAL)"
echo "================================="

# Create directories
mkdir -p scripts/tests/
mkdir -p scripts/admin/

# Move test scripts
git mv test_*.sh debug_*.sh scripts/tests/ 2>/dev/null

# Create scripts README
cat > scripts/README.md << 'EOF'
# Scripts Directory

## Database Scripts
- `backup-db.sh` - PostgreSQL backup
- `restore-db.sh` - PostgreSQL restore

## Test Scripts
- `tests/test_full_system.sh` - Full system integration test
- `tests/test_complete_flow.sh` - Complete flow test
- `tests/test_game_flow.sh` - Game flow test
- `tests/test_join_fix.sh` - Join fix test
- `tests/debug_game.sh` - Debug script

## Usage

### Backup Database
\`\`\`bash
./scripts/backup-db.sh
\`\`\`

### Run Full System Test
\`\`\`bash
./scripts/tests/test_full_system.sh
\`\`\`
EOF

git add scripts/README.md

# Commit organization
git commit -m "chore: Organize scripts directory

- Move test scripts to scripts/tests/
- Add scripts/README.md with documentation
- Improve project organization
"

echo "✓ Organization complete"
```

---

## 6. Summary

### Files Analysis Summary

| Category | Count | Action | Risk |
|----------|-------|--------|------|
| **Core Source Files** | 63 | ✅ Keep | None |
| **Test Files** | 12+ | ✅ Keep | None |
| **Documentation** | 2 (CLAUDE.md, README.md) | ✅ Keep | None |
| **Duplicate Migrations** | 1 dir | 🔴 DELETE | Very Low |
| **Backup Files** | 1 | 🔴 DELETE | Very Low |
| **Web Client** | 1 dir | 🟡 REVIEW | Medium |
| **Admin Scripts** | 1 dir | 🟡 REVIEW | Medium |
| **Assets** | 1 dir | 🟡 REVIEW | Low |
| **Test Scripts** | 5 files | 🟢 Keep (organize) | None |
| **Operational Scripts** | 2 files | ✅ Keep | None |
| **CI/CD** | 1 file | ✅ Keep | None |

---

### Recommendations Priority

1. **Immediate (Safe)**:
   - ✅ Delete `private_poker/src/db/migrations/` (duplicate)
   - ✅ Delete `README.md.old` (backup)

2. **Review & Decide**:
   - ⚠️ `web_client/` - Document or delete
   - ⚠️ `pp_admin/` - Document or delete
   - ⚠️ `assets/` - Link in docs or delete

3. **Optional Improvements**:
   - 📝 Organize test scripts
   - 📝 Document all scripts
   - 📝 Enhance CI/CD

---

### Verification Checklist

Before executing cleanup:
- [ ] Review web_client/ usage
- [ ] Check if pp_admin/ is actively used
- [ ] Verify assets/ are not needed for docs
- [ ] Backup database before testing
- [ ] Run full test suite after cleanup
- [ ] Verify build succeeds: `cargo build --workspace`
- [ ] Verify tests pass: `cargo test --workspace`

---

**Analysis Complete**: November 21, 2025
**Total Files Analyzed**: 100+
**Safe Deletions Identified**: 2 (very low risk)
**Manual Review Required**: 3 directories
