export const meta = {
  name: 'project-code-review',
  description: 'Multi-dimensional code review for cargo-agent project',
  whenToUse: 'When the user wants a comprehensive code review of the project',
  phases: [
    { title: 'Explore', detail: 'Discover project structure and key files' },
    { title: 'Security', detail: 'Security vulnerability scan' },
    { title: 'Quality', detail: 'Code quality and patterns review' },
    { title: 'Performance', detail: 'Performance bottleneck analysis' },
    { title: 'Tests', detail: 'Test coverage and quality analysis' },
    { title: 'Synthesize', detail: 'Combine findings into final report' },
  ],
}

phase('Explore')
const exploreResult = await agent(
  'Explore the cargo-agent Rust project at /Users/kai/projects/cargo-agent. Read the CLAUDE.md, Cargo.toml, src/lib.rs, src/main.rs, and key module files (src/agent/core.rs, src/gateway/mod.rs, src/tools/mod.rs, src/tools/registry.rs, src/ui/mod.rs, src/trading/mod.rs, src/skills/mod.rs). Map the architecture: identify all public APIs, tool implementations, agent lifecycle, error handling patterns, and concurrency patterns. Return a structured summary of: 1) Module hierarchy 2) Key public types/traits 3) Concurrency model 4) Error handling approach 5) Recent changes from git status (modified/deleted files). Focus especially on src/tools/builtin/ which has many new files (async_profiler, container_tool, cross_compile, db_migration, fuzz_driver, license_audit) and the quantitative-trading/src/ module.',
  { label: 'explore-architecture', phase: 'Explore' },
)

const qualityResult = await agent(
  'Review the cargo-agent project code for quality issues. Read src/agent/core.rs, src/gateway/mod.rs, src/ui/mod.rs, src/main.rs, src/tools/builtin/ (file_tools, fs_tools, git_tools, code_executor, code_analyzer, code_transform, code_review, dep_manager, scaffold, memory_tool, task_planner, task_pool, llm_tool, database_tool, crypto_tool, config_store, scheduler, doc_search, diagram, evolution_tools, hello_tool, async_profiler, container_tool, cross_compile, db_migration, fuzz_driver, license_audit), and src/trading/ (strategy, backtest, optimizer, report, indicators, data). Check for: 1) unwrap() usage in non-test code 2) Deep nesting (>4 levels) 3) Functions >50 lines 4) Missing error handling (silent failures) 5) Magic numbers 6) Code duplication 7) Naming inconsistencies 8) Module organization issues. Return a list of findings with file:line, severity (CRITICAL/HIGH/MEDIUM/LOW), and description.',
  { label: 'quality-review', phase: 'Quality', schema: {
    type: 'object',
    properties: {
      findings: {
        type: 'array',
        items: {
          type: 'object',
          properties: {
            file: { type: 'string' },
            line: { type: 'number' },
            severity: { type: 'string', enum: ['CRITICAL', 'HIGH', 'MEDIUM', 'LOW'] },
            category: { type: 'string' },
            description: { type: 'string' },
          },
          required: ['file', 'description', 'severity'],
        },
      },
    },
    required: ['findings'],
  }},
)

const securityResult = await agent(
  'Security audit of cargo-agent Rust project at /Users/kai/projects/cargo-agent. Review all source files for: 1) Hardcoded secrets/credentials/API keys 2) Unsafe code blocks without SAFETY comments 3) Path traversal vulnerabilities (file_tools, fs_tools) 4) Command injection (code_executor, shell execution) 5) SQL injection (database_tool, memory_tool, sqlite_store) 6) Input validation at boundaries 7) Error messages leaking sensitive data 8) Missing rate limiting or resource exhaustion protection 9) Unsafe deserialization 10) TOCTOU race conditions. Read src/tools/builtin/file_tools.rs, fs_tools.rs, net_tools.rs, code_executor.rs, database_tool.rs, memory/sqlite_store.rs, and any file handling user input. Return findings with file:line, severity, and description.',
  { label: 'security-audit', phase: 'Security', schema: {
    type: 'object',
    properties: {
      findings: {
        type: 'array',
        items: {
          type: 'object',
          properties: {
            file: { type: 'string' },
            line: { type: 'number' },
            severity: { type: 'string', enum: ['CRITICAL', 'HIGH', 'MEDIUM', 'LOW'] },
            category: { type: 'string' },
            description: { type: 'string' },
            remediation: { type: 'string' },
          },
          required: ['file', 'description', 'severity'],
        },
      },
    },
    required: ['findings'],
  }},
)

