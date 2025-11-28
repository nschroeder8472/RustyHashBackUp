# Contributing to RustyHashBackup

Thank you for your interest in contributing to RustyHashBackup! This document provides guidelines for contributing to the project.

## Welcome

We welcome contributions of all kinds:
- Bug reports and feature requests
- Documentation improvements
- Code contributions (bug fixes, new features, optimizations)
- Testing and feedback

## Ground Rules & Expectations

### Maintainer Authority

All contributions are subject to final approval by the project maintainer. The maintainer:
- Has final say on accepting or rejecting any contribution
- May request changes, clarifications, or alternative approaches
- Makes decisions based on project vision, code quality, and long-term maintainability

### Review Process

- Pull request reviews happen when the maintainer is available
- There are no guaranteed response timeframes
- Please be patient and respectful while waiting for review
- The maintainer may request changes or provide feedback

### AI-Assisted Development

AI tools (ChatGPT, Claude, GitHub Copilot, etc.) are **encouraged and welcomed** for contributions.

**Requirements:**
- You must be able to explain:
  - What your changes do
  - Why you made them
  - How the code works
- AI-generated code must meet all quality standards (tests, clippy, formatting)

**Optional but helpful:**
- Mention which AI tools you used in your PR description
- This helps provide context during code review

## Development Setup

### Prerequisites

- Rust 1.70 or later (install from [rustup.rs](https://rustup.rs))
- Cargo (comes with Rust)

### Getting Started

1. Clone the repository
   ```bash
   git clone https://github.com/nschroeder8472/RustyHashBackUp.git
   cd RustyHashBackup
   ```
2. Build the project:
   ```bash
   cargo build
   ```
3. Run tests:
   ```bash
   cargo test
   ```

4. Run in CLI mode:
   ```bash
   cargo run -- --config example/config.json
   ```

5. Run in API/Web UI mode:
   ```bash
   cargo run -- --api
   # Open http://localhost:8000 in your browser
   ```

## How to Contribute

### Reporting Bugs

Before submitting a bug report:
1. Check existing issues to avoid duplicates
2. Gather relevant information (Rust version, OS, configuration)

When submitting:
- Use a clear, descriptive title
- Provide detailed steps to reproduce the issue
- Include error messages, logs, or screenshots if applicable
- Specify your environment:
  - Rust version (`rustc --version`)
  - Operating system
  - Configuration file (sanitized)

### Suggesting Features

Before suggesting a feature:
1. Check existing issues and discussions
2. Consider if it fits the project's scope

When suggesting:
- Use a clear, descriptive title
- Explain the use case and benefits
- Describe the desired behavior
- Be open to feedback and alternative approaches

### Code Contributions

#### Before You Start

1. **Check existing issues** - Someone may already be working on it
2. **Discuss major changes** - Open an issue first for significant features or refactors
3. **Create a feature branch** - Branch from `main` with a descriptive name:
   ```bash
   git checkout -b feature/your-feature-name
   # or
   git checkout -b fix/bug-description
   ```

#### Coding Standards

All code contributions must meet these requirements:

**Quality Checks:**
- ✅ Tests must pass: `cargo test`
- ✅ Code must be formatted: `cargo fmt`
- ✅ Linting must pass: `cargo clippy` (zero warnings)

**Code Guidelines:**
- Follow existing code patterns and style
- Add unit tests for new functionality
- Update documentation (README, CLAUDE.md) as needed
- Keep changes focused and avoid unrelated modifications
- Use meaningful variable and function names
- Add comments for complex logic (not obvious code)

**Architecture:**
- Reference CLAUDE.md for architecture overview
- Maintain separation between CLI and API modes
- Follow the existing module structure (models, service, repo, utils)
- Use proper error handling (thiserror + anyhow pattern)

#### Commit Messages

- Use clear, descriptive commit messages
- Format: Start with a verb (Add, Fix, Update, Refactor, etc.)
- Reference issues when applicable: "Fix backup verification (#123)"
- Examples:
  - "Add path traversal security checks"
  - "Fix duplicate backup_file() calls"
  - "Update README with API documentation"

#### Pull Request Process

1. **Ensure quality checks pass locally:**
   ```bash
   cargo test
   cargo fmt --check
   cargo clippy
   ```

2. **Update documentation** if needed:
   - README.md for user-facing changes
   - CLAUDE.md for architecture changes
   - Code comments for complex logic

3. **Fill out PR description** with:
   - **What changed:** Brief summary of modifications
   - **Why it changed:** Problem being solved or feature being added
   - **How to test it:** Steps to verify the changes work
   - **(Optional) AI tools used:** Mention if you used AI assistance

4. **Be responsive to feedback:**
   - Address review comments promptly
   - Ask questions if feedback is unclear
   - Be open to requested changes

5. **Maintainer review:**
   - The maintainer will review when available
   - May approve, request changes, or close the PR
   - Final decision rests with the maintainer

## Code of Conduct

This project adheres to a Code of Conduct (see [CODE_OF_CONDUCT](CODE_OF_CONDUCT.md)). By participating, you are expected to uphold this code. Please report unacceptable behavior by opening an issue or contacting the maintainer.

## Getting Help

- **Questions?** Open a GitHub Discussion or Issue
- **Documentation:** Check README.md, CLAUDE.md, and API.md
- **Examples:** See the `example/` directory for sample configurations

## Recognition

Contributors will be acknowledged in release notes and project documentation. Thank you for helping make RustyHashBackup better!
