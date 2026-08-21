# Contributor Guide

Thank you for your interest in contributing to SwapTrade! This guide will help you get started with contributing to the SDK, Demo App, and Smart Contracts.

## Table of Contents

- [Getting Started](#getting-started)
- [Development Workflow](#development-workflow)
- [Code Standards](#code-standards)
- [Testing](#testing)
- [Documentation](#documentation)
- [Submitting Changes](#submitting-changes)
- [Community Guidelines](#community-guidelines)

## Getting Started

### Prerequisites

- Node.js 18+ and npm/yarn
- Rust and Cargo (for contract development)
- Soroban CLI (`stellar` command)
- Git
- Basic knowledge of TypeScript, React, and Rust

### Setting Up Development Environment

1. **Fork and Clone the Repository**

```bash
git clone https://github.com/your-username/swaptrade-contract.git
cd swaptrade-contract
```

2. **Install SDK Dependencies**

```bash
cd sdk
npm install
```

3. **Install Demo App Dependencies**

```bash
cd ../demo-app
npm install
```

4. **Build the Smart Contract**

```bash
cd ../swaptrade-contracts/atomic-swap
stellar contract build
```

## Development Workflow

### Branch Strategy

- `main`: Stable production code
- `develop`: Integration branch for features
- `feature/*`: Feature branches
- `bugfix/*`: Bug fix branches
- `hotfix/*`: Urgent production fixes

### Creating a Feature Branch

```bash
git checkout develop
git pull origin develop
git checkout -b feature/your-feature-name
```

### Making Changes

#### SDK Changes

1. Make your changes in `sdk/src/`
2. Add TypeScript types for new functions
3. Update `sdk/README.md` if API changes
4. Build the SDK: `npm run build`

#### Demo App Changes

1. Make UI changes in `demo-app/src/`
2. Add corresponding E2E tests in `demo-app/tests/e2e/`
3. Update `demo-app/README.md` if UI changes
4. Test locally: `npm run dev`

#### Contract Changes

1. Make changes in `swaptrade-contracts/atomic-swap/src/`
2. Add unit tests in `swaptrade-contracts/atomic-swap/tests/`
3. Build contract: `stellar contract build`
4. Test locally: `cargo test`

### Committing Changes

Follow conventional commits format:

```
type(scope): description

[optional body]

[optional footer]
```

Types:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting)
- `refactor`: Code refactoring
- `test`: Adding or updating tests
- `chore`: Maintenance tasks

Examples:
```bash
git commit -m "feat(sdk): add support for multi-party swaps"
git commit -m "fix(demo): resolve transaction timeout issue"
git commit -m "docs(readme): update installation instructions"
```

## Code Standards

### TypeScript/JavaScript

- Use TypeScript for all new code
- Follow existing code style
- Use meaningful variable and function names
- Add JSDoc comments for public APIs
- Avoid `any` types - use proper typing
- Use async/await instead of callbacks

### React

- Use functional components with hooks
- Keep components small and focused
- Use proper TypeScript types for props
- Follow React best practices
- Use TailwindCSS for styling

### Rust

- Follow Rust naming conventions
- Use `cargo fmt` for formatting
- Add documentation comments (`///`)
- Write unit tests for new functions
- Use `cargo clippy` for linting

### General Guidelines

- Keep functions short and focused
- DRY (Don't Repeat Yourself)
- Write self-documenting code
- Add comments for complex logic
- Handle errors appropriately

## Testing

### SDK Testing

```bash
cd sdk
npm run build
```

### Demo App Testing

```bash
cd demo-app
# Run E2E tests
npm run test:e2e

# Run tests with UI
npm run test:e2e:ui
```

### Contract Testing

```bash
cd swaptrade-contracts/atomic-swap
cargo test
```

### Test Coverage

- Aim for >80% code coverage
- Test edge cases and error conditions
- Mock external dependencies
- Keep tests fast and reliable

## Documentation

### SDK Documentation

- Update `sdk/README.md` for API changes
- Add JSDoc comments to functions
- Include usage examples
- Document error cases

### Demo App Documentation

- Update `demo-app/README.md` for UI changes
- Add screenshots for new features
- Document configuration changes
- Include troubleshooting tips

### Contract Documentation

- Update contract README
- Document new functions
- Include gas estimates
- Add security considerations

## Submitting Changes

### Pull Request Process

1. **Update Your Branch**

```bash
git checkout develop
git pull origin develop
git checkout feature/your-feature
git merge develop
```

2. **Resolve Conflicts**

Resolve any merge conflicts before submitting.

3. **Run Tests**

```bash
# SDK tests
cd sdk && npm run build

# Demo app tests
cd demo-app && npm run test:e2e

# Contract tests
cd swaptrade-contracts/atomic-swap && cargo test
```

4. **Create Pull Request**

- Use the PR template
- Fill in all required sections
- Link related issues
- Add screenshots for UI changes
- Request review from maintainers

### PR Review Process

1. Automated checks must pass
2. At least one maintainer approval required
3. Address all review comments
4. Update documentation as needed
5. Squash commits before merge

### After Merge

- Delete your feature branch
- Update local develop branch
- Celebrate your contribution! 🎉

## Community Guidelines

### Code of Conduct

- Be respectful and inclusive
- Welcome newcomers and help them learn
- Focus on constructive feedback
- Assume good intentions
- Be patient with different skill levels

### Communication

- Use GitHub issues for bug reports
- Use discussions for questions
- Be clear and specific in communications
- Provide context when asking for help
- Share knowledge with the community

### Recognition

- Contributors will be listed in CONTRIBUTORS.md
- Notable contributions will be highlighted
- Active contributors may be invited as maintainers

## Getting Help

- **Documentation**: Check README files and inline docs
- **Issues**: Search existing GitHub issues
- **Discussions**: Start a new discussion for questions
- **Discord**: Join our Discord community (link in README)

## Areas for Contribution

### High Priority

- [ ] Additional contract examples
- [ ] More comprehensive E2E tests
- [ ] Performance optimizations
- [ ] Security audits and improvements

### Medium Priority

- [ ] Additional language bindings (Python, Go)
- [ ] Mobile app examples
- [ ] Advanced demo scenarios
- [ ] Integration with other Stellar tools

### Low Priority

- [ ] UI improvements and animations
- [ ] Additional themes for demo app
- [ ] Documentation improvements
- [ ] Code examples and tutorials

## Resources

- [Stellar Documentation](https://developers.stellar.org/)
- [Soroban Documentation](https://soroban.stellar.org/)
- [TypeScript Handbook](https://www.typescriptlang.org/docs/)
- [React Documentation](https://react.dev/)
- [Rust Book](https://doc.rust-lang.org/book/)

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
