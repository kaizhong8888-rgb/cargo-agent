# Contributing to cargo-agent

## Test Coverage Requirements

### Policy
All **new tools** (files in `src/tools/builtin/`) must achieve **≥80% line coverage** before merging.

### Why 80%?
- Covers all happy paths and error handling branches
- Reasonable target that doesn't over-penalize edge cases
- Aligns with industry best practices for systems software
- Ensures new code is properly tested from day one

### Coverage Thresholds

| Metric | Requirement | Scope |
|--------|-------------|-------|
| **Line coverage** | **≥80%** | All new tool modules |
| Branch coverage | ≥70% | Recommended |
| Function coverage | ≥85% | Recommended |
| Pure functions | 100% | Deterministic, easy to test |

### What's Covered?

| Scope | Requirement |
|-------|-------------|
| New tool modules (`src/tools/builtin/*.rs`) | **≥80% line coverage** (mandatory) |
| Existing tool modules | Improving coverage is encouraged |
| Pure utility functions | 100% coverage (deterministic, easy to test) |
| Test code itself | Not measured (standard practice) |
| Generated code | Exempt from coverage requirements |

### How to Check Coverage

```bash
# Install cargo-llvm-cov (recommended)
cargo install cargo-llvm-cov

# Generate coverage report (HTML)
cargo llvm-cov --lib --html --open

# Generate coverage report (LCOV for CI)
cargo llvm-cov --lib --lcov --output-path lcov.info

# Check coverage for a specific tool file
cargo llvm-cov --lib --json > coverage.json
# Then parse coverage.json to check per-file coverage

# Quick one-liner to check if all tool files meet 80%
cargo llvm-cov --lib --json | python3 -c "
import json, sys
data = json.load(sys.stdin)
for d in data.get('data', []):
    for f in d.get('files', []):
        fn = f.get('filename', '')
        if 'tools/builtin' in fn:
            pct = f.get('summary', {}).get('lines', {}).get('percent', 0)
            status = '✅' if pct >= 80 else '❌'
            print(f'{status} {fn}: {pct:.1f}%')
"
```

### Minimum Test Requirements for New Tools

Every new tool must have tests covering:

1. **Happy path** — normal successful execution for each action
2. **Error handling** — missing params, invalid input
3. **Unknown action** — invalid action parameter returns proper error
4. **Action-specific tests** — at least one test per action variant
5. **Edge cases** — empty input, boundary values, special characters

### Example: ci_cd_tool.rs Test Checklist

- [x] `test_generate_ci_github` — GitHub CI config generation
- [x] `test_generate_ci_gitlab` — GitLab CI config generation  
- [x] `test_generate_ci_unknown_platform` — error handling for unknown platform
- [x] `test_missing_action` — missing action parameter
- [x] `test_unknown_action` — invalid action parameter
- [x] `test_coverage_info` — coverage instructions generation
- [x] `test_audit_no_audit_installed` — audit when cargo-audit not installed
- [x] `test_run_tests_in_current_project` — test execution
- [x] `test_run_build_in_current_project` — build execution
- [x] `test_run_build_release_profile` — release profile build
- [x] `test_run_tests_with_pattern` — test pattern filtering
- [x] `test_run_build_bench_profile` — bench profile build
- [x] `test_check_coverage_threshold_no_llvm_cov` — coverage check without llvm-cov
- [x] `test_check_coverage_threshold_custom_threshold` — custom threshold value
- [x] `test_check_coverage_threshold_specific_file` — check specific tool file

**Total: 15 tests covering all actions and error paths**

### Pre-merge Checklist

Before submitting a PR for a new tool:

- [ ] `cargo clippy --all-targets -- -D warnings` passes (0 warnings)
- [ ] `cargo test --lib` passes (all tests green)
- [ ] `cargo llvm-cov --lib --html --open` shows **≥80% line coverage** for the new file
- [ ] No `.unwrap()` calls in business code (tests are OK)
- [ ] No `dbg!`, `todo!()`, or `unimplemented!()` in business code
- [ ] Documentation comments for all public functions
- [ ] Tool registered in `mod.rs` and `gateway/mod.rs`
- [ ] Tool added to the parameter list and match statement

### CI Enforcement

The CI pipeline (`.github/workflows/ci.yml`) enforces:

1. **Compilation**: `cargo check --all-targets --all-features` must pass
2. **Tests**: `cargo test --all-targets --all-features` must pass
3. **Clippy**: `cargo clippy --all-targets --all-features -- -D warnings` (0 warnings)
4. **Format**: `cargo fmt --all -- --check` must pass
5. **Security**: `cargo audit` checks for known vulnerabilities
6. **Coverage**: New tool files must have ≥80% line coverage

### Enforcement Commands

```bash
# Quick health check
cargo check && cargo test && cargo clippy -- -D warnings && cargo fmt --check

# Coverage check for all tool files
cargo llvm-cov --lib --json | python3 -c "
import json, sys
data = json.load(sys.stdin)
fail = False
for d in data.get('data', []):
    for f in d.get('files', []):
        fn = f.get('filename', '')
        if 'tools/builtin' in fn:
            pct = f.get('summary', {}).get('lines', {}).get('percent', 0)
            status = '✅' if pct >= 80 else '❌'
            print(f'{status} {fn}: {pct:.1f}%')
            if pct < 80:
                fail = True
if fail:
    print('\\n❌ Some tool files are below 80% coverage threshold!')
    sys.exit(1)
print('\\n✅ All tool files meet 80% coverage threshold')
"
```