const performanceResult = await agent(
  'Performance review of cargo-agent Rust project. Read src/agent/core.rs (tool execution, message handling, context truncation), src/trading/backtest.rs, src/trading/optimizer.rs, src/trading/strategy.rs, src/trading/report.rs, and quantitative-trading/src/backtest.rs. Look for: 1) Unnecessary clones 2) N+1 query patterns 3) Unbounded data structures 4) Inefficient string operations 5) Missing caching 6) Synchronous blocking in async contexts 7) Memory allocation hotspots 8) Inefficient iterator usage 9) Redundant computation. Return findings with file:line, severity, and description.',
  { label: 'performance-review', phase: 'Performance', schema: {
    type: 'object',
    properties: {
      findings: {
        type: 'array',
        items: {
          type: 'object',
          properties: {
            file: { type: 'string' },
            line: { type: 'number' },
            severity: { type: 'string', enum: ['CRITICAL', 'HIGH', 'MEDIUM', 'LOW'] },
            category: { type: 'string' },
            description: { type: 'string' },
          },
          required: ['file', 'description', 'severity'],
        },
      },
    },
    required: ['findings'],
  }},
)

const testsResult = await agent(
  'Test coverage and quality review of cargo-agent Rust project. Find all test modules across the codebase (search for #[cfg(test)] mod tests and #[test] attributes). Read the test files and check: 1) What percentage of public functions have tests 2) Test naming quality 3) AAA pattern usage 4) Edge case coverage 5) Async test coverage 6) Integration tests 7) Missing tests for critical paths (agent chat loop, tool execution, error handling) 8) Test isolation issues. List all test files found and identify major modules lacking test coverage.',
  { label: 'test-review', phase: 'Tests', schema: {
    type: 'object',
    properties: {
      testFiles: { type: 'array', items: { type: 'string' } },
      uncoveredModules: { type: 'array', items: { type: 'string' } },
      qualityIssues: {
        type: 'array',
        items: {
          type: 'object',
          properties: {
            file: { type: 'string' },
            description: { type: 'string' },
          },
          required: ['file', 'description'],
        },
      },
    },
    required: ['testFiles', 'uncoveredModules'],
  }},
)

phase('Synthesize')

// Collect all findings
const allFindings = []
if (qualityResult && qualityResult.findings) allFindings.push(...qualityResult.findings)
if (securityResult && securityResult.findings) allFindings.push(...securityResult.findings)
if (performanceResult && performanceResult.findings) allFindings.push(...performanceResult.findings)

const report = await agent(
  `Synthesize a comprehensive code review report for the cargo-agent project.

ARCHITECTURE OVERVIEW:
${exploreResult}

SECURITY FINDINGS:
${JSON.stringify(securityResult, null, 2)}

CODE QUALITY FINDINGS:
${JSON.stringify(qualityResult, null, 2)}

PERFORMANCE FINDINGS:
${JSON.stringify(performanceResult, null, 2)}

TEST COVERAGE FINDINGS:
${JSON.stringify(testsResult, null, 2)}

Produce a structured report with:
1. Executive Summary (top 3-5 issues that need attention)
2. Security Assessment (by severity)
3. Code Quality Assessment (by severity)
4. Performance Assessment
5. Test Coverage Assessment
6. Architecture Observations
7. Recommended Next Steps (prioritized)

For each finding, include the file:line reference, severity, and a brief remediation suggestion.

Format the report as markdown with clear sections. Be concise but thorough. Focus on actionable findings only.`,
  { label: 'synthesize-report', phase: 'Synthesize' },
)

return report